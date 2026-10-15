//! New games and take-over (design §2.6): the custom-game settings a human picks
//! ([`NewGame`]), the `Game` they make with the human in slot 0, and the human-play
//! configuration every session applies (active player, player options, autorun off, log on).

use ffh_core::controller::ControllerRef;
use ffh_core::defs::MAX_CIV_PLAYERS;
use ffh_core::game::Game;
use ffh_core::ids::PlayerId;
use ffh_core::mapscript::FractalMapScript;
use ffh_core::setup::{GameSetup, SetupOptions, SlotStatus};
use ffh_data::*;

use crate::json::{FieldError, Json};
use crate::names;

/// The new-game screen's settings (design §3). The human plays slot 0 against `opponents` AI
/// civs; `civ`/`leader` `None` means random (FfH's RANDOM_CIVILIZATION pick in `CvGame::init`).
#[derive(Clone, Debug, PartialEq)]
pub struct NewGame {
    pub seed: u32,
    pub opponents: usize,
    pub world: WorldSizeType,
    pub speed: GameSpeedType,
    /// The human's difficulty, which is the game handicap (`CvGame::init(eHandicap)`).
    pub handicap: HandicapType,
    pub era: EraType,
    pub climate: ClimateType,
    pub sea_level: SeaLevelType,
    pub options: Vec<GameOptionType>,
    pub disabled_victories: Vec<VictoryType>,
    /// 0 = from the game speed, as the exe does.
    pub max_turns: i32,
    pub civ: Option<CivilizationType>,
    pub leader: Option<LeaderHeadType>,
}

impl NewGame {
    /// The defaults: 3 opponents on a small map, normal speed, Noble, first era, temperate,
    /// medium sea level, no options, every victory, random civilization and leader.
    pub fn new(d: &GameData, seed: u32) -> NewGame {
        let o = SetupOptions::new(d, 4, "WORLDSIZE_SMALL", seed);
        NewGame {
            seed,
            opponents: 3,
            world: o.world_size,
            speed: o.game_speed,
            handicap: o.handicap,
            era: o.era,
            climate: o.climate,
            sea_level: o.sea_level,
            options: Vec::new(),
            disabled_victories: Vec::new(),
            max_turns: 0,
            civ: None,
            leader: None,
        }
    }

