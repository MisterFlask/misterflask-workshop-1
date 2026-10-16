//! Goal-directed research: an ordered list of research goals adapted from the tech beelines
//! of FfH's own AI (`CvGameUtils.AI_chooseTech`), for the research scorer (`research::choose`,
//! which gives the first two a bonus along their prerequisite paths).
//!
//! [`goals`] lists the techs this player should be heading for, most urgent first ([`ORDER`]),
//! leaving out the techs we know and those we can never research:
//! 1. an early tier-2 military tech until one of Bronze Working, Archery, Hunting or Horseback
//!    Riding is known: the first we do not know and can ever research of the leader's favorite
//!    early military tech, the civ's (Hunting for the elves, Construction for the Luchuirp)
//!    and Bronze Working;
//! 2. the early economy: Exploration, Mining, Calendar, Ancient Chants, Education;
//! 3. an early religion nobody has founded yet, unless we know one of the three early religion
//!    techs or have a holy city ([`religion_preferences`]: the Khazad and elves' own, the
//!    leader's favorite early religion, the favorite religion, the alignment, then a fixed
//!    fallback order); agnostics take Sanitation instead;
//! 4. Code of Laws once the empire has 4+ cities or maintenance eats a fifth of its commerce,
//!    and Writing (not for TRAIT_BARBARIAN leaders);
//! 5. Mysticism, Festivals, Cartography (City States), Sailing for coastal empires;
//! 6. economy tier 2: Trade, Sanitation, Currency, Construction, Taxation, while fewer are known
//!    than `AI_chooseTech`'s "economy tier 2" needs (the rest come after step 7);
//! 7. the per-civ beelines of `AI_chooseTech`;
//! 8. the magic spheres (Necromancy, Elementalism, Divination, Alteration) once tier 3 military
//!    and economy tier 2 are reached.
//!
//! This is not `AI_chooseTech`'s exact order. That function returns a single tech: blocks gated
//! by its tiers assign it in turn and the last assignment wins, which makes its precedence the
//! tier-2 military tech, the early economy, the early religion (once tier 2 and the early
//! economy are known), Cartography, Festivals, Mysticism, Writing, Animal Husbandry, Code of
//! Laws (TRAIT_FINANCIAL with 6+ cities), Archery, Sailing, the civ's early beelines, the leader's
//! favorite early techs and early wonder, then economy tier 2 (Trade, Sanitation, Construction,
//! Currency, Taxation), the civ beelines, and the tier-3 block (magic, victory techs, favorite
//! wonders and heroes). The list keeps the main precedences (military, early economy, religion,
//! the other early techs, economy tier 2, civ beelines, magic) but has no tier gates (every
//! unmet goal is listed), adds Ancient Chants and a Code of Laws condition of its own, takes
//! Writing to Cartography in the reverse order, and leaves out Archery and Animal Husbandry,
//! the leader's early techs and wonders, the starting-resource techs and the tier-3 extras.
//!
//! A pure function of the game state, recomputed on every call (no cache); techs are resolved
//! by name once per call.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// One research goal.
#[derive(Clone, Copy, Debug)]
pub struct Goal {
    pub tech: TechType,
    /// The `AI_chooseTech` stage it comes from (for traces).
    pub stage: &'static str,
}

/// The unknown techs needed to reach a goal (the goal included; the cheapest branch of each OR
/// prerequisite), and their research left in total.
#[derive(Clone, Debug)]
pub struct Path {
    pub techs: Vec<TechType>,
    pub left: i64,
}

/// Techs and other types looked up by name for one call.
struct Names<'a> {
    g: &'a Game,
    team: TeamId,
}

impl Names<'_> {
    fn tech(&self, name: &str) -> Option<TechType> {
        self.g.d.ty::<TechType>(name)
    }

    fn has(&self, name: &str) -> bool {
        self.tech(name).is_some_and(|t| self.g.team_is_has_tech(self.team, Some(t)))
    }
}

