//! A scripted human (design §8 T1): plays the human slot through the protocol only, as a client
//! would: it reads the game with [`Query`]s (the JSON replies of `view`, `map`, `pending`,
//! `group`, `combat`, `city`, `techs`, `civics`, `players`, `trade`) and acts with
//! [`Command`]s, taking the commands of the `group` query's actions as they are. Static data
//! (unit classes, terrain flags) comes from the data tables, as a client reads `data`. It has a
//! private seeded RNG. It is a test driver, not an AI: it exercises the command paths, the
//! decisions and the views.
//!
//! Policy: research the cheapest tech; production rotation warrior, settler, worker, building
//! (a city with fewer than two defenders first trains its strongest land unit); settlers found
//! cities (or walk to a site); workers take the first build; military units attack adjacent
//! enemies at odds of 60% or more, fortify in cities or explore; sometimes cast a spell; take
//! promotions; revolt when a civic unlocks; declare war on a met AI around turn 80 and propose
//! peace from turn 120; answer popups and decisions with a seeded mix (first option, advisor,
//! random); end the turn, sometimes with `force`.
//!
//! With [`Bot::noisy`] it is the noisy client of T10: after every command, at every control
//! point and inside every blocking decision ([`NoisyAnswerer`]) it issues every allow-listed
//! query ([`noise`]); the game must not notice.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::rng::Rng;
use ffh_core::structs::TradeData;
use ffh_data::*;

use crate::command::{CmdOk, Command, Confirm};
use crate::decision::Decision;
use crate::human::{Answer, Answerer, DecisionReader, FnAnswerer};
use crate::json::Json;
use crate::query::{Layer, Query, QueryErr};
use crate::session::{Phase, Session, Stop};

/// What the bot did (test assertions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotStats {
    pub commands: usize,
    pub refused: usize,
    pub confirms: usize,
    pub events_answered: usize,
    pub votes_answered: usize,
    pub elections_answered: usize,
    pub research_chosen: usize,
    pub productions: usize,
    pub found_orders: usize,
    pub attacks: usize,
    pub spells: usize,
    pub promotions: usize,
    pub revolutions: usize,
    pub wars: usize,
    pub peace_proposals: usize,
    pub peace_made: usize,
    pub end_turns: usize,
    pub forced_end_turns: usize,
    /// Queries issued (the noisy client).
    pub queries: usize,
}

// ---- JSON access ------------------------------------------------------------------------------

fn int(j: &Json, k: &str) -> i64 {
    j.get(k).and_then(Json::as_i64).unwrap_or(-1)
}

fn arr<'a>(j: &'a Json, k: &str) -> &'a [Json] {
    j.get(k).and_then(Json::as_arr).unwrap_or(&[])
}

fn flag(j: &Json, k: &str) -> bool {
    j.get(k).and_then(Json::as_bool).unwrap_or(false)
}

fn ints(j: &Json, k: &str) -> Vec<i64> {
    arr(j, k).iter().filter_map(Json::as_i64).collect()
}

fn unit_ids(j: &Json, k: &str) -> Vec<UnitId> {
    ints(j, k).into_iter().map(|u| UnitId(u as u32)).collect()
}

/// A unit the bot sees (`view.units`).
struct SeenUnit {
    id: UnitId,
    owner: i64,
    unit_type: UnitType,
    x: i32,
    y: i32,
    group: i64,
}

/// What the bot knows at a moment: the `view` and the `map` of the human.
struct Seen {
    me: i64,
    w: i32,
    h: i32,
    wrap_x: bool,
    wrap_y: bool,
    vis: Vec<i64>,
    plot_type: Vec<i64>,
    terrain: Vec<i64>,
    feature: Vec<i64>,
    bonus: Vec<i64>,
    improvement: Vec<i64>,
    owner: Vec<i64>,
    city: Vec<i64>,
    /// Our cities: (id, x, y).
    cities: Vec<(CityId, i32, i32)>,
    /// Revealed cities of players we are at war with: (x, y).
    enemy_cities: Vec<(i32, i32)>,
    units: Vec<SeenUnit>,
    /// Players at war with us.
    war: BTreeSet<i64>,
    /// Our capital.
    capital: Option<(i32, i32)>,
    /// The groups waiting for orders (`pending.ready_groups`).
    ready: BTreeSet<i64>,
}