    /// The members of a `setup` object.
    pub const FIELDS: &'static [&'static str] =
        &["seed", "opponents", "world", "speed", "handicap", "era", "climate", "sea_level", "options", "disabled_victories", "max_turns", "civ", "leader"];

    /// Decodes a `setup` object (every member optional; types by name or index; an unknown
    /// member is an error, so a typo cannot start a game with a default instead).
    pub fn from_json(d: &GameData, j: &Json) -> Result<NewGame, FieldError> {
        j.only(NewGame::FIELDS)?;
        let mut ng = NewGame::new(d, j.get_or("seed", 1u32)?);
        ng.opponents = j.get_or("opponents", ng.opponents)?;
        if let Some(v) = names::opt_field(d, j, "world")? {
            ng.world = v;
        }
        if let Some(v) = names::opt_field(d, j, "speed")? {
            ng.speed = v;
        }
        if let Some(v) = names::opt_field(d, j, "handicap")? {
            ng.handicap = v;
        }
        if let Some(v) = names::opt_field(d, j, "era")? {
            ng.era = v;
        }
        if let Some(v) = names::opt_field(d, j, "climate")? {
            ng.climate = v;
        }
        if let Some(v) = names::opt_field(d, j, "sea_level")? {
            ng.sea_level = v;
        }
        if j.opt_value("options")?.is_some() {
            ng.options = names::list_field(d, j, "options")?;
        }
        if j.opt_value("disabled_victories")?.is_some() {
            ng.disabled_victories = names::list_field(d, j, "disabled_victories")?;
        }
        ng.max_turns = j.get_or("max_turns", 0i32)?;
        ng.civ = names::opt_field(d, j, "civ")?;
        ng.leader = names::opt_field(d, j, "leader")?;
        Ok(ng)
    }

    /// The canonical form (indices), as the journal header records it.
    pub fn to_json(&self) -> Json {
        crate::jobj! {
            "seed" => self.seed,
            "opponents" => self.opponents,
            "world" => Json::idx(self.world),
            "speed" => Json::idx(self.speed),
            "handicap" => Json::idx(self.handicap),
            "era" => Json::idx(self.era),
            "climate" => Json::idx(self.climate),
            "sea_level" => Json::idx(self.sea_level),
            "options" => Json::Arr(self.options.iter().map(|&o| Json::idx(o)).collect()),
            "disabled_victories" => Json::Arr(self.disabled_victories.iter().map(|&v| Json::idx(v)).collect()),
            "max_turns" => self.max_turns,
            "civ" => Json::opt_idx(self.civ),
            "leader" => Json::opt_idx(self.leader),
        }
    }

    /// The civilization and leader of the human slot after resolving a half-given pair: a
    /// leader alone gives its civilization; a civilization alone gets one of its leaders
    /// (picked from the seed, as the exe's "random leader" entry). Errors for an invalid pair.
    pub fn resolve_civ(&self, d: &GameData) -> Result<(Option<CivilizationType>, Option<LeaderHeadType>), String> {
        let playable = |c: CivilizationType| d.civilizations[c].playable && !d.civilizations[c].base.graphical_only;
        match (self.civ, self.leader) {
            (None, None) => Ok((None, None)),
            (Some(c), l) => {
                if !playable(c) {
                    return Err(format!("{} is not playable", d.type_name(c)));
                }
                let leaders: Vec<LeaderHeadType> = d.leader_heads.keys().filter(|&l| d.civilizations[c].leaders[l]).collect();
                match l {
                    Some(l) if leaders.contains(&l) => Ok((Some(c), Some(l))),
                    Some(l) => Err(format!("{} does not lead {}", d.type_name(l), d.type_name(c))),
                    None if leaders.is_empty() => Err(format!("{} has no leader", d.type_name(c))),
                    None => Ok((Some(c), Some(leaders[self.seed as usize % leaders.len()]))),
                }
            }
            (None, Some(l)) => match d.civilizations.keys().find(|&c| playable(c) && d.civilizations[c].leaders[l]) {
                Some(c) => Ok((Some(c), Some(l))),
                None => Err(format!("{} leads no playable civilization", d.type_name(l))),
            },
        }
    }

    /// Checks the settings (also done by [`build_game`]).
    pub fn validate(&self, d: &GameData) -> Result<(), String> {
        if self.opponents < 1 || self.opponents + 1 > MAX_CIV_PLAYERS {
            return Err(format!("opponents must be in 1..={}", MAX_CIV_PLAYERS - 1));
        }
        if self.options.contains(&GameOptionType::GAMEOPTION_ADVANCED_START) {
            return Err("GAMEOPTION_ADVANCED_START is not supported (there is no advanced-start interface)".into());
        }
        if self.max_turns < 0 {
            return Err("max_turns must be >= 0".into());
        }
        self.resolve_civ(d).map(|_| ())
    }
}

/// The human player of a new game.
pub const HUMAN_SLOT: PlayerId = PlayerId(0);

