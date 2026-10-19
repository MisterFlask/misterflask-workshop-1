//! When the AI casts a world spell (`bGlobal`: once a game, by any unit of the civilization).
//! FfH 0.41 wrote this timing into the rules, as AI-only branches of the spells' Python
//! requirements (`if pPlayer.isHuman() == False:` in `CvSpellInterface.py` `req*`); the port
//! dropped them from the rules (docs/PORTING.md, "Changes from FFH2"), so a world spell has the
//! same requirements for every player and the AI decides when. [`ready`] is FfH's condition, the
//! AI's default timing: `casting::choose_spell` (the end-of-turn spell choice) and the
//! civilizations' held casts (`civs/world_spell.rs`) cast a world spell only once it holds. A
//! civilization can wait longer through its `spell_value` (`civs/`).

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

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

/// True once player `p` may cast `spell` by the AI's timing (always, for a spell that is not a
/// timed world spell). These are FfH 0.41's AI-only requirements, condition for condition; the
/// rules check the rest (`unit_can_cast`).
pub fn ready(g: &Game, p: PlayerId, spell: SpellType) -> bool {
    let Some(w) = table(g.d)[spell.idx()] else { return true };
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