/// A step of the plan.
enum Step {
    /// The early tier-2 military tech, until tier 2 is reached.
    Military,
    /// This tech.
    Tech(&'static str),
    /// The early religion (Sanitation for agnostics).
    Religion,
    /// Code of Laws once maintenance matters.
    CodeOfLaws,
    /// Writing, except for TRAIT_BARBARIAN leaders.
    Writing,
    /// Sailing for empires with a coastal city.
    Sailing,
    /// Economy tier 2 until it is reached.
    Economy2,
    /// The civ's own beelines.
    Civ,
    /// Economy tier 2 techs still missing.
    Economy2Rest,
    /// The magic techs, once tier 3 military and economy tier 2 are reached.
    Magic,
}

/// The plan, most urgent first.
const ORDER: &[Step] = &[
    Step::Military,
    Step::Tech("TECH_EXPLORATION"),
    Step::Tech("TECH_MINING"),
    Step::Tech("TECH_CALENDAR"),
    Step::Tech("TECH_ANCIENT_CHANTS"),
    Step::Tech("TECH_EDUCATION"),
    Step::Religion,
    Step::CodeOfLaws,
    Step::Writing,
    Step::Tech("TECH_MYSTICISM"),
    Step::Tech("TECH_FESTIVALS"),
    Step::Tech("TECH_CARTOGRAPHY"),
    Step::Sailing,
    Step::Economy2,
    Step::Civ,
    Step::Economy2Rest,
    Step::Magic,
];

/// AI_chooseTech's economy tier 2 techs.
const ECONOMY2: [&str; 5] = ["TECH_TRADE", "TECH_SANITATION", "TECH_CURRENCY", "TECH_CONSTRUCTION", "TECH_TAXATION"];

/// The ordered research goals of `p` (known techs and techs `p` can never research left out).
/// The barbarians have none: their research stays with the plain score (their units).
pub fn goals(g: &Game, p: PlayerId) -> Vec<Goal> {
    if g.player_is_barbarian(p) {
        return Vec::new();
    }
    let d = g.d;
    let pl = &g.players[p];
    let team = pl.team;
    let n = Names { g, team };
    let civ = g.player_get_civilization_type(p);
    let is_civ = |name: &str| civ.is_some() && civ == d.ty::<CivilizationType>(name);
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);
    let has_trait = |name: &str| d.ty::<TraitType>(name).is_some_and(|tr| pl.traits[tr]);

    // AI_chooseTech's tiers.
    let tier2 = ["TECH_BRONZE_WORKING", "TECH_ARCHERY", "TECH_HUNTING", "TECH_HORSEBACK_RIDING"].iter().any(|t| n.has(t))
        || (is_civ("CIVILIZATION_LUCHUIRP") && n.has("TECH_CONSTRUCTION"));
    let tier3 = ["TECH_IRON_WORKING", "TECH_BOWYERS", "TECH_STIRRUPS", "TECH_ANIMAL_HANDLING"].iter().any(|t| n.has(t));
    let econ_count = ECONOMY2.iter().filter(|t| n.has(t)).count() as i32;
    let econ_val = leader.map_or(100, |l| l.economy_tech_value).min(130);
    let econ2 = econ_count * 95 + 150 > 4 * econ_val;