/// `Game::new` for `ng` with the human in slot 0 (not yet `game_init`ed): steps 1-4 of the
/// new-game sequence (design §2.6).
pub fn build_game(d: &'static GameData, ng: &NewGame) -> Result<Game, String> {
    ng.validate(d)?;
    let (civ, leader) = ng.resolve_civ(d)?;
    let mut o = SetupOptions::new(d, ng.opponents + 1, "WORLDSIZE_SMALL", ng.seed);
    o.world_size = ng.world;
    o.game_speed = ng.speed;
    o.handicap = ng.handicap;
    o.era = ng.era;
    o.climate = ng.climate;
    o.sea_level = ng.sea_level;
    o.options = ng.options.clone();
    o.disabled_victories = ng.disabled_victories.clone();
    o.max_turns = ng.max_turns;
    let mut setup = GameSetup::from_options(d, &o);
    let slot = &mut setup.players[HUMAN_SLOT.idx()];
    slot.human = true;
    slot.slot_status = SlotStatus::Taken;
    slot.civ = civ;
    slot.leader = leader;
    slot.handicap = Some(ng.handicap);
    let mut g = Game::new(d, setup);
    // The exe passes the first human slot's handicap to CvGame::init.
    g.handicap = Game::setup_handicap(d, &g.setup);
    g.map_script = std::sync::Arc::new(FractalMapScript::default());
    Ok(g)
}

/// Controllers: every slot but `human` gets the one shared AI controller, as in ffh-cli (no
/// wrapper, so trait methods added later reach the AI); `human` gets `human_controller`.
pub fn attach_controllers(g: &mut Game, human: PlayerId, ai: ControllerRef, human_controller: ControllerRef) {
    for p in g.player_ids() {
        g.controllers[p] = if p == human { human_controller.clone() } else { ai.clone() };
    }
}

/// The human-play flags (design §2.6 steps 6 and 8): the game ends when the human dies
/// (`autorun` off), `human` is the active player (quick moves, `getDeclareWarMove` visibility,
/// the research popup), and the log is on and unbounded (the session drains it).
pub fn configure_human(g: &mut Game, human: PlayerId) {
    g.autorun = false;
    g.active_player = Some(human);
    g.log.enabled = true;
    g.log.cap = 0;
}

/// Player options of the human (design §2.6 step 7, F3): QUICK_MOVES on (mission timers of 1),
/// SHOW_FRIENDLY_MOVES and SHOW_ENEMY_MOVES off (they would give AI groups mission timers).
pub fn set_human_options(g: &mut Game, human: PlayerId) {
    let o = &mut g.players[human].options;
    o[PlayerOptionType::PLAYEROPTION_QUICK_MOVES] = true;
    o[PlayerOptionType::PLAYEROPTION_SHOW_FRIENDLY_MOVES] = false;
    o[PlayerOptionType::PLAYEROPTION_SHOW_ENEMY_MOVES] = false;
}

