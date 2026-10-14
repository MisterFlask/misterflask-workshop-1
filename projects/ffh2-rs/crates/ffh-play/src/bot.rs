//! A scripted human (design §8 T1): plays the human slot only through `Session` commands,
//! reading the game through `Session::read` (under a `QueryGuard`), with a private seeded RNG.
//! It is a test driver, not an AI: it exercises the command paths and the decisions.
//!
//! Policy: research the cheapest tech; production rotation worker, warrior, settler, building;
//! settlers found cities (or walk to a site); workers take the first build; military units
//! attack adjacent enemies at odds of 60% or more, fortify in cities or explore; sometimes cast
//! a spell; take promotions; revolt when a civic unlocks; declare war on a met AI around turn
//! 80; answer popups and decisions with a seeded mix (first option, advisor, random); end the
//! turn, sometimes with `force`.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::rng::Rng;
use ffh_data::*;

use crate::command::{CmdOk, Command, Confirm};
use crate::decision::{Decision, Pending};
use crate::human::{Answer, Answerer, FnAnswerer};
use crate::session::{Phase, Session, Stop};

/// What the bot did (test assertions).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BotStats {
    pub commands: usize,
    pub refused: usize,
    pub confirms: usize,
    pub events_answered: usize,
    pub votes_answered: usize,
    pub research_chosen: usize,
    pub productions: usize,
    pub found_orders: usize,
    pub attacks: usize,
    pub spells: usize,
    pub promotions: usize,
    pub revolutions: usize,
    pub wars: usize,
    pub end_turns: usize,
    pub forced_end_turns: usize,
}

/// The scripted human.
pub struct Bot {
    rng: Arc<Mutex<Rng>>,
    pub stats: BotStats,
    /// Production rotation position per city.
    rotation: BTreeMap<CityId, usize>,
    /// Game turn from which the bot declares war on a met AI (once).
    pub war_turn: i32,
    /// Civics seen available (a new one triggers a revolution).
    civics_seen: BTreeSet<CivicType>,
}

