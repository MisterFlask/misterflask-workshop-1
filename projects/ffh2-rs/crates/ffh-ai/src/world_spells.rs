//! When the AI casts a world spell (`bGlobal`: once a game, by any unit of the civilization).
//! FfH 0.41 wrote this timing into the rules, as AI-only branches of the spells' Python
//! requirements (`if pPlayer.isHuman() == False:` in `CvSpellInterface.py` `req*`); the port
//! dropped them from the rules (docs/PORTING.md, "Changes from FFH2"), so a world spell has the
//! same requirements for every player and the AI decides when. By default it is FfH's condition
//! ([`ready`]): `casting::choose_spell` (the end-of-turn spell choice) and the civilizations' held
//! casts (`civs/world_spell.rs`) cast a world spell only once it holds, and a civilization can
//! wait longer through its `spell_value` (`civs/`). A few spells have a moment of their own
//! instead ([`turn`], once a turn before units move), for effects FfH's condition almost never
//! allows or allows late; `FFH_AI_WS_DEFAULT=SPELL_X,...` (or `all`) gives the listed spells
//! FfH's condition back, the measurement switch (read once; the run cache keys runs by every
//! `FFH_*` variable).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::view::View;

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
enum World {
    ArcaneLacuna,
    Ardor,
    DivineRetribution,
    ForTheHorde,
    GiftsOfNantosuelta,
    IntoTheMist,
    Legends,
    MarchOfTheTrees,
    MotherLode,
    RagingSeas,
    Rally,
    ReligiousFervor,
    Revelry,
    RiverOfBlood,
    Sanctuary,
    Stasis,
    VeilOfNight,
    Warcry,
    WildHunt,
    Worldbreak,
}

const NAMES: [(&str, World); 20] = [
    ("SPELL_ARCANE_LACUNA", World::ArcaneLacuna),
    ("SPELL_ARDOR", World::Ardor),
    ("SPELL_DIVINE_RETRIBUTION", World::DivineRetribution),
    ("SPELL_FOR_THE_HORDE", World::ForTheHorde),
    ("SPELL_GIFTS_OF_NANTOSUELTA", World::GiftsOfNantosuelta),
    ("SPELL_INTO_THE_MIST", World::IntoTheMist),
    ("SPELL_LEGENDS", World::Legends),
    ("SPELL_MARCH_OF_THE_TREES", World::MarchOfTheTrees),
    ("SPELL_MOTHER_LODE", World::MotherLode),
    ("SPELL_RAGING_SEAS", World::RagingSeas),
    ("SPELL_RALLY", World::Rally),
    ("SPELL_RELIGIOUS_FERVOR", World::ReligiousFervor),
    ("SPELL_REVELRY", World::Revelry),
    ("SPELL_RIVER_OF_BLOOD", World::RiverOfBlood),
    ("SPELL_SANCTUARY", World::Sanctuary),
    ("SPELL_STASIS", World::Stasis),
    ("SPELL_VEIL_OF_NIGHT", World::VeilOfNight),
    ("SPELL_WARCRY", World::Warcry),
    ("SPELL_WILD_HUNT", World::WildHunt),
    ("SPELL_WORLDBREAK", World::Worldbreak),
];

/// The typed mana bonuses `reqArcaneLacuna` counts on the map (raw mana excluded).
const MANA_TYPES: [&str; 17] = [
    "BONUS_MANA_AIR",
    "BONUS_MANA_BODY",
    "BONUS_MANA_CHAOS",
    "BONUS_MANA_DEATH",
    "BONUS_MANA_EARTH",
    "BONUS_MANA_ENCHANTMENT",
    "BONUS_MANA_ENTROPY",
    "BONUS_MANA_FIRE",
    "BONUS_MANA_LAW",
    "BONUS_MANA_LIFE",
    "BONUS_MANA_METAMAGIC",
    "BONUS_MANA_MIND",
    "BONUS_MANA_NATURE",
    "BONUS_MANA_SHADOW",
    "BONUS_MANA_SPIRIT",
    "BONUS_MANA_SUN",
    "BONUS_MANA_WATER",
];

/// Per spell type: the world spell whose timing it has (`None` for the other spells, and for
/// Hyborem's Whisper, which FfH never timed for the AI).
fn table(d: &GameData) -> &'static [Option<World>] {
    static TABLE: std::sync::OnceLock<Vec<Option<World>>> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| d.spells.keys().map(|s| NAMES.iter().find(|(n, _)| *n == d.spells[s].base.type_).map(|&(_, w)| w)).collect())
}

/// The spells listed in `FFH_AI_WS_DEFAULT` (type names, `SPELL_` optional, or `ALL`).
fn defaults() -> &'static [String] {
    static LIST: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    LIST.get_or_init(|| {
        std::env::var("FFH_AI_WS_DEFAULT")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().trim_start_matches("SPELL_").to_ascii_uppercase())
            .filter(|s| !s.is_empty())
            .collect()
    })
}