    let mut out: Vec<Goal> = Vec::new();
    let mut push = |name: &str, stage: &'static str| {
        if let Some(t) = n.tech(name) {
            if !g.team_is_has_tech(team, Some(t)) && g.player_can_ever_research(p, t) && !out.iter().any(|o| o.tech == t) {
                out.push(Goal { tech: t, stage });
            }
        }
    };
    for step in ORDER {
        match step {
            Step::Military => {
                if !tier2 {
                    // The leader's favorite (some are no military tech: Festivals, Sailing, Way
                    // of the Earthmother), else the civ's, else Bronze Working: the first we do
                    // not know and can ever research (the Earthmother is closed to agnostics and
                    // with its religion off).
                    let fav = leader.and_then(|l| l.favorite_early_mil_tech).map(|t| d.techs[t].base.type_.as_str());
                    let civ_default = if is_civ("CIVILIZATION_LJOSALFAR") || is_civ("CIVILIZATION_SVARTALFAR") {
                        Some("TECH_HUNTING")
                    } else if is_civ("CIVILIZATION_LUCHUIRP") {
                        Some("TECH_CONSTRUCTION")
                    } else {
                        None
                    };
                    let open = |name: &str| n.tech(name).is_some_and(|t| !g.team_is_has_tech(team, Some(t)) && g.player_can_ever_research(p, t));
                    if let Some(name) = [fav, civ_default, Some("TECH_BRONZE_WORKING")].into_iter().flatten().find(|&name| open(name)) {
                        push(name, "tier2-military");
                    }
                }
            }
            Step::Tech(name) => push(name, "economy"),
            Step::Religion => {
                if pl.agnostic || has_trait("TRAIT_AGNOSTIC") {
                    push("TECH_SANITATION", "agnostic-happiness");
                } else if let Some(t) = early_religion_tech(g, p, &n) {
                    push(&d.techs[t].base.type_, "early-religion");
                }
            }
            Step::CodeOfLaws => {
                let commerce = g.player_get_commerce_rate(p, CommerceType::COMMERCE_GOLD) + g.player_get_commerce_rate(p, CommerceType::COMMERCE_RESEARCH);
                if pl.cities.len() >= 4 || g.player_get_total_maintenance(p) * 5 > commerce.max(1) {
                    push("TECH_CODE_OF_LAWS", "economy");
                }
            }
            Step::Writing => {
                if !has_trait("TRAIT_BARBARIAN") {
                    push("TECH_WRITING", "economy");
                }
            }
            Step::Sailing => {
                if g.player_count_num_coastal_cities(p) > 0 {
                    push("TECH_SAILING", "economy");
                }
            }
            Step::Economy2 | Step::Economy2Rest => {
                if !econ2 || matches!(step, Step::Economy2Rest) {
                    for t in ECONOMY2 {
                        push(t, "economy-2");
                    }
                }
            }
            Step::Civ => {
                for t in civ_beelines(&n, &is_civ) {
                    push(t, "civ");
                }
            }
            Step::Magic => {
                if tier3 && econ2 {
                    for t in ["TECH_NECROMANCY", "TECH_ELEMENTALISM", "TECH_DIVINATION", "TECH_ALTERATION"] {
                        push(t, "magic");
                    }
                }
            }
        }
    }
    out
}

/// The three early religion techs, in the fixed fallback order of [`religion_preferences`].
const EARLY_RELIGION_TECHS: [&str; 3] = ["TECH_WAY_OF_THE_EARTHMOTHER", "TECH_WAY_OF_THE_FORESTS", "TECH_MESSAGE_FROM_THE_DEEP"];

/// AI_chooseTech "Early Religion Beeline": the religion tech we head for, if we know none of the
/// three early ones and have no holy city: the first of [`religion_preferences`] whose religion
/// nobody has founded yet and that we can ever research.
fn early_religion_tech(g: &Game, p: PlayerId, n: &Names) -> Option<TechType> {
    if EARLY_RELIGION_TECHS.iter().any(|t| n.has(t)) || g.d.religions.keys().any(|r| g.player_has_holy_city(p, r)) {
        return None;
    }
    let d = g.d;
    let unfounded = |t: TechType| d.religions.keys().any(|r| d.religions[r].tech_prereq == Some(t) && g.religion_game_turn_founded[r] == -1);
    religion_preferences(g, p, n).into_iter().find(|&t| unfounded(t) && g.player_can_ever_research(p, t))
}

