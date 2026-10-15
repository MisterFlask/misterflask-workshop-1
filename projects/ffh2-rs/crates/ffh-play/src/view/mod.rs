//! What the human sees (design §4.4-4.7): the views and queries, built from the human team's
//! side through a [`Viewer`] (the fog rules are in [`fog`]), or from the true state under debug
//! reveal (replies then carry `"reveal": true`).
//!
//! Every type-valued field of a reply is an integer index (design §4.1); `data` maps them back to
//! names. Moves are in C++ `MOVE_DENOMINATOR` units (60 per move).
//!
//! * [`view`] and [`status`] here; [`map`] (map arrays, `plot`), [`units`] (`group` with its
//!   actions, `path`, `combat`), [`city`] (the city screen), [`player`] (`techs`, `civics`,
//!   `religions`, `players`), [`diplo`] (`trade` and the deal checks of `propose_deal`),
//!   [`data`] (the static tables), [`text`] (`text`, `ascii`, `inspect`).

pub mod city;
pub mod data;
pub mod diplo;
pub mod fog;
pub mod hotkeys;
pub mod map;
pub mod player;
pub mod text;
pub mod units;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use crate::json::Json;
use crate::query::Ctx;
use crate::session::{phase, Phase};

/// The human's side of the game: the game, the human and its team, and whether debug reveal is
/// on. The fog predicates are methods ([`fog`]).
#[derive(Copy, Clone)]
pub struct Viewer<'a> {
    pub g: &'a Game,
    pub h: PlayerId,
    pub team: TeamId,
    /// Debug reveal: the true state (marked in replies).
    pub reveal: bool,
}

impl<'a> Viewer<'a> {
    pub fn new(g: &'a Game, h: PlayerId, reveal: bool) -> Viewer<'a> {
        Viewer { g, h, team: g.players[h].team, reveal }
    }

    pub fn d(&self) -> &'static GameData {
        self.g.d
    }
}

/// A city's name as text (ffh-core stores the `TXT_KEY_CITY_*` key of a generated name).
pub fn city_name(g: &Game, c: CityId) -> String {
    crate::names::text(g.d, &g.cities[c].name).to_string()
}

/// A player's name as text (some leader names are text keys: `TXT_KEY_LEADER_BARBARIAN`).
pub fn player_name(g: &Game, p: PlayerId) -> String {
    crate::names::resolve_keys(g.d, &g.player_get_name(p)).into_owned()
}

/// The FfH mana bonuses in the order the main interface lists them (CvMainInterface.py
/// `manaTypes1` + `manaTypes2`).
pub const MANA: [&str; 18] = [
    "BONUS_MANA_AIR",
    "BONUS_MANA_BODY",
    "BONUS_MANA_CHAOS",
    "BONUS_MANA_DEATH",
    "BONUS_MANA_EARTH",
    "BONUS_MANA_ENCHANTMENT",
    "BONUS_MANA_ENTROPY",
    "BONUS_MANA_FIRE",
    "BONUS_MANA_ICE",
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

/// The phase word of `status` (`decision` while a blocking decision is open).
pub fn phase_name(g: &Game, h: PlayerId, decision: bool) -> &'static str {
    if decision {
        return "decision";
    }
    match phase(g, h) {
        Phase::Human(_) => "human",
        Phase::Running => "running",
        Phase::Over => "over",
    }
}

/// `status` (design §4.3; the transport adds `version`, `ms_in_phase` and the crash report).
pub fn status(v: &Viewer, ctx: &Ctx) -> Json {
    let g = v.g;
    let over = (g.game_state == GameStateType::GAMESTATE_OVER).then(|| crate::jobj! { "winner_team" => g.winner, "victory" => Json::opt_idx(g.victory) });
    crate::jobj! {
        "phase" => phase_name(g, v.h, ctx.decision.is_some()),
        "turn" => g.game_turn(),
        "year" => g.game_turn_year(),
        "human" => v.h,
        "active_players" => g.player_ids().filter(|&p| g.players[p].turn_active).collect::<Vec<_>>(),
        "view_version" => crate::query::UiState::lock(ctx.ui).view_version as i64,
        "decision" => ctx.decision.map(|d| d.to_json()),
        "over" => over,
        "reveal" => v.reveal,
        // The AI autoplay's turns left (`autoplay`; the client offers to stop it while it runs).
        "ai_auto_play" => g.ai_auto_play,
    }
}