impl Seen {
    fn read(s: &Session) -> Seen {
        let view = s.query(&Query::View).expect("view");
        let map = s.query(&Query::Map { since: None, epoch: None }).expect("map");
        let me = int(view.get("me").unwrap(), "player");
        let war: BTreeSet<i64> = arr(&view, "players").iter().filter(|p| flag(p, "war")).map(|p| int(p, "id")).collect();
        let mut cities = Vec::new();
        let mut capital = None;
        let mut enemy_cities = Vec::new();
        for c in arr(&view, "cities") {
            let (x, y) = (int(c, "x") as i32, int(c, "y") as i32);
            if int(c, "owner") == me {
                cities.push((CityId(int(c, "id") as u32), x, y));
                if flag(c, "capital") {
                    capital = Some((x, y));
                }
            } else if war.contains(&int(c, "owner")) {
                enemy_cities.push((x, y));
            }
        }
        let units = arr(&view, "units")
            .iter()
            .map(|u| SeenUnit {
                id: UnitId(int(u, "id") as u32),
                owner: int(u, "owner"),
                unit_type: UnitType::from_usize(int(u, "type") as usize),
                x: int(u, "x") as i32,
                y: int(u, "y") as i32,
                group: int(u, "group"),
            })
            .collect();
        Seen {
            me,
            w: int(&map, "w") as i32,
            h: int(&map, "h") as i32,
            wrap_x: flag(&map, "wrap_x"),
            wrap_y: flag(&map, "wrap_y"),
            vis: ints(&map, "vis"),
            plot_type: ints(&map, "plot_type"),
            terrain: ints(&map, "terrain"),
            feature: ints(&map, "feature"),
            bonus: ints(&map, "bonus"),
            improvement: ints(&map, "improvement"),
            owner: ints(&map, "owner"),
            city: ints(&map, "city"),
            cities,
            enemy_cities,
            units,
            war,
            capital,
            ready: BTreeSet::new(),
        }
    }

    /// The plot index of `(x, y)`, wrapping as the map does.
    fn at(&self, x: i32, y: i32) -> Option<usize> {
        let x = if self.wrap_x { x.rem_euclid(self.w) } else { x };
        let y = if self.wrap_y { y.rem_euclid(self.h) } else { y };
        ((0..self.w).contains(&x) && (0..self.h).contains(&y)).then(|| (y * self.w + x) as usize)
    }

    fn xy(&self, i: usize) -> (i32, i32) {
        (i as i32 % self.w, i as i32 / self.w)
    }

    /// The plots within `r` of `(x, y)` (a square, without the centre), rows first.
    fn near(&self, x: i32, y: i32, r: i32) -> Vec<usize> {
        let mut v = Vec::new();
        let centre = self.at(x, y);
        for dy in -r..=r {
            for dx in -r..=r {
                if let Some(i) = self.at(x + dx, y + dy) {
                    if Some(i) != centre && !v.contains(&i) {
                        v.push(i);
                    }
                }
            }
        }
        v
    }

    /// C++ `plotDistance` (wrap-aware).
    fn distance(&self, x1: i32, y1: i32, x2: i32, y2: i32) -> i32 {
        let d = |a: i32, b: i32, n: i32, wrap: bool| {
            let d = (a - b).abs();
            if wrap {
                d.min(n - d)
            } else {
                d
            }
        };
        let (dx, dy) = (d(x1, x2, self.w, self.wrap_x), d(y1, y2, self.h, self.wrap_y));
        dx.max(dy) + dx.min(dy) / 2
    }

    fn land(&self, i: usize) -> bool {
        self.vis[i] > 0 && self.plot_type[i] != PlotType::PLOT_OCEAN.idx() as i64 && self.plot_type[i] != PlotType::PLOT_PEAK.idx() as i64
    }

    /// A plot that looks like a city site (C++ `canFound` as far as the map shows it): revealed
    /// land of a foundable terrain, no city-blocking feature, not someone else's, no city within
    /// `MIN_CITY_RANGE`, no permanent improvement, no mana.
    fn site(&self, d: &GameData, i: usize) -> bool {
        if !self.land(i) || (self.owner[i] >= 0 && self.owner[i] != self.me) {
            return false;
        }
        let t = &d.terrains[TerrainType::from_usize(self.terrain[i] as usize)];
        if !(t.found || t.found_coast || t.found_fresh_water) || t.impassable {
            return false;
        }
        if self.feature[i] >= 0 && d.features[FeatureType::from_usize(self.feature[i] as usize)].no_city {
            return false;
        }
        if self.improvement[i] >= 0 && d.improvements[ImprovementType::from_usize(self.improvement[i] as usize)].permanent {
            return false;
        }
        if self.bonus[i] >= 0 {
            let class = d.bonuses[BonusType::from_usize(self.bonus[i] as usize)].bonus_class_type.map_or(-1, |c| c.idx() as i32);
            if class >= 0 && (class == d.defines.bonusclass_mana || class == d.defines.bonusclass_mana_raw) {
                return false;
            }
        }
        let (x, y) = self.xy(i);
        let r = d.defines.min_city_range;
        !(-r..=r).any(|dy| (-r..=r).any(|dx| self.at(x + dx, y + dy).is_some_and(|j| self.city[j] >= 0)))
    }