/// The religion techs `p` prefers, most preferred first: Way of the Earthmother for the Khazad
/// and Way of the Forests for the elves, the founding tech of the leader's favorite early
/// religion, the early tech of the favorite religion (Fellowship of Leaves, Runes of Kilmorph,
/// Octopus Overlords), the alignment's (good: Earthmother, neutral: Forests, evil: Message from
/// the Deep); then the three early ones in a fixed order ([`EARLY_RELIGION_TECHS`]), so that a
/// religion someone else founded falls back to a free one. `AI_chooseTech` assigns the same
/// overrides in the reverse order (the last assignment wins) and has no fallback.
fn religion_preferences(g: &Game, p: PlayerId, n: &Names) -> Vec<TechType> {
    let d = g.d;
    let pl = &g.players[p];
    let team = pl.team;
    let civ = g.player_get_civilization_type(p);
    let is_civ = |name: &str| civ.is_some() && civ == d.ty::<CivilizationType>(name);
    let leader = pl.personality_type.map(|l| &d.leader_heads[l]);
    let mut prefs: Vec<TechType> = Vec::new();
    // Civ lore religions first (the dwarves' Runes of Kilmorph, the elves' Fellowship of Leaves).
    if is_civ("CIVILIZATION_KHAZAD") {
        prefs.extend(n.tech("TECH_WAY_OF_THE_EARTHMOTHER"));
    }
    if is_civ("CIVILIZATION_LJOSALFAR") || is_civ("CIVILIZATION_SVARTALFAR") {
        prefs.extend(n.tech("TECH_WAY_OF_THE_FORESTS"));
    }
    // The leader's favourite religions, when the data sets them (FfH leaves these NONE by default).
    if let Some(r) = leader.and_then(|l| l.favorite_early_religion) {
        prefs.extend(d.religions[r].tech_prereq);
    }
    if let Some(r) = pl.favorite_religion.or(leader.and_then(|l| l.favorite_religion)) {
        prefs.extend(d.religions[r].tech_prereq);
    }
    // Data-driven alignment match: the religions whose own alignment (or best alignment) fits
    // ours, cheapest to found first -- so a good civ still grabs the cheap Runes of Kilmorph early
    // rather than beelining a deeper aligned faith, while the deeper aligned religions (The Order,
    // the Ashen Veil) follow as fallbacks once the cheap one is taken. Reads each religion's
    // ALIGNMENT from the data, so it adapts if the religions or their alignments change -- no
    // hardcoded good->Earthmother / evil->Message-from-the-Deep map.
    let align = pl.alignment;
    if align.is_some() {
        let mut aligned: Vec<(i32, TechType)> = Vec::new();
        for r in d.religions.keys() {
            let Some(t) = d.religions[r].tech_prereq else { continue };
            if g.player_can_ever_research(p, t) && (d.religions[r].alignment == align || d.religions[r].alignment_best == align) {
                aligned.push((g.team_get_research_left(team, t), t));
            }
        }
        aligned.sort();
        prefs.extend(aligned.into_iter().map(|(_, t)| t));
    }
    // Fallback: the cheap early religions, so the civ founds something even with no alignment.
    for name in EARLY_RELIGION_TECHS {
        prefs.extend(n.tech(name));
    }
    // Keep the first (most preferred) occurrence of each tech.
    let mut seen: Vec<TechType> = Vec::new();
    prefs.retain(|&t| !seen.contains(&t) && { seen.push(t); true });
    prefs
}

/// AI_chooseTech "Civ Specific" beelines (and the Luchuirp's early Construction).
fn civ_beelines(n: &Names, is_civ: &dyn Fn(&str) -> bool) -> Vec<&'static str> {
    let mut v: Vec<&'static str> = Vec::new();
    if is_civ("CIVILIZATION_AMURITES") {
        v.extend(["TECH_BOWYERS", "TECH_SORCERY"]);
    }
    if is_civ("CIVILIZATION_BANNOR") {
        v.push("TECH_FANATICISM");
    }
    if is_civ("CIVILIZATION_CALABIM") {
        v.push("TECH_FEUDALISM");
    }
    if is_civ("CIVILIZATION_HIPPUS") || is_civ("CIVILIZATION_KURIOTATES") {
        v.push("TECH_STIRRUPS");
    }
    if is_civ("CIVILIZATION_SHEAIM") {
        v.extend(["TECH_CORRUPTION_OF_SPIRIT", "TECH_NECROMANCY", "TECH_SORCERY"]);
    }
    if is_civ("CIVILIZATION_SIDAR") || is_civ("CIVILIZATION_SVARTALFAR") {
        v.extend(["TECH_POISONS", "TECH_ANIMAL_HANDLING"]);
    }
    if is_civ("CIVILIZATION_LJOSALFAR") {
        v.push("TECH_BOWYERS");
    }
    if is_civ("CIVILIZATION_LUCHUIRP") {
        v.push("TECH_CONSTRUCTION");
        if n.has("TECH_TRADE") {
            v.extend(["TECH_ELEMENTALISM", "TECH_SORCERY"]);
        }
    }
    if is_civ("CIVILIZATION_ELOHIM") || is_civ("CIVILIZATION_MALAKIM") {
        v.push("TECH_PRIESTHOOD");
    }
    if ["CIVILIZATION_KHAZAD", "CIVILIZATION_CLAN_OF_EMBERS", "CIVILIZATION_DOVIELLO", "CIVILIZATION_LANUN", "CIVILIZATION_GRIGORI", "CIVILIZATION_ILLIANS", "CIVILIZATION_BALSERAPHS"]
        .iter()
        .any(|c| is_civ(c))
    {
        v.push("TECH_IRON_WORKING");
    }
    if is_civ("CIVILIZATION_GRIGORI") {
        v.push("TECH_MEDICINE");
    }
    v
}

