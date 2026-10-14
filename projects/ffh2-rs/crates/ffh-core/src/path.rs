//! Pathfinding: replacement for the exe's `FAStar` (A* over plots; the DLL only sees
//! `FAStarNode.h` and `CvDLLFAStarIFaceBase.h`) plus the DLL's FAStar callbacks from
//! CvGameCoreUtils.cpp (`pathDestValid`/`pathHeuristic`/`pathCost`/`pathValid`/`pathAdd`,
//! `step*`, `routeValid`, `borderValid`, `areaValid`/`joinArea`, `plotGroupValid`/
//! `countPlotGroup`, `potentialIrrigation`/`checkFreshWater`/`changeIrrigated`).
//!
//! C++ keeps one global `FAStar` per purpose in `GC` (initialized by `CvMap::setup`); here
//! they are `Game::path_finder`, `step_finder`, ... (`RefCell`, because C++
//! `CvSelectionGroup::generatePath` is `const`). Callbacks get the finder's search context as
//! a [`FinderCtx`] (C++ `GetInfo`, `GetStartX`, `IsPathDest`, ...) and the C++ `SetData`
//! pointer as an explicit argument. Callbacks that mutate the game (`joinArea`,
//! `changeIrrigated`) only act on `ASNL_ADDCLOSED` notifications and do not influence the
//! search, so callers can apply them to the closed list after a search.

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::defs::*;
use crate::game::Game;
use crate::ids::*;
use crate::utils::MapGeom;
use ffh_data::*;

// CvGameCoreUtils.cpp path cost weights.
pub const PATH_MOVEMENT_WEIGHT: i32 = 1000;
pub const PATH_RIVER_WEIGHT: i32 = 100;
pub const PATH_CITY_WEIGHT: i32 = 100;
pub const PATH_DEFENSE_WEIGHT: i32 = 10;
pub const PATH_TERRITORY_WEIGHT: i32 = 3;
pub const PATH_STEP_WEIGHT: i32 = 2;
pub const PATH_STRAIGHT_WEIGHT: i32 = 1;
/// C++ `#define PATH_DAMAGE_WEIGHT (500)` (pathCost itself reads `GC.getPATH_DAMAGE_WEIGHT()`).
pub const PATH_DAMAGE_WEIGHT: i32 = 500;

// FAStarNode.h list notifications (`data` argument of NotifyList callbacks).
pub const ASNL_ADDOPEN: i32 = 0;
pub const ASNL_STARTOPEN: i32 = 1;
pub const ASNL_DELETEOPEN: i32 = 2;
pub const ASNL_ADDCLOSED: i32 = 3;

// FAStarNode.h child notifications (`data` argument of NotifyChild callbacks).
pub const ASNC_INITIALADD: i32 = 0;
pub const ASNC_OPENADD_UP: i32 = 1;
pub const ASNC_CLOSEDADD_UP: i32 = 2;
pub const ASNC_PARENTADD_UP: i32 = 3;
pub const ASNC_NEWADD: i32 = 4;

/// C++ `FAStarListType`.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PathListType {
    Open,
    Closed,
}

/// C++ `FAStarNode`. Nodes are indexed by plot index; links are node indices.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PathNode {
    pub x: i32,
    pub y: i32,
    /// Fitness (f).
    pub total_cost: i32,
    /// Goal (g).
    pub known_cost: i32,
    /// Heuristic (h).
    pub heuristic_cost: i32,
    pub num_children: i32,
    /// Path finder: moves left at this node; step finder: steps from the start.
    pub data1: i32,
    /// Path finder: turn number (1 = reached this turn).
    pub data2: i32,
    pub on_stack: bool,
    pub list: Option<PathListType>,
    pub parent: Option<u32>,
    pub children: [Option<u32>; 8],
}

