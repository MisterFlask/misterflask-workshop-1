//! Port of the gameplay structs in CvStructs.h (UI/graphics/network structs omitted).

use crate::ids::*;
use ffh_data::*;

/// C++ `XYCoords`. Ordering matches the C++ operators (by y, then x).
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct XYCoords {
    pub x: i32,
    pub y: i32,
}

impl PartialOrd for XYCoords {
    fn partial_cmp(&self, o: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(o))
    }
}
impl Ord for XYCoords {
    fn cmp(&self, o: &Self) -> std::cmp::Ordering {
        (self.y, self.x).cmp(&(o.y, o.x))
    }
}

/// C++ `GameTurnInfo` lives in ffh-data (`GameSpeedInfo`).

/// C++ `OrderData`: an entry of a city's build queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OrderData {
    pub order_type: OrderType,
    /// C++ `iData1`: unit type / building type / project type / process type / specialist...
    pub data1: i32,
    /// C++ `iData2`: UnitAITypes for ORDER_TRAIN (-1 = none), etc.
    pub data2: i32,
    /// C++ `bSave`: keep the order in the queue after completion (repeat).
    pub save: bool,
}

/// C++ `MissionData`: an entry of a selection group's mission queue.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MissionData {
    pub mission_type: MissionType,
    pub data1: i32,
    pub data2: i32,
    /// C++ `MOVE_*` flags (see `crate::defs`).
    pub flags: i32,
    pub push_turn: i32,
}

/// C++ `TradeData`: one item of a diplomatic deal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TradeData {
    pub item_type: TradeableItem,
    pub data: i32,
    pub offering: bool,
    pub hidden: bool,
}

/// C++ `EventTriggeredData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventTriggeredData {
    pub id: TriggeredId,
    pub trigger: EventTriggerType,
    pub turn: i32,
    pub player: PlayerId,
    pub city: Option<CityId>,
    /// Plot coordinates (C++ -1/INVALID when none).
    pub plot_x: i32,
    pub plot_y: i32,
    pub unit: Option<UnitId>,
    pub other_player: Option<PlayerId>,
    pub other_player_city: Option<CityId>,
    pub religion: Option<ReligionType>,
    pub corporation: Option<CorporationType>,
    pub building: Option<BuildingType>,
    pub text: String,
    pub global_text: String,
}

/// C++ `VoteSelectionSubData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteSelectionSubData {
    pub vote: VoteType,
    pub player: Option<PlayerId>,
    pub city: Option<CityId>,
    pub other_player: Option<PlayerId>,
    pub text: String,
}

/// C++ `VoteSelectionData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteSelectionData {
    pub id: VoteId,
    pub vote_source: VoteSourceType,
    pub vote_options: Vec<VoteSelectionSubData>,
}

/// C++ `VoteTriggeredData`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VoteTriggeredData {
    pub id: VoteId,
    pub vote_source: VoteSourceType,
    pub vote_option: VoteSelectionSubData,
}

/// C++ `PlotExtraYield`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlotExtraYield {
    pub x: i32,
    pub y: i32,
    pub extra_yield: [i32; YieldType::NUM],
}

/// C++ `PlotExtraCost`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlotExtraCost {
    pub x: i32,
    pub y: i32,
    pub cost: i32,
}

// C++ `BuildingYieldChange` / `BuildingCommerceChange` are defined in ffh-data
// (`ffh_data::BuildingYieldChange`, `ffh_data::BuildingCommerceChange`).

/// C++ `BuildingChangeArray` = vector<pair<BuildingClassTypes, int>>.
pub type BuildingChangeArray = Vec<(BuildingClassType, i32)>;

/// C++ `PlayerVoteTypes`: the negative sentinels of CvEnums.h, or (team votes only: secretary
/// general and diplomatic victory, `CvGame::isTeamVote`) a `TeamTypes` value, the team voted
/// for. C++ casts both ways (`(PlayerVoteTypes)eTeam` in `doVoteResults` and `AI_diploVote`,
/// `(TeamTypes)` in `castVote` and `getSecretaryGeneral`); a non-negative value is never a
/// player.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlayerVote(pub i32);

impl PlayerVote {
    pub const NO_PLAYER_VOTE_CHECKED: PlayerVote = PlayerVote(-6);
    pub const PLAYER_VOTE_NEVER: PlayerVote = PlayerVote(-5);
    pub const PLAYER_VOTE_ABSTAIN: PlayerVote = PlayerVote(-4);
    pub const PLAYER_VOTE_NO: PlayerVote = PlayerVote(-3);
    pub const PLAYER_VOTE_YES: PlayerVote = PlayerVote(-2);
    pub const NO_PLAYER_VOTE: PlayerVote = PlayerVote(-1);

    /// C++ `(PlayerVoteTypes)eTeam`: a vote for team `t` (team votes).
    pub fn for_team(t: TeamId) -> PlayerVote {
        PlayerVote(t.0 as i32)
    }

    /// C++ `(TeamTypes)ePlayerVote`: the team voted for, `None` for a sentinel.
    pub fn team(self) -> Option<TeamId> {
        (self.0 >= 0).then(|| TeamId(self.0 as u8))
    }
}