    /// Our combat units on plot `(x, y)` outside group `group`.
    fn guards(&self, d: &GameData, x: i32, y: i32, group: i64) -> usize {
        self.units.iter().filter(|u| u.owner == self.me && u.x == x && u.y == y && u.group != group && d.units[u.unit_type].combat > 0).count()
    }

    /// A unit of a player at war with us on plot `i`.
    fn enemy_on(&self, i: usize) -> bool {
        let (x, y) = self.xy(i);
        self.units.iter().any(|u| u.x == x && u.y == y && self.war.contains(&u.owner))
    }
}

/// The scripted human.
pub struct Bot {
    rng: Arc<Mutex<Rng>>,
    pub stats: BotStats,
    /// Production rotation position per city.
    rotation: BTreeMap<CityId, usize>,
    /// Game turn from which the bot declares war on a met AI (once).
    pub war_turn: i32,
    /// Game turn from which the bot proposes peace to the civilizations at war with it.
    pub peace_turn: i32,
    /// Civics seen available (a new one triggers a revolution).
    civics_seen: BTreeSet<i64>,
    /// The noisy client (T10): every allow-listed query after each command and at each control
    /// point.
    pub noisy: bool,
}

impl Bot {
    pub fn new(seed: u32) -> Bot {
        Bot {
            rng: Arc::new(Mutex::new(Rng::new(seed))),
            stats: BotStats::default(),
            rotation: BTreeMap::new(),
            war_turn: 80,
            peace_turn: 120,
            civics_seen: BTreeSet::new(),
            noisy: false,
        }
    }

    fn rand(&self, n: usize) -> usize {
        if n <= 1 {
            return 0;
        }
        self.rng.lock().unwrap().get_i(n as i32, "bot") as usize
    }

    /// The bot's answerer for blocking decisions: a seeded mix of the first option, the
    /// advisor's pick and a random enabled option. Shares the bot's RNG.
    pub fn answerer(&self) -> Arc<dyn Answerer> {
        let rng = self.rng.clone();
        Arc::new(FnAnswerer(move |_g: &Game, d: &Decision| {
            let mut r = rng.lock().unwrap();
            let enabled: Vec<usize> = (0..d.options.len()).filter(|&i| d.is_valid(i)).collect();
            match r.get_i(3, "bot answer") {
                0 => Answer::Option(enabled.first().copied().unwrap_or(0)),
                1 => Answer::Advisor,
                _ => Answer::Option(enabled[r.get_i(enabled.len().max(1) as i32, "bot answer") as usize % enabled.len().max(1)]),
            }
        }))
    }

    /// [`Bot::answerer`] that first issues every query through the decision's reader.
    pub fn noisy_answerer(&self) -> (Arc<NoisyAnswerer>, Arc<dyn Answerer>) {
        let noisy = Arc::new(NoisyAnswerer { inner: self.answerer(), queries: Mutex::new(0), decisions: Mutex::new(0) });
        (noisy.clone(), noisy)
    }

    /// Executes `cmd`; confirmations are accepted for the commands the bot means (a war move
    /// is declined). Returns the reply when applied.
    fn exec(&mut self, s: &mut Session, cmd: Command) -> Option<Option<bool>> {
        if !matches!(s.phase(), Phase::Human(_)) {
            return None;
        }
        self.stats.commands += 1;
        let r = match s.execute(&cmd) {
            Ok(CmdOk::Applied { accepted, .. }) => Some(accepted),
            Ok(CmdOk::NeedsConfirm(c)) => {
                self.stats.confirms += 1;
                let again = match (c, cmd) {
                    (Confirm::DeclareWar { .. }, Command::Move { units, x, y, append, .. }) => Some(Command::Move { units, x, y, append, declare_war: Some(false) }),
                    (Confirm::LoadUnit { .. }, _) => None,
                    (Confirm::EndTurn { .. }, _) => Some(Command::EndTurn { force: true }),
                    (_, Command::DoCommand { units, command, data1, data2, alt, .. }) => Some(Command::DoCommand { units, command, data1, data2, alt, confirmed: true }),
                    _ => None,
                };
                match again {
                    None => None,
                    Some(again) => {
                        self.stats.commands += 1;
                        match s.execute(&again) {
                            Ok(CmdOk::Applied { accepted, .. }) => Some(accepted),
                            _ => {
                                self.stats.refused += 1;
                                None
                            }
                        }
                    }
                }
            }
            Err(_) => {
                self.stats.refused += 1;
                None
            }
        };
        if self.noisy {
            self.stats.queries += noise(s);
        }
        r
    }