/// Which C++ `FAStar` instance (and callback set) a finder is.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FinderKind {
    /// `GC.getPathFinder()`: selection group movement (pathDestValid, pathHeuristic, pathCost,
    /// pathValid, pathAdd); `SetData` = the selection group; info = `MOVE_*` flags.
    Path,
    /// `GC.getStepFinder()`: steps within an area (stepDestValid, stepHeuristic, stepCost,
    /// stepValid, stepAdd); used by `CvMap::calculatePathDistance`.
    Step,
    /// `GC.getRouteFinder()`: routeValid; info = player.
    Route,
    /// `GC.getBorderFinder()`: borderValid; info = player.
    Border,
    /// `GC.getAreaFinder()`: flood fill with areaValid + joinArea; info = area id.
    Area,
    /// `GC.getPlotGroupFinder()`: flood fill with plotGroupValid + countPlotGroup; info = player.
    PlotGroup,
    /// `CvPlot::updateIrrigated`'s local finder: potentialIrrigation + checkFreshWater or
    /// changeIrrigated.
    Irrigation,
}

/// The parts of the C++ `FAStar*` that callbacks query (`GetInfo`, `GetStartX/Y`,
/// `GetDestX/Y`, `IsPathStart`, `IsPathDest`).
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct FinderCtx {
    pub start_x: i32,
    pub start_y: i32,
    /// -1 for flood fills.
    pub dest_x: i32,
    pub dest_y: i32,
    /// C++ `GetInfo`: `iInfo` argument of `GeneratePath`.
    pub info: i32,
}

impl FinderCtx {
    /// C++ `IsPathStart`.
    pub fn is_path_start(&self, x: i32, y: i32) -> bool {
        self.start_x == x && self.start_y == y
    }

    /// C++ `IsPathDest`.
    pub fn is_path_dest(&self, x: i32, y: i32) -> bool {
        self.dest_x == x && self.dest_y == y
    }
}

/// C++ `FAStar`: A* (or flood fill when the destination is -1, -1) over the plot grid.
#[derive(Clone, Debug)]
pub struct PathFinder {
    pub kind: FinderKind,
    /// Grid size and wrapping (C++ `Initialize(columns, rows, wrapX, wrapY, ...)`).
    pub geom: MapGeom,
    /// Context of the last `generate_path`.
    pub ctx: FinderCtx,
    pub cardinal_only: bool,
    /// C++ `SetData` for the path finder: the selection group being moved.
    pub group: Option<GroupId>,
    /// One node per plot (index = plot index).
    pub nodes: Vec<PathNode>,
    /// C++ `GetLastNode`: the destination node of the last successful search.
    pub last_node: Option<u32>,
}

impl PathFinder {
    /// C++ `gDLL->getFAStarIFace()->create()`.
    pub fn new(kind: FinderKind) -> PathFinder {
        PathFinder {
            kind,
            geom: MapGeom::default(),
            ctx: FinderCtx { start_x: -1, start_y: -1, dest_x: -1, dest_y: -1, info: 0 },
            cardinal_only: false,
            group: None,
            nodes: Vec::new(),
            last_node: None,
        }
    }

    /// C++ `FAStar::Initialize(columns, rows, wrapX, wrapY, callbacks...)` (CvMap::setup).
    pub fn initialize(&mut self, geom: MapGeom) {
        self.geom = geom;
        self.nodes = vec![PathNode::default(); geom.num_plots().max(0) as usize];
        self.last_node = None;
    }

    /// C++ `FAStar::SetData` for the path finder.
    pub fn set_group(&mut self, group: Option<GroupId>) {
        self.group = group;
    }