/// Lists for the new-game screen: AI-playable civilizations with their leaders, world sizes,
/// speeds, handicaps, eras, climates, sea levels, visible game options, victories, and the
/// defaults (`NewGame::new`).
pub fn setup_lists(d: &GameData) -> Json {
    fn list<T: names::Named>(d: &GameData, keep: impl Fn(T) -> bool) -> Json {
        Json::Arr(
            (0..T::count(d))
                .map(T::from_usize)
                .filter(|&t| keep(t))
                .map(|t| crate::jobj! { "id" => Json::idx(t), "type" => names::name(d, t), "name" => names::display(d, t).into_owned() })
                .collect(),
        )
    }
    let civs = Json::Arr(
        d.civilizations
            .keys()
            .filter(|&c| d.civilizations[c].playable && !d.civilizations[c].base.graphical_only)
            .map(|c| {
                let leaders: Vec<Json> = d.leader_heads.keys().filter(|&l| d.civilizations[c].leaders[l]).map(Json::idx).collect();
                crate::jobj! {
                    "id" => Json::idx(c),
                    "type" => names::name(d, c),
                    "name" => names::display(d, c).into_owned(),
                    "leaders" => Json::Arr(leaders),
                }
            })
            .collect(),
    );
    crate::jobj! {
        "civilizations" => civs,
        "leaders" => list::<LeaderHeadType>(d, |_| true),
        "worlds" => list::<WorldSizeType>(d, |_| true),
        "speeds" => list::<GameSpeedType>(d, |_| true),
        "handicaps" => list::<HandicapType>(d, |_| true),
        "eras" => list::<EraType>(d, |_| true),
        "climates" => list::<ClimateType>(d, |_| true),
        "sea_levels" => list::<SeaLevelType>(d, |_| true),
        "options" => list::<GameOptionType>(d, |o| d.game_options[o].visible && o != GameOptionType::GAMEOPTION_ADVANCED_START),
        "victories" => list::<VictoryType>(d, |_| true),
        "max_opponents" => MAX_CIV_PLAYERS - 1,
        "defaults" => NewGame::new(d, 1).to_json(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_round_trip_and_defaults() {
        let d = ffh_data::global();
        let ng = NewGame::new(d, 9);
        assert_eq!(NewGame::from_json(d, &ng.to_json()).unwrap(), ng);
        let j = Json::parse(r#"{"seed":3,"opponents":2,"world":"WORLDSIZE_TINY","civ":"CIVILIZATION_BANNOR","options":["GAMEOPTION_RAGING_BARBARIANS"]}"#).unwrap();
        let ng = NewGame::from_json(d, &j).unwrap();
        assert_eq!(ng.world, d.t::<WorldSizeType>("WORLDSIZE_TINY"));
        assert_eq!(ng.options, vec![GameOptionType::GAMEOPTION_RAGING_BARBARIANS]);
        let (civ, leader) = ng.resolve_civ(d).unwrap();
        assert_eq!(civ, Some(d.t("CIVILIZATION_BANNOR")));
        assert!(d.civilizations[civ.unwrap()].leaders[leader.unwrap()]);
        assert_eq!(NewGame::from_json(d, &ng.to_json()).unwrap(), ng);
        let err = NewGame::from_json(d, &Json::parse(r#"{"world":"TECH_X"}"#).unwrap()).unwrap_err();
        assert_eq!(err.key, "world");
        // A misspelt member is an error, not a default game (P4 review).
        let err = NewGame::from_json(d, &Json::parse(r#"{"seed":4,"world_size":"WORLDSIZE_DUEL","oponents":1}"#).unwrap()).unwrap_err();
        assert_eq!(err.key, "world_size");
        assert!(err.to_string().contains("unknown field"), "{err}");
    }

    #[test]
    fn invalid_settings_are_refused() {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 1);
        ng.options.push(GameOptionType::GAMEOPTION_ADVANCED_START);
        assert!(ng.validate(d).unwrap_err().contains("ADVANCED_START"));
        let mut ng = NewGame::new(d, 1);
        ng.opponents = 0;
        assert!(ng.validate(d).is_err());
        let mut ng = NewGame::new(d, 1);
        ng.civ = Some(d.t("CIVILIZATION_BANNOR"));
        ng.leader = d.leader_heads.keys().find(|&l| !d.civilizations[ng.civ.unwrap()].leaders[l]);
        assert!(ng.validate(d).unwrap_err().contains("does not lead"));
        // A leader alone gives its civilization.
        let mut ng = NewGame::new(d, 1);
        let bannor = d.t::<CivilizationType>("CIVILIZATION_BANNOR");
        ng.leader = d.leader_heads.keys().find(|&l| d.civilizations[bannor].leaders[l]);
        assert_eq!(ng.resolve_civ(d).unwrap().0, Some(bannor));
    }

    #[test]
    fn human_slot_and_lists() {
        let d = ffh_data::global();
        let mut ng = NewGame::new(d, 4);
        ng.handicap = d.t("HANDICAP_PRINCE");
        let g = build_game(d, &ng).unwrap();
        assert!(g.setup.players[0].human);
        assert_eq!(g.setup.players[0].slot_status, SlotStatus::Taken);
        assert!(g.setup.players[1..].iter().all(|p| !p.human));
        assert_eq!(g.handicap, ng.handicap);
        assert_eq!(g.num_human_players(), 1);
        let lists = setup_lists(d);
        assert!(!lists.req_arr("civilizations").unwrap().is_empty());
        assert!(lists.req_arr("options").unwrap().iter().all(|o| o.req_str("type").unwrap() != "GAMEOPTION_ADVANCED_START"));
    }
}