/// True if `w` is cast at a moment of its own ([`turn`]) rather than by FfH's condition.
fn own_moment(w: World) -> bool {
    if !matches!(w, World::Stasis | World::RagingSeas | World::Sanctuary | World::MarchOfTheTrees | World::Worldbreak) {
        return false;
    }
    let name = NAMES.iter().find(|&&(_, x)| x == w).map_or("", |&(n, _)| n).trim_start_matches("SPELL_");
    !defaults().iter().any(|d| d == "ALL" || d == name)
}

/// True once player `p` may cast `spell` at the end of the turn by the AI's timing (always, for a
/// spell that is not a timed world spell; never, for one cast at a moment of its own by
/// [`turn`]). FfH 0.41's AI-only requirements, condition for condition; the rules check the rest
/// (`unit_can_cast`).
pub fn ready(g: &Game, p: PlayerId, spell: SpellType) -> bool {
    let Some(w) = table(g.d)[spell.idx()] else { return true };
    !own_moment(w) && ffh_ready(g, p, w)
}

/// FfH's condition for world spell `w`.
fn ffh_ready(g: &Game, p: PlayerId, w: World) -> bool {
    let d = g.d;
    let pl = &g.players[p];
    let wars = || g.team_get_at_war_count(pl.team, true);
    match w {
        World::ArcaneLacuna => MANA_TYPES.iter().map(|name| g.map.num_bonus[d.t::<BonusType>(name)]).sum::<i32>() >= 7,
        World::Ardor => pl.great_people_created >= 5,
        World::DivineRetribution | World::MarchOfTheTrees | World::Sanctuary => wars() >= 2,
        World::ForTheHorde => g.players[g.barbarian_player()].num_units() >= 60,
        World::GiftsOfNantosuelta | World::Legends | World::Rally | World::RiverOfBlood | World::Stasis => pl.num_cities() >= 5,
        World::IntoTheMist => pl.num_units() >= 40,
        World::MotherLode => pl.improvement_count[d.t::<ImprovementType>("IMPROVEMENT_MINE")] >= 10,
        World::RagingSeas => enemy_fleet(g, p),
        World::ReligiousFervor => g.player_get_state_religion(p).is_some_and(|r| pl.cities.iter().filter(|&&c| g.cities[c].has_religion[r]).count() >= 5),
        World::Revelry => pl.total_population >= 25,
        World::VeilOfNight => pl.num_units() >= 25 && wars() == 0,
        World::Warcry => pl.num_units() >= 25 && wars() > 0,
        World::WildHunt => wars() > 0 && pl.num_units() >= 20,
        World::Worldbreak => g.get_global_counter() >= 50,
    }
}

/// `reqRagingSeas`'s AI condition: a player we are at war with (the barbarians included) has a
/// fleet worth more than 10 (frigates and galleons 2, men-o'-war and queens of the line 3, other
/// warships and transports 1).
fn enemy_fleet(g: &Game, p: PlayerId) -> bool {
    let d = g.d;
    let team = g.players[p].team;
    let weights = [
        ("UNITCLASS_ARCANE_BARGE", 1),
        ("UNITCLASS_CARAVEL", 1),
        ("UNITCLASS_GALLEY", 1),
        ("UNITCLASS_FRIGATE", 2),
        ("UNITCLASS_GALLEON", 2),
        ("UNITCLASS_MAN_O_WAR", 3),
        ("UNITCLASS_PRIVATEER", 1),
        ("UNITCLASS_QUEEN_OF_THE_LINE", 3),
        ("UNITCLASS_TRIREME", 1),
    ];
    g.player_ids().any(|o| {
        let other = &g.players[o];
        other.alive
            && g.teams[team].at_war[other.team]
            && weights.iter().map(|&(class, w)| other.unit_class_count[d.t::<UnitClassType>(class)] * w).sum::<i32>() > 10
    })
}

/// Once a turn, before units move (after `civs::turn`): casts our world spell when it is one cast
/// at a moment of its own and the moment has come. A cast changes the picture (units damaged,
/// expelled or summoned), so the view is rebuilt.
pub fn turn(g: &mut Game, v: &mut View) {
    let p = v.p;
    if v.barbarian || g.players[p].feat_accomplished[FeatType::FEAT_GLOBAL_SPELL] {
        return;
    }
    let Some(civ) = g.player_get_civilization_type(p) else { return };
    let d = g.d;
    let Some(spell) = d.spells.keys().find(|&s| d.spells[s].global && d.spells[s].civilization_prereq == Some(civ)) else { return };
    let Some(w) = table(d)[spell.idx()] else { return };
    if !own_moment(w) {
        return;
    }
    if let Some(why) = moment(g, v, w) {
        if cast_now(g, p, spell, why) {
            *v = View::new(g, p);
        }
    }
}