    /// C++ `FAStar::GeneratePath(iXstart, iYstart, iXdest, iYdest, bCardinalOnly, iInfo, bReuse)`
    /// with this finder's callbacks. A destination of (-1, -1) floods everything reachable.
    /// `reuse` keeps the previous search tree when start, info and data are unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn generate_path(
        &mut self,
        g: &Game,
        start_x: i32,
        start_y: i32,
        dest_x: i32,
        dest_y: i32,
        cardinal_only: bool,
        info: i32,
        reuse: bool,
    ) -> bool {
        todo!("FAStar::GeneratePath (A* over plots with the FinderKind callbacks)")
    }

    /// C++ `FAStar::ForceReset`.
    pub fn force_reset(&mut self) {
        for n in &mut self.nodes {
            *n = PathNode::default();
        }
        self.last_node = None;
    }

    /// C++ `FAStar::GetLastNode`.
    pub fn last_node(&self) -> Option<&PathNode> {
        self.last_node.map(|i| &self.nodes[i as usize])
    }

    /// Node by index.
    pub fn node(&self, i: u32) -> &PathNode {
        &self.nodes[i as usize]
    }

    /// The node's parent.
    pub fn parent_of(&self, n: &PathNode) -> Option<&PathNode> {
        n.parent.map(|i| &self.nodes[i as usize])
    }

    /// Logic of C++ `CvSelectionGroup::getPathFirstPlot`: the first step of the last path
    /// (the destination itself when it has no parent). `None` if there is no last node.
    pub fn path_first_xy(&self) -> Option<(i32, i32)> {
        let mut node = self.last_node()?;
        if node.parent.is_none() {
            return Some((node.x, node.y));
        }
        loop {
            let parent = self.parent_of(node)?;
            if parent.parent.is_none() {
                return Some((node.x, node.y));
            }
            node = parent;
        }
    }

    /// Logic of C++ `CvSelectionGroup::getPathEndTurnPlot`: the last plot of the path reached
    /// in the first turn (`m_iData2 == 1`).
    pub fn path_end_turn_xy(&self) -> Option<(i32, i32)> {
        let mut node = self.last_node()?;
        if node.parent.is_none() || node.data2 == 1 {
            return Some((node.x, node.y));
        }
        while let Some(parent) = self.parent_of(node) {
            if parent.data2 == 1 {
                return Some((parent.x, parent.y));
            }
            node = parent;
        }
        debug_assert!(false, "getPathEndTurnPlot: no node reached in the first turn");
        None
    }

    /// C++ `*piPathTurns = pNode->m_iData2` after `CvSelectionGroup::generatePath`.
    pub fn path_turns(&self) -> Option<i32> {
        self.last_node().map(|n| n.data2)
    }
}

/// C++ `stepCost`.
#[inline]
pub fn step_cost(parent: &PathNode, node: &PathNode, data: i32) -> i32 {
    1
}

/// C++ `stepAdd`: steps from the start in `m_iData1`.
pub fn step_add(parent: Option<&PathNode>, node: &mut PathNode, data: i32) -> i32 {
    if data == ASNC_INITIALADD {
        node.data1 = 0;
    } else {
        node.data1 = parent.expect("stepAdd: parent").data1 + 1;
    }
    debug_assert!(node.data1 >= 0, "node->m_iData1 is expected to be non-negative (invalid Index)");
    1
}

/// C++ `countPlotGroup` (the counter is the finder's `SetData` pointer).
pub fn count_plot_group(node: &PathNode, data: i32, count: &mut i32) -> i32 {
    if data == ASNL_ADDCLOSED {
        *count += 1;
    }
    1
}

impl Game {
    /// C++ `pathDestValid(iToX, iToY, pointer = group, finder)`.
    pub fn path_dest_valid(&self, to_x: i32, to_y: i32, group: GroupId, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::pathDestValid")
    }