    fn ok(&mut self, s: &mut Session, cmd: Command) -> bool {
        self.exec(s, cmd).is_some()
    }

    /// Plays the human's turn from a control point through `end_turn`.
    pub fn play(&mut self, s: &mut Session) {
        self.play_with(s, &mut |_| {});
    }

    /// [`Bot::play`], calling `mid` halfway (after the popups, production and empire commands,
    /// before the units move): tests save and reload the session there.
    pub fn play_with(&mut self, s: &mut Session, mid: &mut dyn FnMut(&mut Session)) {
        if self.noisy {
            self.stats.queries += noise(s);
        }
        let d = s.g.d;
        let pending = s.query(&Query::Pending).expect("pending");
        self.popups(s, &pending);
        self.cities(s, &pending);
        self.empire(s);
        mid(s);
        // Units: a few passes over the ready groups (an order can leave a group ready).
        for _ in 0..3 {
            let pending = s.query(&Query::Pending).expect("pending");
            let ready = ints(&pending, "ready_groups");
            if ready.is_empty() {
                break;
            }
            let mut seen = Seen::read(s);
            seen.ready = ready.iter().copied().collect();
            // Units a stack order of this pass took along.
            let mut handled: BTreeSet<UnitId> = BTreeSet::new();
            for grp in ready {
                // The group's units, if it still waits for orders.
                let units: Vec<UnitId> = seen.units.iter().filter(|u| u.group == grp).map(|u| u.id).collect();
                if units.is_empty() || units.iter().any(|u| handled.contains(u)) {
                    continue;
                }
                let Ok(gq) = s.query(&Query::Group { units: units.clone() }) else { continue };
                if !flag(&gq, "ready") {
                    continue;
                }
                let order = self.order(s, &seen, grp, &units, &gq, &handled);
                if let Command::Move { units: stack, .. } = &order {
                    handled.extend(stack.iter().copied());
                }
                if !self.ok(s, order) {
                    self.ok(s, Command::Mission { units: units.clone(), mission: MissionType::MISSION_SKIP, data1: -1, data2: -1, flags: 0, append: false });
                }
            }
        }
        // Promotions: the first promotion the unit can take.
        let view = s.query(&Query::View).expect("view");
        let ready: Vec<UnitId> = arr(&view, "units").iter().filter(|u| flag(u, "promo_ready") && int(u, "owner") == int(view.get("me").unwrap(), "player")).map(|u| UnitId(int(u, "id") as u32)).collect();
        for u in ready {
            let Ok(gq) = s.query(&Query::Group { units: vec![u] }) else { continue };
            let cmd = arr(&gq, "actions").iter().find(|a| a.get("kind") == Some(&Json::from("promotion")) && flag(a, "enabled")).and_then(|a| a.get("cmd")).and_then(|c| Command::from_json(d, c).ok());
            if let Some(cmd) = cmd {
                if self.ok(s, cmd) {
                    self.stats.promotions += 1;
                }
            }
        }
        // End the turn.
        let force = self.rand(5) == 0;
        if self.ok(s, Command::EndTurn { force }) {
            self.stats.end_turns += 1;
            if force {
                self.stats.forced_end_turns += 1;
            }
        }
    }