/// The human player's empire summary (the top bar).
pub fn me(v: &Viewer) -> Json {
    let g = v.g;
    let d = g.d;
    let h = v.h;
    let pl = &g.players[h];
    let research = g.player_get_current_research(h).map(|t| {
        crate::jobj! {
            "tech" => Json::idx(t),
            "progress" => g.teams[v.team].research_progress[t],
            "cost" => g.team_get_research_cost(v.team, t),
            "turns" => g.player_get_research_turns_left(h, t, true),
            "rate" => g.player_calculate_research_rate(h, Some(t)),
        }
    });
    let mana: Vec<Json> = MANA
        .iter()
        .filter_map(|n| d.ty::<BonusType>(n))
        .map(|b| crate::jobj! { "bonus" => Json::idx(b), "count" => g.player_get_num_available_bonuses(h, b) })
        .collect();
    crate::jobj! {
        "player" => h,
        "team" => v.team,
        "name" => crate::view::player_name(g, h),
        "civ" => Json::opt_idx(g.player_get_civilization_type(h)),
        "leader" => Json::opt_idx(g.player_get_leader_type(h)),
        "gold" => pl.gold,
        "gold_rate" => g.player_calculate_gold_rate(h),
        "research" => research,
        "research_queue" => Json::Arr(pl.research_queue.iter().map(|&t| Json::idx(t)).collect()),
        "research_rate" => g.player_calculate_research_rate(h, None),
        "commerce_percent" => pl.commerce_percent.to_vec(),
        "flexible" => (0..CommerceType::NUM).map(|c| g.player_is_commerce_flexible(h, CommerceType::from_usize(c))).collect::<Vec<_>>(),
        "anarchy" => pl.anarchy_turns,
        "golden_age" => pl.golden_age_turns,
        "state_religion" => Json::opt_idx(g.player_get_state_religion(h)),
        "civics" => Json::Arr(pl.civics.iter().map(|&c| Json::opt_idx(c)).collect()),
        "score" => g.player_score[h],
        "armageddon" => g.get_global_counter(),
        "mana" => Json::Arr(mana),
        "stasis" => crate::jobj! {
            "research" => pl.disable_research, "production" => pl.disable_production, "spells" => pl.disable_spellcasting,
        },
        "alive" => pl.alive,
        // Player options by PlayerOptionType index (`set_option` changes a few of them).
        "options" => pl.options.iter().copied().collect::<Vec<bool>>(),
        "ai_auto_play" => g.ai_auto_play,
    }
}

/// The players the human knows, from its side (the `view`'s list).
pub fn players_brief(v: &Viewer) -> Json {
    let g = v.g;
    let mut out = Vec::new();
    for p in g.player_ids() {
        if p == v.h || !v.player_met(p) || !g.players[p].ever_alive {
            continue;
        }
        let pl = &g.players[p];
        let t = pl.team;
        out.push(crate::jobj! {
            "id" => p,
            "name" => crate::view::player_name(g, p),
            "leader" => Json::opt_idx(g.player_get_leader_type(p)),
            "civ" => Json::opt_idx(g.player_get_civilization_type(p)),
            "team" => t,
            "barbarian" => g.player_is_barbarian(p),
            "alive" => pl.alive,
            "war" => g.at_war(v.team, t),
            "attitude" => if pl.alive { Json::idx(g.player_ai_get_attitude(p, v.h, true)) } else { Json::Null },
            "open_borders" => g.teams[v.team].open_borders[t],
            "defensive_pact" => g.teams[v.team].defensive_pact[t],
            "vassal_of_us" => g.teams[t].vassal[v.team],
            "our_master" => g.teams[v.team].vassal[t],
            "score" => g.player_score[p],
            "contact" => g.player_can_contact(v.h, p),
        });
    }
    Json::Arr(out)
}

/// The head order of a city's queue: `{order, type, turns}` (`turns` null for a process).
pub fn production(g: &Game, c: CityId) -> Json {
    let city = &g.cities[c];
    let Some(o) = city.order_queue.first() else { return Json::Null };
    let turns = match o.order_type {
        OrderType::ORDER_MAINTAIN => Json::Null,
        _ => Json::from(g.city_production_turns_left(c)),
    };
    crate::jobj! { "order" => Json::idx(o.order_type), "type" => o.data1, "turns" => turns }
}

/// The cities the human has revealed: own cities in full, foreign ones by name and owner (size
/// and defence while visible).
pub fn cities(v: &Viewer) -> Json {
    let g = v.g;
    let mut out = Vec::new();
    // Walls and the palisade (the buildings of those classes).
    let wall_classes = ["BUILDINGCLASS_WALLS", "BUILDINGCLASS_PALISADE"].map(|n| g.d.ty::<BuildingClassType>(n));
    let walls: Vec<BuildingType> = g.d.buildings.keys().filter(|&b| g.d.buildings[b].building_class_type.is_some_and(|k| wall_classes.contains(&Some(k)))).collect();
    for (c, city) in g.cities.iter() {
        if !v.city_revealed(c) {
            continue;
        }
        let own = city.owner == v.h || v.reveal;
        let visible = v.city_visible(c);
        let mut j = crate::jobj! {
            "id" => c,
            "owner" => city.owner,
            "x" => city.x,
            "y" => city.y,
            "name" => city_name(g, c),
            "capital" => g.city_is_capital(c),
            "visible" => visible,
            "size" => visible.then_some(city.population),
            "defense" => visible.then(|| g.city_defense_modifier(c, false)),
            // The city art's curtain wall (docs/ART.md 9.3), seen like its size.
            "walls" => visible.then(|| walls.iter().any(|&b| g.city_num_building(c, b) > 0)),
        };
        if own {
            j.set("production", production(g, c));
            j.set("idle", !g.city_is_production(c) && !city.production_automated);
            j.set("automated_production", city.production_automated);
            j.set("food_turns", g.city_food_turns_left(c));
            j.set("disorder", g.city_is_disorder(c));
        }
        out.push(j);
    }
    Json::Arr(out)
}