/// The moment for a world spell with a timing of its own, and why (`None`: not yet).
/// - Stasis (every other player: no production or research for 20 turns): as early as the
///   Illians have a city; the setback is a share of the rivals' game, largest while it is short.
/// - Raging Seas (75 damage to every non-Lanun unit on a plot next to water, a quarter of their
///   coastal improvements destroyed): a stack of ours next to a coastal city of a civ we are at
///   war with (the strike comes this turn), or one of our coastal cities in peril in a civ war.
/// - Sanctuary (no foreign unit may enter our land for 30 turns; those inside are thrown out): a
///   city of ours in peril in a civ war.
/// - March of the Trees (a Treant for 5 turns on every forest of ours): a city in peril in a civ
///   war with 6+ of our forest plots within 2.
/// - Worldbreak (every unit in a foreign city takes the Armageddon counter as damage, cities may
///   burn): a counter of 20+ and a stack of ours next to a city of a civ we are at war with.
fn moment(g: &Game, v: &View, w: World) -> Option<&'static str> {
    let p = v.p;
    let civ_war = !v.civ_enemies.is_empty();
    match w {
        World::Stasis => (g.players[p].num_cities() >= 1).then_some("a city"),
        World::RagingSeas if civ_war => {
            if stack_next_to_enemy_city(g, v, true) {
                Some("a strike on a coastal city")
            } else if v.peril.iter().any(|&(q, _)| g.plot_is_adjacent_to_water(q)) {
                Some("a coastal city in peril")
            } else {
                None
            }
        }
        World::Sanctuary if civ_war => (!v.peril.is_empty()).then_some("a city in peril"),
        World::MarchOfTheTrees if civ_war => v.peril.iter().any(|&(q, _)| forests_near(g, p, q, 2) >= 6).then_some("a city in peril among our forests"),
        World::Worldbreak if civ_war => (g.get_global_counter() >= 20 && stack_next_to_enemy_city(g, v, false)).then_some("a strike"),
        _ => None,
    }
}

/// A stack of ours (3+ land units that can fight) stands next to a city of a civilization we are
/// at war with, on the coast if `coastal`: its strike comes this turn.
fn stack_next_to_enemy_city(g: &Game, v: &View, coastal: bool) -> bool {
    g.players[v.p].groups.iter().any(|&grp| {
        let units = &g.groups[grp].units;
        let fighters = units.iter().filter(|&&u| g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND && g.unit_can_fight(u)).count();
        let Some(&head) = units.first() else { return false };
        if fighters < 3 {
            return false;
        }
        let (x, y) = (g.units[head].x, g.units[head].y);
        (-1..=1).any(|dx| {
            (-1..=1).any(|dy| {
                let Some(q) = g.plot_xy(x, y, dx, dy) else { return false };
                let Some(c) = g.map.plots[q].plot_city else { return false };
                v.civ_enemies.contains(&g.players[g.cities[c].owner].team) && (!coastal || g.plot_is_adjacent_to_water(q))
            })
        })
    })
}

/// Forest and ancient forest plots of `p` within `r` plots of `q`.
fn forests_near(g: &Game, p: PlayerId, q: PlotIdx, r: i32) -> usize {
    let d = g.d;
    let forests = [d.ty::<FeatureType>("FEATURE_FOREST"), d.ty::<FeatureType>("FEATURE_FOREST_ANCIENT")];
    let (x, y) = g.map.xy(q);
    let mut n = 0;
    for dx in -r..=r {
        for dy in -r..=r {
            let Some(t) = g.plot_xy(x, y, dx, dy) else { continue };
            let pl = &g.map.plots[t];
            if pl.owner == Some(p) && pl.feature_type.is_some() && forests.contains(&pl.feature_type) {
                n += 1;
            }
        }
    }
    n
}

/// Casts world spell `spell` with one of the player's units that can (a city defender first, so
/// the army keeps its turn); true if cast. No timing check: the caller decided.
pub fn cast_now(g: &mut Game, p: PlayerId, spell: SpellType, why: &str) -> bool {
    let units: Vec<UnitId> = g.players[p].units.clone();
    let caster = units
        .iter()
        .copied()
        .filter(|&u| g.unit_can_cast(u, spell, false))
        .min_by_key(|&u| (g.units[u].ai_type != Some(UnitAIType::UNITAI_CITY_DEFENSE), u));
    let Some(u) = caster else { return false };
    crate::trace!(g, p, "world spell {} by {} ({})", g.d.spells[spell].base.type_, g.d.units[g.units[u].unit_type].base.type_, why);
    g.unit_cast(u, spell);
    true
}