    /// Events, votes, elections, research.
    fn popups(&mut self, s: &mut Session, p: &Json) {
        for e in arr(p, "events") {
            let options: Vec<i64> = arr(e, "options").iter().filter(|o| flag(o, "enabled")).map(|o| int(o, "event")).collect();
            if options.is_empty() {
                continue;
            }
            let event = EventType::from_usize(options[self.rand(options.len())] as usize);
            if self.ok(s, Command::Event { triggered: TriggeredId(int(e, "triggered") as u32), event }) {
                self.stats.events_answered += 1;
            }
        }
        for v in arr(p, "votes") {
            let choices = ints(v, "choices");
            let choice = ffh_core::structs::PlayerVote(choices[self.rand(choices.len())] as i32);
            if self.ok(s, Command::Vote { vote: VoteId(int(v, "vote") as u32), choice }) {
                self.stats.votes_answered += 1;
            }
        }
        for e in arr(p, "elections") {
            // One of the resolutions, or none.
            let n = arr(e, "options").len();
            let pick = self.rand(n + 1);
            let option = (pick < n).then_some(pick);
            if self.ok(s, Command::Election { selection: VoteId(int(e, "selection") as u32), option }) {
                self.stats.elections_answered += 1;
            }
        }
        if flag(p, "research_needed") {
            let techs = s.query(&Query::Techs).expect("techs");
            let tech = arr(&techs, "techs").iter().filter(|t| flag(t, "can_research")).min_by_key(|t| (int(t, "cost"), int(t, "tech"))).map(|t| int(t, "tech"));
            if let Some(t) = tech {
                if self.ok(s, Command::Research { tech: Some(TechType::from_usize(t as usize)), shift: false }) {
                    self.stats.research_chosen += 1;
                }
            }
        }
    }

    /// Production for idle cities: the rotation warrior, settler, worker, building.
    fn cities(&mut self, s: &mut Session, p: &Json) {
        let d = s.g.d;
        let class_of = |t: i64| d.units[UnitType::from_usize(t as usize)].unit_class_type;
        for c in ints(p, "idle_cities") {
            let c = CityId(c as u32);
            let Ok(cq) = s.query(&Query::City { city: c }) else { continue };
            let me = int(&cq, "owner");
            let start = *self.rotation.get(&c).unwrap_or(&0);
            let enabled: Vec<(i64, i64)> = arr(&cq, "buildable").iter().filter(|b| flag(b, "enabled")).map(|b| (int(b, "order"), int(b, "type"))).collect();
            let train = OrderType::ORDER_TRAIN.idx() as i64;
            let trains = |pred: &dyn Fn(i64) -> bool| enabled.iter().filter(|(o, t)| *o == train && pred(*t)).map(|&(_, t)| t).collect::<Vec<_>>();
            let guards = arr(&cq, "garrison").iter().filter(|u| int(u, "owner") == me && d.units[UnitType::from_usize(int(u, "type") as usize)].combat > 0).count();
            let mut choice = None;
            // A city with fewer than two combat units (the capital three, from turn 30: the
            // barbarians come) first trains the strongest it can.
            let wanted = if flag(&cq, "capital") && s.g.game_turn() >= 30 { 3 } else { 2 };
            if guards < wanted {
                let best = trains(&|t| {
                    let u = &d.units[UnitType::from_usize(t as usize)];
                    u.combat > 0 && u.domain_type == Some(DomainType::DOMAIN_LAND)
                })
                .into_iter()
                .max_by_key(|&t| (d.units[UnitType::from_usize(t as usize)].combat, std::cmp::Reverse(t)));
                if let Some(t) = best {
                    choice = Some((start.wrapping_sub(1), OrderType::ORDER_TRAIN, t));
                }
            }
            if choice.is_none() {
                let rotation = ["UNITCLASS_WARRIOR", "UNITCLASS_SETTLER", "UNITCLASS_WORKER"].map(|n| d.ty::<UnitClassType>(n));
                for k in 0..4 {
                    let slot = (start + k) % 4;
                    if slot < 3 {
                        if let Some(class) = rotation[slot] {
                            if let Some(&t) = trains(&|t| class_of(t) == Some(class)).first() {
                                choice = Some((slot, OrderType::ORDER_TRAIN, t));
                                break;
                            }
                        }
                    } else if let Some(&(_, b)) = enabled.iter().find(|(o, _)| *o == OrderType::ORDER_CONSTRUCT.idx() as i64) {
                        choice = Some((slot, OrderType::ORDER_CONSTRUCT, b));
                        break;
                    }
                }
            }
            if choice.is_none() {
                choice = trains(&|_| true).first().map(|&t| (start, OrderType::ORDER_TRAIN, t));
            }
            if choice.is_none() {
                choice = enabled.iter().find(|(o, _)| *o == OrderType::ORDER_MAINTAIN.idx() as i64).map(|&(_, t)| (start, OrderType::ORDER_MAINTAIN, t));
            }
            if let Some((slot, order, data)) = choice {
                if self.ok(s, Command::PushOrder { city: c, order, data: data as i32, alt: false, shift: false, ctrl: false }) {
                    self.stats.productions += 1;
                    self.rotation.insert(c, slot.wrapping_add(1));
                }
            }
        }
    }