    /// C++ `pathHeuristic`.
    pub fn path_heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.step_distance(from_x, from_y, to_x, to_y) * PATH_MOVEMENT_WEIGHT
    }

    /// C++ `pathCost(parent, node, data, pointer = group, finder)`.
    pub fn path_cost(&self, parent: &PathNode, node: &PathNode, data: i32, group: GroupId, finder: &FinderCtx) -> i32 {
        todo!("CvGameCoreUtils::pathCost")
    }

    /// C++ `pathValid(parent, node, data, pointer = group, finder)` (`parent` NULL = start node).
    pub fn path_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, group: GroupId, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::pathValid")
    }

    /// C++ `pathAdd(parent, node, data, pointer = group, finder)`: sets `node.data1` (moves left)
    /// and `node.data2` (turns).
    pub fn path_add(&self, parent: Option<&PathNode>, node: &mut PathNode, data: i32, group: GroupId, finder: &FinderCtx) -> i32 {
        todo!("CvGameCoreUtils::pathAdd")
    }

    /// C++ `stepDestValid(iToX, iToY, pointer, finder)`: same area as the start plot.
    pub fn step_dest_valid(&self, to_x: i32, to_y: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::stepDestValid")
    }

    /// C++ `stepHeuristic`.
    pub fn step_heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.step_distance(from_x, from_y, to_x, to_y)
    }

    /// C++ `stepValid(parent, node, ...)`: passable and in the parent's area.
    pub fn step_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::stepValid")
    }

    /// C++ `routeValid(parent, node, ...)` (info = player).
    pub fn route_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::routeValid")
    }

    /// C++ `borderValid(parent, node, ...)` (info = player).
    pub fn border_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::borderValid")
    }

    /// C++ `areaValid(parent, node, ...)`: same water/land status as the parent.
    pub fn area_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::areaValid")
    }

    /// C++ `joinArea(parent, node, data, ...)`: on `ASNL_ADDCLOSED` sets the plot's area to
    /// the finder info.
    pub fn join_area(&mut self, node: &PathNode, data: i32, finder: &FinderCtx) -> i32 {
        todo!("CvGameCoreUtils::joinArea")
    }

    /// C++ `plotGroupValid(parent, node, ...)` (info = player).
    pub fn plot_group_valid(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::plotGroupValid")
    }

    /// C++ `potentialIrrigation(parent, node, ...)`.
    pub fn potential_irrigation(&self, parent: Option<&PathNode>, node: &PathNode, data: i32, finder: &FinderCtx) -> bool {
        todo!("CvGameCoreUtils::potentialIrrigation")
    }

    /// C++ `checkFreshWater(parent, node, data, pointer = &bFoundFreshWater, ...)`.
    pub fn check_fresh_water(&self, node: &PathNode, data: i32, found_fresh_water: &mut bool) -> i32 {
        todo!("CvGameCoreUtils::checkFreshWater")
    }

    /// C++ `changeIrrigated(parent, node, data, pointer = &bIrrigated, ...)`.
    pub fn change_irrigated(&mut self, node: &PathNode, data: i32, irrigated: bool) -> i32 {
        todo!("CvGameCoreUtils::changeIrrigated")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 4x1 strip with a 3-step path 0 -> 1 -> 2 -> 3 (turns 1, 1, 2, 2).
    fn finder_with_path() -> PathFinder {
        let mut f = PathFinder::new(FinderKind::Path);
        f.initialize(MapGeom::new(4, 1, false, false));
        for i in 0..4u32 {
            f.nodes[i as usize].x = i as i32;
            f.nodes[i as usize].parent = if i == 0 { None } else { Some(i - 1) };
        }
        f.nodes[0].data2 = 1;
        f.nodes[1].data2 = 1;
        f.nodes[2].data2 = 2;
        f.nodes[3].data2 = 2;
        f.last_node = Some(3);
        f
    }

    #[test]
    fn path_walks() {
        let f = finder_with_path();
        assert_eq!(f.path_first_xy(), Some((1, 0)));
        assert_eq!(f.path_end_turn_xy(), Some((1, 0)));
        assert_eq!(f.path_turns(), Some(2));
        let mut g = finder_with_path();
        g.last_node = Some(0);
        assert_eq!(g.path_first_xy(), Some((0, 0)));
        assert_eq!(g.path_end_turn_xy(), Some((0, 0)));
        g.force_reset();
        assert_eq!(g.last_node(), None);
        assert_eq!(g.path_first_xy(), None);
    }

    #[test]
    fn step_callbacks() {
        let parent = PathNode { data1: 4, ..Default::default() };
        let mut node = PathNode::default();
        step_add(None, &mut node, ASNC_INITIALADD);
        assert_eq!(node.data1, 0);
        step_add(Some(&parent), &mut node, ASNC_NEWADD);
        assert_eq!(node.data1, 5);
        assert_eq!(step_cost(&parent, &node, 0), 1);
        let mut count = 0;
        count_plot_group(&node, ASNL_ADDOPEN, &mut count);
        count_plot_group(&node, ASNL_ADDCLOSED, &mut count);
        assert_eq!(count, 1);
        let ctx = FinderCtx { start_x: 1, start_y: 2, dest_x: 3, dest_y: 4, info: 0 };
        assert!(ctx.is_path_start(1, 2) && ctx.is_path_dest(3, 4) && !ctx.is_path_dest(1, 2));
    }
}