impl Bot {
    pub fn new(seed: u32) -> Bot {
        Bot {
            rng: Arc::new(Mutex::new(Rng::new(seed))),
            stats: BotStats::default(),
            rotation: BTreeMap::new(),
            war_turn: 80,
            civics_seen: BTreeSet::new(),
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

    /// Executes `cmd`; confirmations are accepted for the commands the bot means
    /// (a war move is declined). Returns whether it was applied.
    fn exec(&mut self, s: &mut Session, cmd: Command) -> bool {
        if !matches!(s.phase(), Phase::Human(_)) {
            return false;
        }
        self.stats.commands += 1;
        match s.execute(&cmd) {
            Ok(CmdOk::Applied { .. }) => true,
            Ok(CmdOk::NeedsConfirm(c)) => {
                self.stats.confirms += 1;
                let again = match (c, cmd) {
                    (Confirm::DeclareWar { .. }, Command::Move { units, x, y, append, .. }) => {
                        Command::Move { units, x, y, append, declare_war: Some(false) }
                    }
                    (Confirm::LoadUnit { .. }, _) => return false,
                    (Confirm::EndTurn { .. }, _) => Command::EndTurn { force: true },
                    (_, Command::DoCommand { units, command, data1, data2, alt, .. }) => {
                        Command::DoCommand { units, command, data1, data2, alt, confirmed: true }
                    }
                    _ => return false,
                };
                self.stats.commands += 1;
                match s.execute(&again) {
                    Ok(CmdOk::Applied { .. }) => true,
                    _ => {
                        self.stats.refused += 1;
                        false
                    }
                }
            }
            Err(_) => {
                self.stats.refused += 1;
                false
            }
        }
    }

    /// Plays the human's turn from a control point through `end_turn`.
    pub fn play(&mut self, s: &mut Session) {
        self.play_with(s, &mut |_| {});
    }

    /// [`Bot::play`], calling `mid` halfway (after the popups, production and empire commands,
    /// before the units move): tests save and reload the session there.
    pub fn play_with(&mut self, s: &mut Session, mid: &mut dyn FnMut(&mut Session)) {
        let h = s.human;
        let pending = s.pending();
        self.popups(s, &pending);
        self.cities(s, &pending);
        self.empire(s);
        mid(s);
        // Units: a few passes over the ready groups (an order can leave a group ready).
        for _ in 0..3 {
            let ready = s.pending().ready_groups;
            if ready.is_empty() {
                break;
            }
            for grp in ready {
                let still = s.read(|g| g.groups.contains(grp) && g.group_ready_to_move(grp, true));
                if !still {
                    continue;
                }
                let units = s.read(|g| g.groups[grp].units.clone());
                let order = self.order(s, &units);
                if !self.exec(s, order) {
                    self.exec(s, Command::Mission { units: units.clone(), mission: MissionType::MISSION_SKIP, data1: -1, data2: -1, flags: 0, append: false });
                }
            }
        }
        // Promotions.
        let ready: Vec<(UnitId, Option<PromotionType>)> = s.read(|g| {
            g.players[h]
                .units
                .iter()
                .filter(|&&u| g.units[u].promotion_ready)
                .map(|&u| (u, g.d.promotions.keys().find(|&p| g.unit_can_promote(u, Some(p), None))))
                .collect()
        });
        for (u, p) in ready {
            if let Some(p) = p {
                if self.exec(s, Command::DoCommand { units: vec![u], command: CommandType::COMMAND_PROMOTION, data1: p.idx() as i32, data2: -1, alt: false, confirmed: false }) {
                    self.stats.promotions += 1;
                }
            }
        }
        // End the turn.
        let force = self.rand(5) == 0;
        if self.exec(s, Command::EndTurn { force }) {
            self.stats.end_turns += 1;
            if force {
                self.stats.forced_end_turns += 1;
            }
        }
    }

    /// Events, votes, research.
    fn popups(&mut self, s: &mut Session, p: &Pending) {
        for e in &p.events {
            let options: Vec<EventType> = e.options.iter().filter(|o| o.enabled).map(|o| o.event).collect();
            if options.is_empty() {
                continue;
            }
            let event = options[self.rand(options.len())];
            if self.exec(s, Command::Event { triggered: e.triggered, event }) {
                self.stats.events_answered += 1;
            }
        }
        for v in &p.votes {
            let choice = v.choices[self.rand(v.choices.len())];
            if self.exec(s, Command::Vote { vote: v.vote, choice }) {
                self.stats.votes_answered += 1;
            }
        }
        if p.research_needed {
            let h = s.human;
            let tech = s.read(|g| {
                let team = g.players[h].team;
                g.d.techs.keys().filter(|&t| g.player_can_research(h, t, false)).min_by_key(|&t| (g.team_get_research_cost(team, t), t))
            });
            if let Some(tech) = tech {
                if self.exec(s, Command::Research { tech: Some(tech), shift: false }) {
                    self.stats.research_chosen += 1;
                }
            }
        }
    }

    /// Production for idle cities: the rotation warrior, settler, worker, building.
    fn cities(&mut self, s: &mut Session, p: &Pending) {
        let h = s.human;
        for &c in &p.idle_cities {
            let start = *self.rotation.get(&c).unwrap_or(&0);
            let choice = s.read(|g| {
                let d = g.d;
                let civ = g.player_get_civilization_type(h)?;
                let unit = |class: &str| d.ty::<UnitClassType>(class).and_then(|uc| d.civ_unit(civ, uc));
                // A city with fewer than two combat units first trains the strongest it can.
                let plot = g.city_plot(c);
                let guards = g.map.plots[plot].units.iter().filter(|&&u| g.units[u].owner == h && d.units[g.units[u].unit_type].combat > 0).count();
                if guards < 2 {
                    let best = d
                        .units
                        .keys()
                        .filter(|&u| d.units[u].combat > 0 && d.units[u].domain_type == Some(DomainType::DOMAIN_LAND))
                        .filter(|&u| g.city_can_train(c, u, false, false, false, false))
                        .max_by_key(|&u| (d.units[u].combat, std::cmp::Reverse(u)));
                    if let Some(u) = best {
                        return Some((start.wrapping_sub(1), OrderType::ORDER_TRAIN, u.idx() as i32));
                    }
                }
                let rotation = [unit("UNITCLASS_WARRIOR"), unit("UNITCLASS_SETTLER"), unit("UNITCLASS_WORKER")];
                for k in 0..4 {
                    let slot = (start + k) % 4;
                    if slot < 3 {
                        if let Some(u) = rotation[slot] {
                            if g.city_can_train(c, u, false, false, false, false) {
                                return Some((slot, OrderType::ORDER_TRAIN, u.idx() as i32));
                            }
                        }
                    } else if let Some(b) = d.buildings.keys().find(|&b| g.city_can_construct(c, b, false, false, false)) {
                        return Some((slot, OrderType::ORDER_CONSTRUCT, b.idx() as i32));
                    }
                }
                if let Some(u) = d.units.keys().find(|&u| g.city_can_train(c, u, false, false, false, false)) {
                    return Some((start, OrderType::ORDER_TRAIN, u.idx() as i32));
                }
                d.processes.keys().find(|&pr| g.city_can_maintain(c, pr, false)).map(|pr| (start, OrderType::ORDER_MAINTAIN, pr.idx() as i32))
            });
            if let Some((slot, order, data)) = choice {
                if self.exec(s, Command::PushOrder { city: c, order, data, alt: false, shift: false, ctrl: false }) {
                    self.stats.productions += 1;
                    self.rotation.insert(c, slot.wrapping_add(1));
                }
            }
        }
    }

    /// Civics, war.
    fn empire(&mut self, s: &mut Session) {
        let h = s.human;
        // Revolt when a civic unlocks (once per civic; the first ones are what we start with).
        let available: Vec<CivicType> = s.read(|g| g.d.civics.keys().filter(|&c| g.player_can_do_civics(h, Some(c))).collect());
        let first = self.civics_seen.is_empty();
        let new: Vec<CivicType> = available.iter().copied().filter(|c| !self.civics_seen.contains(c)).collect();
        self.civics_seen.extend(new.iter().copied());
        if !first {
            if let Some(&c) = new.first() {
                let current = s.read(|g| g.d.civics[c].civic_option_type.is_some_and(|o| g.players[h].civics[o] == Some(c)));
                if !current && self.exec(s, Command::UpdateCivics { civics: vec![c] }) {
                    self.stats.revolutions += 1;
                }
            }
        }
        if self.stats.wars == 0 && s.g.game_turn() >= self.war_turn {
            let target = s.read(|g| {
                let team = g.players[h].team;
                g.civ_team_ids().find(|&t| t != team && g.team_can_declare_war(team, t))
            });
            if let Some(t) = target {
                if self.exec(s, Command::DeclareWar { team: t }) {
                    self.stats.wars += 1;
                }
            }
        }
    }

    /// An order for a ready group (its head decides).
    fn order(&mut self, s: &Session, units: &[UnitId]) -> Command {
        let h = s.human;
        let roll = self.rand(100);
        let pick = self.rand(1 << 16);
        let mission = |m: MissionType, d1: i32, d2: i32| Command::Mission { units: units.to_vec(), mission: m, data1: d1, data2: d2, flags: 0, append: false };
        let head = units[0];
        let decided = s.read(|g| -> Option<Command> {
            let d = g.d;
            let info = &d.units[g.units[head].unit_type];
            let plot = g.unit_plot(head);
            let (x, y) = (g.units[head].x, g.units[head].y);
            let near = |r: i32| -> Vec<PlotIdx> {
                let mut v = Vec::new();
                for dy in -r..=r {
                    for dx in -r..=r {
                        if let Some(p) = g.plot_xy(x, y, dx, dy) {
                            if p != plot {
                                v.push(p);
                            }
                        }
                    }
                }
                v
            };
            let xy = |p: PlotIdx| (g.map.plots[p].x, g.map.plots[p].y);
            // Settlers.
            if info.found {
                if g.unit_can_found(head, plot, false) {
                    return Some(mission(MissionType::MISSION_FOUND, -1, -1));
                }
                let sites: Vec<PlotIdx> = near(4).into_iter().filter(|&p| {
                    let (px, py) = xy(p);
                    g.player_can_found(h, px, py, false)
                }).collect();
                if !sites.is_empty() {
                    let (px, py) = xy(sites[pick % sites.len()]);
                    return Some(Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war: None });
                }
            }
            // Workers: the first build here, else go where one is possible.
            if let Some(b) = d.builds.keys().find(|&b| g.unit_can_build(head, plot, b, false)) {
                return Some(mission(MissionType::MISSION_BUILD, b.idx() as i32, -1));
            }
            if info.builds.iter().any(|&b| b) {
                let spots: Vec<PlotIdx> = near(3)
                    .into_iter()
                    .filter(|&p| g.map.plots[p].owner == Some(h) && d.builds.keys().any(|b| g.unit_can_build(head, p, b, false)))
                    .collect();
                if !spots.is_empty() {
                    let (px, py) = xy(spots[pick % spots.len()]);
                    return Some(Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war: None });
                }
                return None;
            }
            if info.combat <= 0 {
                return None;
            }
            // Attack an adjacent enemy at good odds.
            if g.unit_can_attack(head) && g.unit_moves_left(head) > 0 {
                for p in near(1) {
                    if let Some(def) = g.plot_get_best_defender(p, None, Some(h), Some(head), true, false, false) {
                        if g.get_combat_odds(head, def) >= 600 && g.unit_can_move_or_attack_into(head, p, false) {
                            let (px, py) = xy(p);
                            return Some(Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war: Some(false) });
                        }
                    }
                }
            }
            // A spell now and then.
            if roll < 5 {
                if let Some(sp) = d.spells.keys().find(|&sp| g.unit_can_cast(head, sp, false)) {
                    return Some(Command::DoCommand { units: units.to_vec(), command: CommandType::COMMAND_CAST, data1: sp.idx() as i32, data2: -1, alt: false, confirmed: true });
                }
            }
            // Defend cities (two combat units each), else explore.
            let grp = g.units[head].group;
            let guards = |p: PlotIdx| {
                g.map.plots[p].units.iter().filter(|&&u| g.units[u].owner == h && g.units[u].group != grp && d.units[g.units[u].unit_type].combat > 0).count()
            };
            if g.map.plots[plot].plot_city.is_some() && guards(plot) < 2 && g.unit_can_fortify(head, plot) {
                return Some(mission(MissionType::MISSION_FORTIFY, -1, -1));
            }
            let weak = g.players[h].cities.iter().map(|&c| g.city_plot(c)).filter(|&p| p != plot && guards(p) < 2).min_by_key(|&p| {
                let (px, py) = xy(p);
                (g.plot_distance(x, y, px, py), p)
            });
            if let Some(p) = weak {
                let (px, py) = xy(p);
                return Some(Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war: None });
            }
            // Spare units go for the nearest known enemy city (we are at war with its owner).
            let team = g.players[h].team;
            let target = g
                .cities
                .iter()
                .filter(|(_, city)| g.teams[team].at_war[g.players[city.owner].team] && g.map.plots[g.city_plot(city.id)].revealed[team])
                .map(|(_, city)| (g.plot_distance(x, y, city.x, city.y), city.x, city.y))
                .filter(|&(dist, _, _)| dist <= 8)
                .min();
            if let (Some((_, cx, cy)), true) = (target, roll < 60) {
                return Some(Command::Move { units: units.to_vec(), x: cx, y: cy, append: false, declare_war: Some(false) });
            }
            if roll < 70 {
                let around: Vec<PlotIdx> = near(2).into_iter().filter(|&p| !g.plot_is_water(p) && g.map.plots[p].revealed[g.players[h].team]).collect();
                if !around.is_empty() {
                    let (px, py) = xy(around[pick % around.len()]);
                    return Some(Command::Move { units: units.to_vec(), x: px, y: py, append: false, declare_war: None });
                }
            }
            if g.unit_can_fortify(head, plot) {
                return Some(mission(MissionType::MISSION_FORTIFY, -1, -1));
            }
            None
        });
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
            None => mission(MissionType::MISSION_SKIP, -1, -1),
        }
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