    /// Civics, war, peace.
    fn empire(&mut self, s: &mut Session) {
        // Revolt when a civic unlocks (once per civic; the first ones are what we start with).
        let civics = s.query(&Query::Civics { with: Vec::new() }).expect("civics");
        let mut available: Vec<(i64, bool)> = Vec::new();
        for o in arr(&civics, "options") {
            for c in arr(o, "civics") {
                if flag(c, "available") {
                    available.push((int(c, "civic"), flag(c, "current")));
                }
            }
        }
        available.sort_unstable();
        let first = self.civics_seen.is_empty();
        let new: Vec<(i64, bool)> = available.iter().copied().filter(|(c, _)| !self.civics_seen.contains(c)).collect();
        self.civics_seen.extend(new.iter().map(|&(c, _)| c));
        if !first {
            if let Some(&(c, current)) = new.first() {
                if !current && self.ok(s, Command::UpdateCivics { civics: vec![CivicType::from_usize(c as usize)] }) {
                    self.stats.revolutions += 1;
                }
            }
        }
        let turn = s.g.game_turn();
        if self.stats.wars == 0 && turn >= self.war_turn {
            let players = s.query(&Query::Players).expect("players");
            let target = arr(&players, "players").iter().find(|p| flag(p, "can_declare_war") && !flag(p, "barbarian")).map(|p| int(p, "team"));
            if let Some(t) = target {
                if self.ok(s, Command::DeclareWar { team: TeamId(t as u8) }) {
                    self.stats.wars += 1;
                }
            }
        }
        // Peace with the civilizations at war with us (every 5 turns, until one accepts).
        if turn >= self.peace_turn && self.stats.peace_made == 0 && (turn - self.peace_turn) % 5 == 0 {
            let players = s.query(&Query::Players).expect("players");
            let at_war: Vec<i64> = arr(&players, "players").iter().filter(|p| flag(p, "war") && flag(p, "contact") && !flag(p, "barbarian") && flag(p, "alive")).map(|p| int(p, "id")).collect();
            for p in at_war {
                let to = PlayerId(p as u8);
                let Ok(t) = s.query(&Query::Trade { with: to }) else { continue };
                let peace = Json::idx(TradeableItem::TRADE_PEACE_TREATY);
                if !arr(&t, "we_give").iter().any(|i| i.get("item") == Some(&peace)) {
                    continue;
                }
                let item = || vec![TradeData { item_type: TradeableItem::TRADE_PEACE_TREATY, data: 0, offering: false, hidden: false }];
                self.stats.peace_proposals += 1;
                if self.exec(s, Command::ProposeDeal { to, give: item(), get: item() }) == Some(Some(true)) {
                    self.stats.peace_made += 1;
                    break;
                }
            }
        }
    }