/// The unknown techs `p` needs for `goal` and their research left, or `None` if some needed
/// tech can never be researched.
pub fn path(g: &Game, p: PlayerId, goal: TechType) -> Option<Path> {
    let d = g.d;
    let team = g.players[p].team;
    // Cost of reaching each tech alone (memoized; i64::MAX = unreachable).
    let mut memo: Vec<Option<i64>> = vec![None; d.techs.len()];
    fn cost(g: &Game, p: PlayerId, team: TeamId, t: TechType, memo: &mut Vec<Option<i64>>, depth: u32) -> i64 {
        if g.team_is_has_tech(team, Some(t)) {
            return 0;
        }
        if let Some(c) = memo[t.idx()] {
            return c;
        }
        if depth > 32 || !g.player_can_ever_research(p, t) {
            memo[t.idx()] = Some(i64::MAX);
            return i64::MAX;
        }
        let info = &g.d.techs[t];
        let mut total = g.team_get_research_left(team, t).max(1) as i64;
        for &a in &info.prereq_and_techs {
            let c = cost(g, p, team, a, memo, depth + 1);
            if c == i64::MAX {
                memo[t.idx()] = Some(i64::MAX);
                return i64::MAX;
            }
            total += c;
        }
        if !info.prereq_or_techs.is_empty() {
            let best = info.prereq_or_techs.iter().map(|&o| cost(g, p, team, o, memo, depth + 1)).min().unwrap_or(0);
            if best == i64::MAX {
                memo[t.idx()] = Some(i64::MAX);
                return i64::MAX;
            }
            total += best;
        }
        memo[t.idx()] = Some(total);
        total
    }
    if cost(g, p, team, goal, &mut memo, 0) == i64::MAX {
        return None;
    }
    // Collect the techs (shared prerequisites once).
    let mut techs: Vec<TechType> = Vec::new();
    let mut stack = vec![goal];
    while let Some(t) = stack.pop() {
        if g.team_is_has_tech(team, Some(t)) || techs.contains(&t) {
            continue;
        }
        techs.push(t);
        let info = &d.techs[t];
        stack.extend(info.prereq_and_techs.iter().copied());
        if !info.prereq_or_techs.is_empty() && !info.prereq_or_techs.iter().any(|&o| g.team_is_has_tech(team, Some(o))) {
            // Cheapest branch; ties go to the first listed.
            let mut best: Option<(i64, TechType)> = None;
            for &o in &info.prereq_or_techs {
                let c = cost(g, p, team, o, &mut memo, 0);
                if best.map_or(true, |(bc, _)| c < bc) {
                    best = Some((c, o));
                }
            }
            if let Some((_, o)) = best {
                stack.push(o);
            }
        }
    }
    let left = techs.iter().map(|&t| g.team_get_research_left(team, t).max(1) as i64).sum();
    Some(Path { techs, left })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ffh_core::setup::GameSetup;

    #[test]
    fn early_goals_and_paths() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_TINY", 3));
        g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
        g.game_init();
        let p = PlayerId(0);
        let team = g.players[p].team;
        let t = |name: &str| d.t::<TechType>(name);
        assert!(goals(&g, g.barbarian_player()).is_empty());
        let goals = goals(&g, p);
        assert!(!goals.is_empty());
        assert!(goals.iter().all(|gl| !g.team_is_has_tech(team, Some(gl.tech))));
        // The early economy is planned, Education after the Ancient Chants it needs.
        let pos = |tech: TechType| goals.iter().position(|gl| gl.tech == tech);
        if !g.team_is_has_tech(team, Some(t("TECH_EDUCATION"))) {
            assert!(pos(t("TECH_EDUCATION")).is_some());
            let path = path(&g, p, t("TECH_EDUCATION")).unwrap();
            assert_eq!(path.techs[0], t("TECH_EDUCATION"));
            assert!(g.team_is_has_tech(team, Some(t("TECH_ANCIENT_CHANTS"))) || path.techs.contains(&t("TECH_ANCIENT_CHANTS")));
            let sum: i64 = path.techs.iter().map(|&x| g.team_get_research_left(team, x) as i64).sum();
            assert_eq!(path.left, sum);
        }
        // Taxation needs Currency and Mathematics, both behind Education: counted once.
        let tax = path(&g, p, t("TECH_TAXATION")).unwrap();
        let mut sorted = tax.techs.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(sorted.len(), tax.techs.len());
        assert!(tax.techs.contains(&t("TECH_CURRENCY")) && tax.techs.contains(&t("TECH_MATHEMATICS")));
        // Known techs have no path left.
        let known = d.techs.keys().find(|&x| g.team_is_has_tech(team, Some(x)));
        if let Some(k) = known {
            assert!(path(&g, p, k).unwrap().techs.is_empty());
        }
    }

    /// A player whose civilization has no religion or military override (not the Khazad, the
    /// elves or the Luchuirp), who can research the three early religion techs and knows none.
    fn plain_player(g: &Game) -> PlayerId {
        let d = g.d;
        let early: Vec<TechType> = EARLY_RELIGION_TECHS.iter().map(|n| d.t::<TechType>(n)).collect();
        let special = ["CIVILIZATION_KHAZAD", "CIVILIZATION_LJOSALFAR", "CIVILIZATION_SVARTALFAR", "CIVILIZATION_LUCHUIRP"];
        g.player_ids()
            .filter(|&p| g.players[p].alive && !g.player_is_barbarian(p))
            .find(|&p| {
                let civ = g.player_get_civilization_type(p);
                !special.iter().any(|c| civ == d.ty::<CivilizationType>(c))
                    && early.iter().all(|&t| g.player_can_ever_research(p, t) && !g.team_is_has_tech(g.players[p].team, Some(t)))
                    && !d.religions.keys().any(|r| g.player_has_holy_city(p, r))
            })
            .expect("a player without religion overrides")
    }

    // Invariants of the data-driven religion choice (not a fixed preference vector, which would
    // break whenever religions or their alignments change): a civ heads for the cheapest unfounded
    // religion whose own alignment fits its own, a favourite/leader preference wins over that, and
    // founding the pick advances the fallback until nothing is left.
    #[test]
    fn early_religion_preference_order() {
        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 6, "WORLDSIZE_SMALL", 5));
        g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
        g.game_init();
        let p = plain_player(&g);
        let team = g.players[p].team;
        let pick = |g: &Game| early_religion_tech(g, p, &Names { g, team });
        let align = |name: &str| Some(d.t::<AlignmentType>(name));
        let religion_of = |tech: TechType| d.religions.keys().find(|&r| d.religions[r].tech_prereq == Some(tech)).unwrap();
        let fits = |tech: TechType, al: Option<AlignmentType>| {
            let r = religion_of(tech);
            d.religions[r].alignment == al || d.religions[r].alignment_best == al
        };
        // The cheapest unfounded, researchable religion whose alignment fits `al`.
        let cheapest_fit = |g: &Game, al: Option<AlignmentType>| -> Option<TechType> {
            d.religions
                .keys()
                .filter_map(|r| d.religions[r].tech_prereq)
                .filter(|&t| g.player_can_ever_research(p, t) && g.religion_game_turn_founded[religion_of(t)] == -1 && fits(t, al))
                .min_by_key(|&t| (g.team_get_research_left(team, t), t))
        };
        g.players[p].personality_type = None;
        g.players[p].favorite_religion = None;
        // Alignment decides: it heads for the cheapest unfounded religion that fits its alignment,
        // and that religion is alignment-appropriate (read from the data, no hardcoded name).
        for al in ["ALIGNMENT_GOOD", "ALIGNMENT_NEUTRAL", "ALIGNMENT_EVIL"] {
            g.players[p].alignment = align(al);
            let got = pick(&g).expect("an aligned religion to head for");
            assert_eq!(Some(got), cheapest_fit(&g, align(al)), "{al}: the cheapest fitting religion");
            assert!(fits(got, align(al)), "{al}: the religion it heads for fits its alignment");
        }
        // The favourite religion is preferred over the alignment default.
        g.players[p].alignment = align("ALIGNMENT_EVIL");
        let fav = d.t::<ReligionType>("RELIGION_FELLOWSHIP_OF_LEAVES");
        g.players[p].favorite_religion = Some(fav);
        assert_eq!(pick(&g), d.religions[fav].tech_prereq, "the favourite religion wins over the alignment");
        g.players[p].favorite_religion = None;
        // A leader's favourite early religion is preferred over the alignment default.
        let deep_r = d.t::<ReligionType>("RELIGION_OCTOPUS_OVERLORDS");
        if let Some(l) = d.leader_heads.keys().find(|&l| d.leader_heads[l].favorite_early_religion == Some(deep_r) && d.leader_heads[l].favorite_religion.is_none()) {
            g.players[p].personality_type = Some(l);
            g.players[p].alignment = align("ALIGNMENT_GOOD");
            assert_eq!(pick(&g), d.religions[deep_r].tech_prereq, "the leader's favourite early religion wins");
            g.players[p].personality_type = None;
        }
        // Founding the current pick advances the fallback; eventually nothing is left to found.
        g.players[p].alignment = align("ALIGNMENT_EVIL");
        let mut prev = None;
        for _ in 0..d.religions.len() + 1 {
            let Some(t) = pick(&g) else { break };
            assert_ne!(Some(t), prev, "the fallback advances to a different religion");
            prev = Some(t);
            g.religion_game_turn_founded[religion_of(t)] = 1;
        }
        assert_eq!(pick(&g), None, "with every religion founded there is nothing to head for");
    }

    #[test]
    fn military_goal_falls_back_to_bronze_working() {
        let d = ffh_data::global();
        // Runes of Kilmorph off: Way of the Earthmother can never be researched.
        let mut opts = ffh_core::setup::SetupOptions::new(d, 6, "WORLDSIZE_SMALL", 5);
        opts.options.push(d.t::<GameOptionType>("GAMEOPTION_NO_RELIGION_3"));
        let mut g = Game::new(d, GameSetup::from_options(d, &opts));
        g.map_script = std::sync::Arc::new(ffh_core::mapscript::FractalMapScript::default());
        g.game_init();
        let t = |name: &str| d.t::<TechType>(name);
        let earth = t("TECH_WAY_OF_THE_EARTHMOTHER");
        let p = g
            .player_ids()
            .filter(|&p| g.players[p].alive && !g.player_is_barbarian(p))
            .find(|&p| {
                let civ = g.player_get_civilization_type(p);
                !["CIVILIZATION_LJOSALFAR", "CIVILIZATION_SVARTALFAR", "CIVILIZATION_LUCHUIRP"].iter().any(|c| civ == d.ty::<CivilizationType>(c))
                    && !["TECH_BRONZE_WORKING", "TECH_ARCHERY", "TECH_HUNTING", "TECH_HORSEBACK_RIDING"].iter().any(|&x| g.team_is_has_tech(g.players[p].team, Some(t(x))))
            })
            .expect("a civ without the elves' or the Luchuirp's military default, before tier 2");
        assert!(!g.player_can_ever_research(p, earth));
        let first = |g: &Game| goals(g, p).into_iter().find(|gl| gl.stage == "tier2-military").map(|gl| gl.tech);
        // A leader whose favorite early military tech is Way of the Earthmother.
        let l = d.leader_heads.keys().find(|&l| d.leader_heads[l].favorite_early_mil_tech == Some(earth)).expect("a leader favoring Way of the Earthmother");
        g.players[p].personality_type = Some(l);
        assert_eq!(first(&g), Some(t("TECH_BRONZE_WORKING")));
        // A favorite that can be researched stays the goal.
        let archery = d.leader_heads.keys().find(|&l| d.leader_heads[l].favorite_early_mil_tech == Some(t("TECH_ARCHERY"))).expect("a leader favoring Archery");
        g.players[p].personality_type = Some(archery);
        assert_eq!(first(&g), Some(t("TECH_ARCHERY")));
    }
}