/// One unit as the human sees it: own units in full, foreign ones by type, owner and health.
pub fn unit_json(v: &Viewer, u: UnitId) -> Json {
    let g = v.g;
    let unit = &g.units[u];
    let mut j = crate::jobj! {
        "id" => u,
        "owner" => v.unit_owner(u),
        "type" => Json::idx(unit.unit_type),
        "x" => unit.x,
        "y" => unit.y,
        "hp" => g.unit_curr_hit_points(u),
        "max_hp" => g.unit_max_hit_points(u),
        "level" => unit.level,
    };
    if unit.owner == v.h || v.reveal {
        let grp = unit.group;
        j.set("group", grp);
        j.set("moves", g.unit_moves_left(u));
        j.set("max_moves", g.unit_max_moves(u));
        j.set("xp", unit.experience);
        j.set("xp_needed", g.unit_experience_needed(u));
        j.set("promo_ready", unit.promotion_ready);
        j.set("activity", grp.filter(|&gr| g.groups.contains(gr)).map(|gr| Json::idx(g.groups[gr].activity_type)));
        j.set("transport", unit.transport_unit);
        j.set("summon_turns", (unit.duration > 0).then_some(unit.duration));
    }
    j
}

/// The units the human sees: its own (creation order), then visible foreign ones (id order).
pub fn units(v: &Viewer) -> Json {
    let g = v.g;
    let mut out: Vec<Json> = g.players[v.h].units.iter().map(|&u| unit_json(v, u)).collect();
    for (u, unit) in g.units.iter() {
        if unit.owner != v.h && v.unit_visible(u) {
            out.push(unit_json(v, u));
        }
    }
    Json::Arr(out)
}

/// The human's groups.
pub fn groups(v: &Viewer) -> Json {
    let g = v.g;
    let out = g.players[v.h].groups.iter().filter(|&&grp| g.groups.contains(grp)).map(|&grp| {
        let group = &g.groups[grp];
        crate::jobj! {
            "id" => grp,
            "head" => g.group_get_head_unit(grp),
            "units" => group.units.clone(),
            "ready" => g.group_ready_to_move(grp, true) && !crate::decision::is_ai_group(g, grp),
            // What Civ4's group cycle selects (W, Enter, the next group after a command, the
            // waiting groups on a plot, the nearest group): C++ `readyToSelect()` with its
            // default `bAny = false`, i.e. every unit can move (`canAllMove`), not only one.
            "selectable" => crate::decision::is_selectable_group(g, grp),
            "busy" => g.group_is_busy(grp),
            "activity" => Json::idx(group.activity_type),
            "missions" => Json::Arr(group.mission_queue.iter().map(|m| crate::jobj! {
                "m" => Json::idx(m.mission_type), "d1" => m.data1, "d2" => m.data2,
            }).collect()),
            "automated" => Json::opt_idx(group.automate_type),
            "ai_control" => g.group_is_ai_control(grp),
        }
    });
    Json::Arr(out.collect())
}

/// `view` (design §4.4).
pub fn view(v: &Viewer, ctx: &Ctx) -> Json {
    let g = v.g;
    let view_version = crate::query::UiState::lock(ctx.ui).view_version as i64;
    crate::jobj! {
        "view_version" => view_version,
        "turn" => g.game_turn(),
        "year" => g.game_turn_year(),
        "reveal" => v.reveal,
        "me" => me(v),
        "players" => players_brief(v),
        "cities" => cities(v),
        "units" => units(v),
        "groups" => groups(v),
        // C++ `CvPlayer::m_groupCycle`: the order W cycles the waiting groups in
        // (`cycleSelectionGroups`), and whether some group has orders to continue
        // (`hasAutoUnit`: the cycle's wrap then sends AutoMoves).
        "group_cycle" => g.players[v.h].group_cycle.iter().filter(|&&grp| g.groups.contains(grp)).copied().collect::<Vec<_>>(),
        "has_auto_unit" => g.player_has_auto_unit(v.h),
        "pending" => crate::decision::pending(g, v.h, ctx.events).to_json(),
        "log_next" => ctx.messages.map(|m| m.total()),
    }
}