    /// An order for a ready group (its head decides), from the `group` query's actions.
    fn order(&mut self, s: &Session, seen: &Seen, grp: i64, units: &[UnitId], gq: &Json, handled: &BTreeSet<UnitId>) -> Command {
        let d = s.g.d;
        let roll = self.rand(100);
        let pick = self.rand(1 << 16);
        let actions = arr(gq, "actions");
        let action = |kind: &str, pred: &dyn Fn(&Json) -> bool| -> Option<Command> {
            actions
                .iter()
                .find(|a| a.get("kind") == Some(&Json::from(kind)) && flag(a, "enabled") && pred(a))
                .and_then(|a| a.get("cmd"))
                .and_then(|c| Command::from_json(d, c).ok())
        };
        let mission = |m: MissionType| action("mission", &|a| int(a, "mission") == m.idx() as i64);
        let head = &arr(gq, "units")[0];
        let info = &d.units[UnitType::from_usize(int(head, "type") as usize)];
        let (x, y) = (int(head, "x") as i32, int(head, "y") as i32);
        let move_to = |i: usize, declare_war: Option<bool>| {
            let (px, py) = seen.xy(i);
            Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war }
        };
        let decided = (|| -> Option<Command> {
            // Settlers.
            if info.found {
                if let Some(c) = mission(MissionType::MISSION_FOUND) {
                    return Some(c);
                }
                let sites: Vec<usize> = seen.near(x, y, 4).into_iter().filter(|&i| seen.site(d, i)).collect();
                if !sites.is_empty() {
                    return Some(move_to(sites[pick % sites.len()], None));
                }
            }
            // Workers: the first build here, else go to a plot of ours without improvement.
            if let Some(c) = mission(MissionType::MISSION_BUILD) {
                return Some(c);
            }
            if info.builds.iter().any(|&b| b) {
                let spots: Vec<usize> =
                    seen.near(x, y, 3).into_iter().filter(|&i| seen.land(i) && seen.owner[i] == seen.me && seen.improvement[i] < 0 && seen.city[i] < 0).collect();
                if !spots.is_empty() {
                    return Some(move_to(spots[pick % spots.len()], None));
                }
                return None;
            }
            if info.combat <= 0 {
                return None;
            }
            // Attack an adjacent enemy at good odds.
            if int(head, "moves") > 0 {
                for i in seen.near(x, y, 1) {
                    if !seen.enemy_on(i) {
                        continue;
                    }
                    let (px, py) = seen.xy(i);
                    if let Ok(c) = s.query(&Query::Combat { units: units.to_vec(), x: px, y: py }) {
                        if flag(&c, "valid") && int(&c, "odds") >= 600 {
                            return Some(move_to(i, Some(false)));
                        }
                    }
                }
            }
            // A spell now and then.
            if roll < 5 {
                if let Some(Command::DoCommand { units, command, data1, data2, alt, .. }) = action("spell", &|_| true) {
                    return Some(Command::DoCommand { units, command, data1, data2, alt, confirmed: true });
                }
            }
            // Defend cities (two combat units each), else explore.
            let in_city = seen.cities.iter().any(|&(_, cx, cy)| (cx, cy) == (x, y));
            if in_city && seen.guards(d, x, y, grp) < 2 {
                if let Some(c) = mission(MissionType::MISSION_FORTIFY) {
                    return Some(c);
                }
            }
            let weak = seen.cities.iter().filter(|&&(_, cx, cy)| (cx, cy) != (x, y) && seen.guards(d, cx, cy, grp) < 2).min_by_key(|&&(c, cx, cy)| (seen.distance(x, y, cx, cy), c));
            if let Some(&(_, cx, cy)) = weak {
                return Some(Command::Move { units: units.to_vec(), x: cx, y: cy, append: false, declare_war: None });
            }
            // Spare units rally in the capital; three or more ready there march together on the
            // nearest known enemy city (the selection becomes one group: `move` joins it).
            if let Some((cx, cy)) = seen.capital {
                let target = seen.enemy_cities.iter().map(|&(tx, ty)| (seen.distance(x, y, tx, ty), tx, ty)).filter(|&(dist, _, _)| dist <= 16).min();
                if (x, y) == (cx, cy) {
                    let stack: Vec<UnitId> = seen
                        .units
                        .iter()
                        .filter(|u| {
                            let info = &d.units[u.unit_type];
                            u.owner == seen.me
                                && (u.x, u.y) == (x, y)
                                && seen.ready.contains(&u.group)
                                && !handled.contains(&u.id)
                                && info.combat > 0
                                && info.domain_type == Some(DomainType::DOMAIN_LAND)
                        })
                        .map(|u| u.id)
                        .collect();
                    if let (true, Some((_, tx, ty))) = (stack.len() >= 3 && stack.iter().any(|u| units.contains(u)), target) {
                        return Some(Command::Move { units: stack, x: tx, y: ty, append: false, declare_war: Some(false) });
                    }
                } else if roll < 40 && target.is_none_or(|(dist, _, _)| dist > 4) {
                    return Some(Command::Move { units: units.to_vec(), x: cx, y: cy, append: false, declare_war: None });
                }
            }
            // Spare units go for the nearest known enemy city (barbarian cities included).
            let target = seen.enemy_cities.iter().map(|&(cx, cy)| (seen.distance(x, y, cx, cy), cx, cy)).filter(|&(dist, _, _)| dist <= 12).min();
            if let (Some((_, cx, cy)), true) = (target, roll < 60) {
                return Some(Command::Move { units: units.to_vec(), x: cx, y: cy, append: false, declare_war: Some(false) });
            }
            if roll < 70 {
                let around: Vec<usize> = seen.near(x, y, 2).into_iter().filter(|&i| seen.land(i)).collect();
                if !around.is_empty() {
                    return Some(move_to(around[pick % around.len()], None));
                }
            }
            // Otherwise wait (skip): a spare unit is ordered again next turn.
            None
        })();
        match decided {
            Some(c) => {
                match &c {
                    Command::Mission { mission: MissionType::MISSION_FOUND, .. } => self.stats.found_orders += 1,
                    Command::Move { declare_war: Some(false), .. } => self.stats.attacks += 1,
                    Command::DoCommand { command: CommandType::COMMAND_CAST, .. } => self.stats.spells += 1,
                    _ => {}
                }
                c
            }
            None => Command::Mission { units: units.to_vec(), mission: MissionType::MISSION_SKIP, data1: -1, data2: -1, flags: 0, append: false },
        }
    }
}

/// Every allow-listed query of the noisy client (design §8 T10), through `query`: view, map,
/// every group's details and actions, 5 path and 5 combat previews, every city screen, techs,
/// civics, religions, players, trade with every met civilization, inspect, text and the ASCII
/// layers. Returns the number of queries; an error other than `busy` fails.
pub fn noise_with(mut query: impl FnMut(&Query) -> Result<Json, QueryErr>) -> usize {
    let mut n = 0;
    // `edge`: a preview whose target is off the map (at an edge) is `invalid`: skipped.
    let mut ask = |q: Query, edge: bool| -> Option<Json> {
        n += 1;
        match query(&q) {
            Ok(j) => Some(j),
            Err(QueryErr::Busy(_)) => None,
            Err(QueryErr::Invalid(_)) if edge => None,
            Err(e) => panic!("noisy client: {q:?} refused: {e}"),
        }
    };
    let Some(view) = ask(Query::View, false) else { return 1 };
    ask(Query::Map { since: None, epoch: None }, false);
    let me = int(view.get("me").unwrap(), "player");
    let mut previews = 0;
    for grp in arr(&view, "groups") {
        let units = unit_ids(grp, "units");
        if units.is_empty() {
            continue;
        }
        let Some(gq) = ask(Query::Group { units: units.clone() }, false) else { continue };
        if previews < 5 {
            previews += 1;
            let plot = arr(&gq, "plot");
            let (x, y) = (plot.first().and_then(Json::as_i64).unwrap_or(0) as i32, plot.get(1).and_then(Json::as_i64).unwrap_or(0) as i32);
            ask(Query::Path { units: units.clone(), x: x + 3, y, flags: 0 }, true);
            ask(Query::Combat { units: units.clone(), x: x + 1, y }, true);
        }
    }
    for c in arr(&view, "cities").iter().filter(|c| int(c, "owner") == me) {
        ask(Query::City { city: CityId(int(c, "id") as u32) }, false);
    }
    for q in [Query::Techs, Query::Civics { with: Vec::new() }, Query::Religions, Query::Text] {
        ask(q, false);
    }
    if let Some(players) = ask(Query::Players, false) {
        for p in arr(&players, "players").iter().filter(|p| !flag(p, "barbarian") && flag(p, "alive")) {
            ask(Query::Trade { with: PlayerId(int(p, "id") as u8) }, false);
        }
    }
    if let Some(u) = arr(&view, "units").iter().find(|u| int(u, "owner") == me) {
        ask(Query::Inspect { kind: "unit".into(), oid: int(u, "id") }, false);
    }
    for layer in Layer::ALL {
        ask(Query::Ascii { x: 0, y: 0, w: 0, h: 0, layer }, false);
    }
    n
}

/// [`noise_with`] on a session.
pub fn noise(s: &Session) -> usize {
    noise_with(|q| s.query(q))
}

/// The noisy client's answerer: issues every query through the decision's reader, then answers
/// as `inner`.
pub struct NoisyAnswerer {
    inner: Arc<dyn Answerer>,
    /// Queries issued inside decisions.
    pub queries: Mutex<usize>,
    /// Decisions seen.
    pub decisions: Mutex<usize>,
}

impl Answerer for NoisyAnswerer {
    fn ask(&self, g: &Game, d: &Decision) -> Answer {
        self.inner.ask(g, d)
    }

    fn ask_with(&self, r: &DecisionReader, d: &Decision) -> Answer {
        let n = noise_with(|q| r.query(q));
        *self.queries.lock().unwrap() += n;
        *self.decisions.lock().unwrap() += 1;
        self.inner.ask(r.game(), d)
    }
}

/// Plays `s` with `bot` until game turn `until` or the game ends: `advance`, then the bot's
/// turn at each control point. `on_control` runs at every control point before the bot acts.
pub fn run(s: &mut Session, bot: &mut Bot, until: i32, mut on_control: impl FnMut(&mut Session, &mut Bot)) -> Stop {
    loop {
        let stop = s.advance(&mut |_| {});
        if stop != Stop::Human {
            return stop;
        }
        if s.g.game_turn() >= until {
            return stop;
        }
        on_control(s, bot);
        if !matches!(s.phase(), Phase::Human(_)) {
            continue;
        }
        bot.play(s);
    }
}
