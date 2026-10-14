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
//! search, so callers apply them after the search to [`PathFinder::closed_nodes`] (in the
//! order C++ notifies them), e.g. `self.join_area(&node, ASNL_ADDCLOSED, &ctx)`. The same holds
//! for the read-only list callbacks `countPlotGroup` and `checkFreshWater`.
//!
//! # The search (`FAStar::GeneratePath`)
//!
//! FAStar's source is not available; FAStarNode.h shows it is the "A* Explorer" algorithm of
//! *AI Game Programming Wisdom* with one node per plot: an open and a closed list, parent links,
//! up to 8 child links per node, and a stack to propagate a lower cost found for a closed node
//! to its descendants. This port implements that algorithm:
//!
//! - The start node is checked with `Valid(NULL, start)`, gets known cost 0 and
//!   `NotifyChild(NULL, start, ASNC_INITIALADD)` (`pathAdd`/`stepAdd` set `m_iData1/2`).
//! - A destination (not `(-1, -1)`) must pass the `DestValid` callback first. The best open node
//!   is moved to the closed list; the search succeeds when that node is the destination
//!   (`GetLastNode`), and fails when the open list runs dry. Flood fills (destination
//!   `(-1, -1)`) close every reachable node and return `false`.
//! - Children are the 8 neighbours (4 with `bCardinalOnly`), with map wrapping, that pass
//!   `Valid(parent, child)`; their known cost is `parent + Cost(parent, child)`. A new child gets
//!   `ASNC_NEWADD`; a cheaper route to an open node re-parents it (`ASNC_OPENADD_UP`), to a closed
//!   node re-parents it (`ASNC_CLOSEDADD_UP`) and propagates the new cost to its recorded
//!   children (`ASNC_PARENTADD_UP`). The `NotifyChild` callbacks recompute `m_iData1/2` from the
//!   new parent each time.
//! - NULL callbacks (route/border/area/plot group/irrigation finders): every destination is
//!   valid, each step costs 1, and the heuristic is `stepDistance` (route/border finders) or 0
//!   (flood fills). Only the result (reachable or not, or the set of closed nodes) of those
//!   finders is used by the DLL, so these choices do not change any observable behaviour.
//!
//! **Tie-breaking (deterministic; FAStar's own order is unknowable).** The open node with the
//! lowest total cost (f) is expanded first; among equal f the node opened (or re-opened with a
//! lower cost) most recently wins, as in A* Explorer's sorted open list, which inserts a node in
//! front of the nodes of equal f. Children are generated in `DirectionType` order (N, NE, E,
//! SE, S, SW, W, NW; cardinal: N, E, S, W). Paths of equal cost may therefore differ from the
//! exe's.
//!
//! **Reuse (`bReuse`).** With `reuse` and the same start, info, cardinal flag and (path finder)
//! selection group as the kept search, the search tree is kept: a destination that is already
//! closed returns at once; otherwise the kept nodes get their heuristic recomputed for the new
//! destination, the node where the previous search stopped (closed, not yet expanded) is
//! expanded, and the search continues, so the result equals a fresh search up to tie-breaking.
//! As in C++, the kept tree is not refreshed when the game changes; callers reset it
//! (`ForceReset`: `CvMap::resetPathDistance`, `CvSelectionGroup::resetPath`).
//!
//! Efficiency: node storage is allocated once per map (`initialize`); a search only resets the
//! nodes the previous search touched; the open list is a binary heap with lazy deletion.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

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

/// C++ `#define DANGER_RANGE (4)` (CvPlayerAI.cpp): default range of `AI_getPlotDanger`.
pub const DANGER_RANGE: i32 = 4;

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

/// Child generation order: `DirectionType` order (N, NE, E, SE, S, SW, W, NW), as `(dx, dy)`.
const ALL_STEPS: [(i32, i32); 8] = [(0, 1), (1, 1), (1, 0), (1, -1), (0, -1), (-1, -1), (-1, 0), (-1, 1)];
/// Child generation order with `bCardinalOnly`: `CardinalDirectionType` order (N, E, S, W).
const CARDINAL_STEPS: [(i32, i32); 4] = [(0, 1), (1, 0), (0, -1), (-1, 0)];

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

/// An open-list entry (lazy deletion: only the entry whose `seq` is the node's current one is
/// live).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct OpenEntry {
    total_cost: i32,
    seq: u32,
    node: u32,
}

impl Ord for OpenEntry {
    /// `BinaryHeap` pops the greatest entry: the lowest total cost, then the most recently
    /// (re)opened node (see the module docs).
    fn cmp(&self, other: &Self) -> Ordering {
        other.total_cost.cmp(&self.total_cost).then_with(|| self.seq.cmp(&other.seq))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What a kept search tree was built for (C++ `bReuse` only continues the same search).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
struct SearchKey {
    start_x: i32,
    start_y: i32,
    info: i32,
    cardinal_only: bool,
    group: Option<GroupId>,
}

/// The FAStar callbacks of one finder (C++ `Initialize(.., DestValidFunc, HeuristicFunc,
/// CostFunc, ValidFunc, NotifyChildFunc, NotifyListFunc, pData)`). The defaults are this port's
/// NULL-callback behaviour (see the module docs). NotifyList callbacks are applied by the
/// callers after the search (see the module docs).
pub(crate) trait FinderCallbacks {
    /// C++ `DestValidFunc`.
    fn dest_valid(&self, _ctx: &FinderCtx, _x: i32, _y: i32) -> bool {
        true
    }

    /// C++ `HeuristicFunc` (only called when there is a destination).
    fn heuristic(&self, _from_x: i32, _from_y: i32, _to_x: i32, _to_y: i32) -> i32 {
        0
    }

    /// C++ `CostFunc`.
    fn cost(&self, _ctx: &FinderCtx, _parent: &PathNode, _node: &PathNode) -> i32 {
        1
    }

    /// C++ `ValidFunc` (`parent` NULL = the start node).
    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool;

    /// C++ `NotifyChildFunc` (`ASNC_*`).
    fn notify_child(&self, _ctx: &FinderCtx, _parent: Option<&PathNode>, _node: &mut PathNode, _data: i32) {}
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
    /// One node per plot (index = plot index). Owned by the finder: reset lazily (only the
    /// nodes a search touched are cleared before the next search).
    pub nodes: Vec<PathNode>,
    /// C++ `GetLastNode`: the destination node of the last successful search.
    pub last_node: Option<u32>,
    /// Open list (binary heap with lazy deletion).
    open: BinaryHeap<OpenEntry>,
    /// Per node: sequence number of its live open-list entry.
    open_seq: Vec<u32>,
    next_seq: u32,
    /// Nodes changed since the last reset (to reset only those).
    touched: Vec<u32>,
    /// Closed nodes in closing order (C++ `ASNL_ADDCLOSED` notifications).
    closed_order: Vec<u32>,
    /// C++ `m_pStack`: nodes whose lower known cost is being propagated to their children.
    stack: Vec<u32>,
    /// The search the nodes hold (for `reuse`); None after a reset.
    search_key: Option<SearchKey>,
    /// The destination where the kept search stopped: closed but not yet expanded.
    unexpanded: Option<u32>,
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
            open: BinaryHeap::new(),
            open_seq: Vec::new(),
            next_seq: 0,
            touched: Vec::new(),
            closed_order: Vec::new(),
            stack: Vec::new(),
            search_key: None,
            unexpanded: None,
        }
    }

    /// C++ `FAStar::Initialize(columns, rows, wrapX, wrapY, callbacks...)` (CvMap::setup).
    /// The callbacks are those of the finder's [`FinderKind`]. `generate_path` calls this itself
    /// when the game's map size differs from `geom`.
    pub fn initialize(&mut self, geom: MapGeom) {
        self.geom = geom;
        let num_plots = geom.num_plots().max(0) as usize;
        let width = geom.width.max(1);
        self.nodes = (0..num_plots as i32).map(|i| PathNode { x: i % width, y: i / width, ..PathNode::default() }).collect();
        self.open_seq = vec![0; num_plots];
        self.open.clear();
        self.next_seq = 0;
        self.touched.clear();
        self.closed_order.clear();
        self.stack.clear();
        self.search_key = None;
        self.unexpanded = None;
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
        let geom = g.map_geom();
        if self.geom != geom || self.nodes.len() != geom.num_plots().max(0) as usize {
            self.initialize(geom);
        }

        match self.kind {
            FinderKind::Path => {
                // C++ dereferences the SetData pointer in every callback.
                let group = self.group.expect("FAStar path finder: SetData (selection group) not set");
                self.search(&PathCallbacks { g, group }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, Some(group))
            }
            FinderKind::Step => self.search(&StepCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None),
            FinderKind::Route => self.search(&RouteCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None),
            FinderKind::Border => self.search(&BorderCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None),
            FinderKind::Area => self.search(&AreaCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None),
            FinderKind::PlotGroup => {
                self.search(&PlotGroupCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None)
            }
            FinderKind::Irrigation => {
                self.search(&IrrigationCallbacks { g }, start_x, start_y, dest_x, dest_y, cardinal_only, info, reuse, None)
            }
        }
    }

    /// C++ `FAStar::ForceReset`.
    pub fn force_reset(&mut self) {
        self.clear_search();
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

    /// The closed nodes of the kept search, in closing order: the nodes (and order) for which
    /// C++ calls the NotifyList callback with `ASNL_ADDCLOSED` (`joinArea`, `countPlotGroup`,
    /// `checkFreshWater`, `changeIrrigated`).
    pub fn closed_nodes(&self) -> impl Iterator<Item = &PathNode> + '_ {
        self.closed_order.iter().map(move |&i| &self.nodes[i as usize])
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

    // ---------------------------------------------------------------------------------------
    // The A* (FAStar::GeneratePath)
    // ---------------------------------------------------------------------------------------

    /// Resets the nodes the last search touched (C++ `FAStarNode::clear`, which keeps the
    /// coordinates) and the lists.
    fn clear_search(&mut self) {
        for &n in &self.touched {
            let node = &mut self.nodes[n as usize];
            *node = PathNode { x: node.x, y: node.y, ..PathNode::default() };
        }
        self.touched.clear();
        self.open.clear();
        self.closed_order.clear();
        self.stack.clear();
        self.next_seq = 0;
        self.search_key = None;
        self.unexpanded = None;
        self.last_node = None;
    }

    /// `FAStar::GeneratePath` over the callbacks `cb` (see the module docs). `group` is the
    /// path finder's `SetData` (part of the reuse key).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn search<C: FinderCallbacks>(
        &mut self,
        cb: &C,
        start_x: i32,
        start_y: i32,
        dest_x: i32,
        dest_y: i32,
        cardinal_only: bool,
        info: i32,
        reuse: bool,
        group: Option<GroupId>,
    ) -> bool {
        debug_assert!(self.geom.is_plot(start_x, start_y), "GeneratePath: start ({start_x}, {start_y}) off the map");
        let key = SearchKey { start_x, start_y, info, cardinal_only, group };
        let reusing = reuse && self.search_key == Some(key);
        if !reusing {
            self.clear_search();
        }

        self.ctx = FinderCtx { start_x, start_y, dest_x, dest_y, info };
        self.cardinal_only = cardinal_only;
        self.last_node = None;

        let dest = if self.geom.is_plot(dest_x, dest_y) {
            Some(self.geom.plot_num(dest_x, dest_y) as u32)
        } else {
            debug_assert!(dest_x == -1 && dest_y == -1, "GeneratePath: destination ({dest_x}, {dest_y}) off the map");
            None
        };

        if dest.is_some() && !cb.dest_valid(&self.ctx, dest_x, dest_y) {
            return false;
        }

        if reusing {
            if let Some(d) = dest {
                if self.nodes[d as usize].list == Some(PathListType::Closed) {
                    self.last_node = Some(d);
                    return true;
                }
            }
            self.retarget(cb, dest.map(|_| (dest_x, dest_y)));
            // The previous search stopped at its destination without expanding it.
            if let Some(n) = self.unexpanded.take() {
                self.expand(cb, n, dest.is_some());
            }
        } else {
            let s = self.geom.plot_num(start_x, start_y) as u32;
            if !cb.valid(&self.ctx, None, &self.nodes[s as usize]) {
                return false;
            }
            let h = if dest.is_some() { cb.heuristic(start_x, start_y, dest_x, dest_y) } else { 0 };
            {
                let node = &mut self.nodes[s as usize];
                node.known_cost = 0;
                node.heuristic_cost = h;
                node.total_cost = h;
                node.list = Some(PathListType::Open);
            }
            self.touched.push(s);
            self.notify(cb, None, s, ASNC_INITIALADD);
            self.push_open(s);
            self.search_key = Some(key);
        }

        while let Some(n) = self.pop_open() {
            self.nodes[n as usize].list = Some(PathListType::Closed);
            self.closed_order.push(n);

            if Some(n) == dest {
                self.last_node = Some(n);
                self.unexpanded = Some(n);
                return true;
            }

            self.expand(cb, n, dest.is_some());
        }

        false
    }

    /// Reuse with a new destination (`None` = flood): recompute the heuristic of the kept nodes
    /// and rebuild the open list (the open nodes keep their relative order among equal totals).
    fn retarget<C: FinderCallbacks>(&mut self, cb: &C, dest: Option<(i32, i32)>) {
        self.open.clear();
        for i in 0..self.touched.len() {
            let n = self.touched[i] as usize;
            let node = &mut self.nodes[n];
            node.heuristic_cost = match dest {
                Some((dest_x, dest_y)) => cb.heuristic(node.x, node.y, dest_x, dest_y),
                None => 0,
            };
            node.total_cost = node.known_cost.wrapping_add(node.heuristic_cost);
            if node.list == Some(PathListType::Open) {
                self.open.push(OpenEntry { total_cost: node.total_cost, seq: self.open_seq[n], node: n as u32 });
            }
        }
    }

    /// C++ `CreateChildren`: link every valid neighbour of `n`.
    fn expand<C: FinderCallbacks>(&mut self, cb: &C, n: u32, has_dest: bool) {
        let (x, y) = (self.nodes[n as usize].x, self.nodes[n as usize].y);
        let steps: &[(i32, i32)] = if self.cardinal_only { &CARDINAL_STEPS } else { &ALL_STEPS };
        for &(dx, dy) in steps {
            let Some(c) = self.geom.plot(x + dx, y + dy) else {
                continue;
            };
            let c = c.0;
            if !cb.valid(&self.ctx, Some(&self.nodes[n as usize]), &self.nodes[c as usize]) {
                continue;
            }
            self.link_child(cb, n, c, has_dest);
        }
    }

    /// C++ `LinkChild(parent, child)`.
    fn link_child<C: FinderCallbacks>(&mut self, cb: &C, p: u32, c: u32, has_dest: bool) {
        let cost = cb.cost(&self.ctx, &self.nodes[p as usize], &self.nodes[c as usize]);
        let known = self.nodes[p as usize].known_cost.wrapping_add(cost);

        match self.nodes[c as usize].list {
            Some(PathListType::Open) => {
                self.add_child_link(p, c);
                if known < self.nodes[c as usize].known_cost {
                    self.reparent(cb, p, c, known, ASNC_OPENADD_UP);
                    self.push_open(c);
                }
            }
            Some(PathListType::Closed) => {
                self.add_child_link(p, c);
                if known < self.nodes[c as usize].known_cost {
                    self.reparent(cb, p, c, known, ASNC_CLOSEDADD_UP);
                    self.update_parents(cb, c);
                }
            }
            None => {
                let h = if has_dest {
                    let node = &self.nodes[c as usize];
                    cb.heuristic(node.x, node.y, self.ctx.dest_x, self.ctx.dest_y)
                } else {
                    0
                };
                {
                    let node = &mut self.nodes[c as usize];
                    node.parent = Some(p);
                    node.known_cost = known;
                    node.heuristic_cost = h;
                    node.total_cost = known.wrapping_add(h);
                    node.list = Some(PathListType::Open);
                }
                self.touched.push(c);
                self.push_open(c);
                self.add_child_link(p, c);
                self.notify(cb, Some(p), c, ASNC_NEWADD);
            }
        }
    }

    /// C++ `UpdateParents(node)`: a closed node got a lower known cost; propagate it to its
    /// children (and theirs) through the stack.
    fn update_parents<C: FinderCallbacks>(&mut self, cb: &C, n: u32) {
        self.nodes[n as usize].on_stack = true;
        self.stack.push(n);
        while let Some(par) = self.stack.pop() {
            self.nodes[par as usize].on_stack = false;
            let num_children = self.nodes[par as usize].num_children as usize;
            for i in 0..num_children {
                let Some(kid) = self.nodes[par as usize].children[i] else {
                    continue;
                };
                let cost = cb.cost(&self.ctx, &self.nodes[par as usize], &self.nodes[kid as usize]);
                let known = self.nodes[par as usize].known_cost.wrapping_add(cost);
                if known < self.nodes[kid as usize].known_cost {
                    self.reparent(cb, par, kid, known, ASNC_PARENTADD_UP);
                    if self.nodes[kid as usize].list == Some(PathListType::Open) {
                        self.push_open(kid);
                    }
                    if !self.nodes[kid as usize].on_stack {
                        self.nodes[kid as usize].on_stack = true;
                        self.stack.push(kid);
                    }
                }
            }
        }
    }

    /// Gives `c` the parent `p` with known cost `known` and notifies the callbacks.
    fn reparent<C: FinderCallbacks>(&mut self, cb: &C, p: u32, c: u32, known: i32, data: i32) {
        {
            let node = &mut self.nodes[c as usize];
            node.parent = Some(p);
            node.known_cost = known;
            node.total_cost = known.wrapping_add(node.heuristic_cost);
        }
        self.notify(cb, Some(p), c, data);
    }

    /// C++ `NotifyChild(parent, node, data)`.
    fn notify<C: FinderCallbacks>(&mut self, cb: &C, parent: Option<u32>, c: u32, data: i32) {
        let ctx = &self.ctx;
        match parent {
            None => cb.notify_child(ctx, None, &mut self.nodes[c as usize], data),
            Some(p) => {
                let (parent_node, node) = parent_and_child(&mut self.nodes, p as usize, c as usize);
                cb.notify_child(ctx, Some(parent_node), node, data);
            }
        }
    }

    /// Records `c` as a child of `p` (C++ `m_apChildren`).
    fn add_child_link(&mut self, p: u32, c: u32) {
        let node = &mut self.nodes[p as usize];
        let i = node.num_children as usize;
        debug_assert!(i < 8, "FAStarNode: more than 8 children");
        if i < 8 {
            node.children[i] = Some(c);
            node.num_children += 1;
        }
    }

    fn push_open(&mut self, n: u32) {
        let seq = self.next_seq;
        self.next_seq = self.next_seq.wrapping_add(1);
        self.open_seq[n as usize] = seq;
        self.open.push(OpenEntry { total_cost: self.nodes[n as usize].total_cost, seq, node: n });
    }

    /// C++ `GetBest`: the best live open node.
    fn pop_open(&mut self) -> Option<u32> {
        while let Some(e) = self.open.pop() {
            let n = e.node as usize;
            if self.nodes[n].list == Some(PathListType::Open) && self.open_seq[n] == e.seq {
                return Some(e.node);
            }
        }
        None
    }
}

/// Shared parent and mutable child out of the node array (`p != c`).
fn parent_and_child(nodes: &mut [PathNode], p: usize, c: usize) -> (&PathNode, &mut PathNode) {
    assert_ne!(p, c, "FAStar: a node cannot be its own parent");
    if p < c {
        let (left, right) = nodes.split_at_mut(c);
        (&left[p], &mut right[0])
    } else {
        let (left, right) = nodes.split_at_mut(p);
        (&right[0], &mut left[c])
    }
}

// -------------------------------------------------------------------------------------------
// Callback sets (C++ `CvMap::setup` / `CvPlot::updateIrrigated` `Initialize` calls)
// -------------------------------------------------------------------------------------------

/// `GC.getPathFinder()`: pathDestValid, pathHeuristic, pathCost, pathValid, pathAdd.
struct PathCallbacks<'a> {
    g: &'a Game,
    group: GroupId,
}

impl FinderCallbacks for PathCallbacks<'_> {
    fn dest_valid(&self, ctx: &FinderCtx, x: i32, y: i32) -> bool {
        self.g.path_dest_valid(x, y, self.group, ctx)
    }

    fn heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.g.path_heuristic(from_x, from_y, to_x, to_y)
    }

    fn cost(&self, ctx: &FinderCtx, parent: &PathNode, node: &PathNode) -> i32 {
        self.g.path_cost(parent, node, 0, self.group, ctx)
    }

    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.path_valid(parent, node, 0, self.group, ctx)
    }

    fn notify_child(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &mut PathNode, data: i32) {
        self.g.path_add(parent, node, data, self.group, ctx);
    }
}

/// `GC.getStepFinder()`: stepDestValid, stepHeuristic, stepCost, stepValid, stepAdd.
struct StepCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for StepCallbacks<'_> {
    fn dest_valid(&self, ctx: &FinderCtx, x: i32, y: i32) -> bool {
        self.g.step_dest_valid(x, y, ctx)
    }

    fn heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.g.step_heuristic(from_x, from_y, to_x, to_y)
    }

    fn cost(&self, _ctx: &FinderCtx, parent: &PathNode, node: &PathNode) -> i32 {
        step_cost(parent, node, 0)
    }

    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.step_valid(parent, node, 0, ctx)
    }

    fn notify_child(&self, _ctx: &FinderCtx, parent: Option<&PathNode>, node: &mut PathNode, data: i32) {
        step_add(parent, node, data);
    }
}

/// `GC.getRouteFinder()`: routeValid only.
struct RouteCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for RouteCallbacks<'_> {
    fn heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.g.step_distance(from_x, from_y, to_x, to_y)
    }

    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.route_valid(parent, node, 0, ctx)
    }
}

/// `GC.getBorderFinder()`: borderValid only.
struct BorderCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for BorderCallbacks<'_> {
    fn heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.g.step_distance(from_x, from_y, to_x, to_y)
    }

    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.border_valid(parent, node, 0, ctx)
    }
}

/// `GC.getAreaFinder()`: areaValid (+ joinArea, applied by the caller).
struct AreaCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for AreaCallbacks<'_> {
    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.area_valid(parent, node, 0, ctx)
    }
}

/// `GC.getPlotGroupFinder()`: plotGroupValid (+ countPlotGroup, applied by the caller).
struct PlotGroupCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for PlotGroupCallbacks<'_> {
    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.plot_group_valid(parent, node, 0, ctx)
    }
}

/// `CvPlot::updateIrrigated`'s finder: potentialIrrigation (+ checkFreshWater or
/// changeIrrigated, applied by the caller).
struct IrrigationCallbacks<'a> {
    g: &'a Game,
}

impl FinderCallbacks for IrrigationCallbacks<'_> {
    fn valid(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
        self.g.potential_irrigation(parent, node, 0, ctx)
    }
}

// -------------------------------------------------------------------------------------------
// CvGameCoreUtils.cpp callbacks
// -------------------------------------------------------------------------------------------

/// C++ `stepCost`.
#[inline]
pub fn step_cost(_parent: &PathNode, _node: &PathNode, _data: i32) -> i32 {
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
pub fn count_plot_group(_node: &PathNode, data: i32, count: &mut i32) -> i32 {
    if data == ASNL_ADDCLOSED {
        *count += 1;
    }
    1
}

/// C++ `CvPlayerAI::AI_getPlotDanger(pPlot, iRange, bTestMoves)` (C++ defaults `iRange = -1`
/// = `DANGER_RANGE`, `bTestMoves = true`): enemy units that can reach `plot`, plus border danger
/// for AI players outside cities. An AI query that rules code reads (pathfinding of AI-controlled
/// groups, `CvSelectionGroup::autoMission`); not declared elsewhere, so ported here.
pub(crate) fn ai_get_plot_danger(g: &Game, player: PlayerId, plot: PlotIdx, range: i32, test_moves: bool) -> i32 {
    let team = g.players[player].team;
    let plot_area = g.map.plots[plot].area;
    let (px, py) = g.map.xy(plot);

    let mut count = 0;
    let mut border_danger = 0;

    let range = if range == -1 { DANGER_RANGE } else { range };

    for dx in -range..=range {
        for dy in -range..=range {
            let Some(loop_plot) = g.map.plot_xy(px + dx, py + dy) else {
                continue;
            };

            if g.map.plots[loop_plot].area == plot_area {
                let (lx, ly) = g.map.xy(loop_plot);
                let distance = g.step_distance(px, py, lx, ly);
                if g.at_war_opt(g.plot_get_team(loop_plot), Some(team)) {
                    if distance == 1 {
                        border_danger += 1;
                    } else if distance == 2 && g.plot_is_route(loop_plot) {
                        border_danger += 1;
                    }
                }

                for &loop_unit in &g.map.plots[loop_plot].units {
                    if g.unit_is_enemy(loop_unit, Some(team), None) {
                        if g.unit_can_attack(loop_unit) {
                            if !g.unit_is_invisible(loop_unit, team, true) {
                                if g.unit_can_move_or_attack_into(loop_unit, plot, false) {
                                    if !test_moves {
                                        count += 1;
                                    } else {
                                        let mut danger_range = g.unit_base_moves(loop_unit);
                                        danger_range += if g.plot_is_valid_route(loop_plot, loop_unit) { 1 } else { 0 };
                                        if danger_range >= distance {
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    if border_danger > 0 {
        if !g.players[player].human && g.map.plots[plot].plot_city.is_none() {
            count += border_danger;
        }
    }

    count
}

impl Game {
    /// C++ `pathDestValid(iToX, iToY, pointer = group, finder)`.
    pub fn path_dest_valid(&self, to_x: i32, to_y: i32, group: GroupId, finder: &FinderCtx) -> bool {
        let to_plot = self.map.plot_soren(to_x, to_y).expect("pathDestValid: ToPlot");

        if self.group_at_plot(group, Some(to_plot)) {
            return true;
        }

        let domain = self.group_get_domain_type(group);
        if domain == Some(DomainType::DOMAIN_IMMOBILE) {
            return false;
        }

        let ai_control = self.group_ai_is_controlled(group);

        if ai_control {
            if finder.info & MOVE_IGNORE_DANGER == 0 {
                if !self.group_can_fight(group) && !self.group_always_invisible(group) {
                    let head_owner = self.group_get_head_owner(group).expect("pathDestValid: group without a head unit");
                    if ai_get_plot_danger(self, head_owner, to_plot, -1, true) > 0 {
                        return false;
                    }
                }
            }

            if domain == Some(DomainType::DOMAIN_LAND) {
                let group_area = self.group_area(group);
                if self.map.plots[to_plot].area != group_area {
                    if !group_area.is_some_and(|a| self.plot_is_adjacent_to_area(to_plot, a)) {
                        return false;
                    }
                }
            }
        }

        let head_team = self.group_get_head_team(group);
        if ai_control || head_team.is_some_and(|t| self.map.plots[to_plot].revealed[t]) {
            let declare_war = self.group_ai_is_declare_war(group, Some(to_plot)) || (finder.info & MOVE_DECLARE_WAR) != 0;

            if self.group_is_amphib_plot(group, to_plot) {
                for &loop_unit1 in &self.groups[group].units {
                    if self.units[loop_unit1].cargo > 0 && self.unit_domain_cargo(loop_unit1) == Some(DomainType::DOMAIN_LAND) {
                        let mut valid = false;

                        let unit1_plot = self.unit_plot(loop_unit1);
                        for &loop_unit2 in &self.map.plots[unit1_plot].units {
                            if self.units[loop_unit2].transport_unit == Some(loop_unit1) {
                                if self.unit_is_group_head(loop_unit2) {
                                    let cargo_group = self.units[loop_unit2].group.expect("pathDestValid: group head without a group");
                                    if self.group_can_move_or_attack_into(cargo_group, to_plot, declare_war) {
                                        valid = true;
                                        break;
                                    }
                                }
                            }
                        }

                        if valid {
                            return true;
                        }
                    }
                }

                return false;
            } else if !self.group_can_move_or_attack_into(group, to_plot, declare_war) {
                return false;
            }
        }

        true
    }

    /// C++ `pathHeuristic`.
    pub fn path_heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.step_distance(from_x, from_y, to_x, to_y) * PATH_MOVEMENT_WEIGHT
    }

    /// C++ `pathCost(parent, node, data, pointer = group, finder)`.
    pub fn path_cost(&self, parent: &PathNode, node: &PathNode, _data: i32, group: GroupId, finder: &FinderCtx) -> i32 {
        let d = self.d;
        let from_plot = self.map.plot_soren(parent.x, parent.y).expect("pathCost: FromPlot");
        let to_plot = self.map.plot_soren(node.x, node.y).expect("pathCost: ToPlot");

        let mut worst_cost = MAX_INT;
        let mut worst_moves_left = MAX_INT;
        let mut worst_max = MAX_INT;

        // `AI_isControlled()` and `IsPathDest` do not depend on the unit (C++ asks per unit).
        let ai_control = self.group_ai_is_controlled(group);
        let is_path_dest = finder.is_path_dest(node.x, node.y);
        let to_team = self.plot_get_team(to_plot);

        for &loop_unit in &self.groups[group].units {
            debug_assert!(self.unit_get_domain_type(loop_unit) != DomainType::DOMAIN_AIR);

            let max = if parent.data1 > 0 { parent.data1 } else { self.unit_max_moves(loop_unit) };

            let mut cost = self.plot_movement_cost(to_plot, loop_unit, from_plot);

            let moves_left = 0.max(max - cost);

            if moves_left <= worst_moves_left {
                if moves_left < worst_moves_left || max <= worst_max {
                    let unit_team = self.unit_get_team(loop_unit);

                    if moves_left == 0 {
                        cost = PATH_MOVEMENT_WEIGHT * max;

                        if to_team != Some(unit_team) {
                            cost += PATH_TERRITORY_WEIGHT;
                        }

                        // Damage caused by features (mods)
                        if d.defines.path_damage_weight != 0 {
                            if let Some(feature) = self.map.plots[to_plot].feature_type {
                                cost += (d.defines.path_damage_weight * 0.max(d.features[feature].turn_damage)) / d.defines.max_hit_points;
                            }

                            if self.plot_get_extra_move_path_cost(to_plot) > 0 {
                                cost += PATH_MOVEMENT_WEIGHT * self.plot_get_extra_move_path_cost(to_plot);
                            }
                        }
                    } else {
                        cost *= PATH_MOVEMENT_WEIGHT;
                    }

                    if self.unit_can_fight(loop_unit) {
                        let no_defensive_bonus = self.unit_no_defensive_bonus(loop_unit);

                        if moves_left == 0 {
                            let defense = if no_defensive_bonus { 0 } else { self.plot_defense_modifier(to_plot, Some(unit_team), false) };
                            cost += PATH_DEFENSE_WEIGHT * 0.max(200 - defense);
                        }

                        if ai_control {
                            if self.unit_can_attack(loop_unit) {
                                if is_path_dest {
                                    if self.plot_is_visible_enemy_defender(to_plot, loop_unit) {
                                        let defense = if no_defensive_bonus {
                                            0
                                        } else {
                                            self.plot_defense_modifier(from_plot, Some(unit_team), false)
                                        };
                                        cost += PATH_DEFENSE_WEIGHT * 0.max(200 - defense);

                                        if self.map.plots[from_plot].plot_city.is_none() {
                                            cost += PATH_CITY_WEIGHT;
                                        }

                                        if self.plot_is_river_crossing(from_plot, self.direction_xy_plots(from_plot, to_plot)) {
                                            if !self.unit_is_river(loop_unit) {
                                                cost += PATH_RIVER_WEIGHT * -d.defines.river_attack_modifier;
                                                cost += PATH_MOVEMENT_WEIGHT * moves_left;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if cost < worst_cost {
                        worst_cost = cost;
                        worst_moves_left = moves_left;
                        worst_max = max;
                    }
                }
            }
        }

        debug_assert!(worst_cost != MAX_INT);

        // C++ int arithmetic (wraps for an empty group, which C++ asserts against).
        worst_cost = worst_cost.wrapping_add(PATH_STEP_WEIGHT);

        if parent.x != node.x && parent.y != node.y {
            worst_cost = worst_cost.wrapping_add(PATH_STRAIGHT_WEIGHT);
        }

        debug_assert!(worst_cost > 0);

        worst_cost
    }

    /// C++ `pathValid(parent, node, data, pointer = group, finder)` (`parent` NULL = start node).
    pub fn path_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, group: GroupId, finder: &FinderCtx) -> bool {
        let Some(parent) = parent else {
            return true;
        };

        let from_plot = self.map.plot_soren(parent.x, parent.y).expect("pathValid: FromPlot");
        let to_plot = self.map.plot_soren(node.x, node.y).expect("pathValid: ToPlot");

        // XXX might want to take this out...
        if self.group_get_domain_type(group) == Some(DomainType::DOMAIN_SEA) {
            if self.plot_is_water(from_plot) && self.plot_is_water(to_plot) {
                let corner1 = self.map.plot_xy(parent.x, node.y).expect("pathValid: diagonal corner");
                let corner2 = self.map.plot_xy(node.x, parent.y).expect("pathValid: diagonal corner");
                if !self.plot_is_water(corner1) && !self.plot_is_water(corner2) {
                    return false;
                }
            }
        }

        if self.group_at_plot(group, Some(from_plot)) {
            return true;
        }

        let head_team = self.group_get_head_team(group);
        let from_revealed = head_team.is_some_and(|t| self.map.plots[from_plot].revealed[t]);

        if finder.info & MOVE_SAFE_TERRITORY != 0 {
            if !from_revealed {
                return false;
            }

            if self.plot_is_owned(from_plot) {
                if self.plot_get_team(from_plot) != head_team {
                    return false;
                }
            }
        }

        if finder.info & MOVE_NO_ENEMY_TERRITORY != 0 {
            if self.plot_is_owned(from_plot) {
                if self.at_war_opt(self.plot_get_team(from_plot), head_team) {
                    return false;
                }
            }
        }

        let ai_control = self.group_ai_is_controlled(group);

        if ai_control {
            if parent.data2 > 1 || parent.data1 == 0 {
                if finder.info & MOVE_IGNORE_DANGER == 0 {
                    if !self.group_can_fight(group) && !self.group_always_invisible(group) {
                        let head_owner = self.group_get_head_owner(group).expect("pathValid: group without a head unit");
                        if ai_get_plot_danger(self, head_owner, from_plot, -1, true) > 0 {
                            return false;
                        }
                    }
                }
            }
        }

        if ai_control || from_revealed {
            if finder.info & MOVE_THROUGH_ENEMY != 0 {
                if !self.group_can_move_or_attack_into(group, from_plot, false) {
                    return false;
                }
            } else if !self.group_can_move_through(group, from_plot) {
                return false;
            }
        }

        true
    }

    /// C++ `pathAdd(parent, node, data, pointer = group, finder)`: sets `node.data1` (moves left)
    /// and `node.data2` (turns).
    pub fn path_add(&self, parent: Option<&PathNode>, node: &mut PathNode, data: i32, group: GroupId, finder: &FinderCtx) -> i32 {
        debug_assert!(!self.groups[group].units.is_empty());

        let mut turns = 1;
        let mut moves = MAX_INT;

        if data == ASNC_INITIALADD {
            let max_moves = finder.info & MOVE_MAX_MOVES != 0;
            if max_moves {
                moves = 0;
            }

            for &loop_unit in &self.groups[group].units {
                if max_moves {
                    moves = moves.max(self.unit_max_moves(loop_unit));
                } else {
                    moves = moves.min(self.unit_moves_left(loop_unit));
                }
            }
        } else {
            let parent = parent.expect("pathAdd: parent");
            let from_plot = self.map.plot_soren(parent.x, parent.y).expect("FromPlot is not assigned a valid value");
            let to_plot = self.map.plot_soren(node.x, node.y).expect("ToPlot is not assigned a valid value");

            let start_moves = parent.data1;
            turns = parent.data2;
            if start_moves == 0 {
                turns += 1;
            }

            for &loop_unit in &self.groups[group].units {
                let mut unit_moves = if start_moves == 0 { self.unit_max_moves(loop_unit) } else { start_moves };
                unit_moves -= self.plot_movement_cost(to_plot, loop_unit, from_plot);
                unit_moves = unit_moves.max(0);

                moves = moves.min(unit_moves);
            }
        }

        debug_assert!(moves >= 0, "iMoves is expected to be non-negative (invalid Index)");

        node.data1 = moves;
        node.data2 = turns;

        1
    }

    /// C++ `stepDestValid(iToX, iToY, pointer, finder)`: same area as the start plot.
    pub fn step_dest_valid(&self, to_x: i32, to_y: i32, finder: &FinderCtx) -> bool {
        let from_plot = self.map.plot_soren(finder.start_x, finder.start_y).expect("stepDestValid: FromPlot");
        let to_plot = self.map.plot_soren(to_x, to_y).expect("stepDestValid: ToPlot");

        self.map.plots[from_plot].area == self.map.plots[to_plot].area
    }

    /// C++ `stepHeuristic`.
    pub fn step_heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
        self.step_distance(from_x, from_y, to_x, to_y)
    }

    /// C++ `stepValid(parent, node, ...)`: passable and in the parent's area.
    pub fn step_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, _finder: &FinderCtx) -> bool {
        let Some(parent) = parent else {
            return true;
        };

        let new_plot = self.map.plot_soren(node.x, node.y).expect("stepValid: NewPlot");

        if self.plot_is_impassable(new_plot) {
            return false;
        }

        let parent_plot = self.map.plot_soren(parent.x, parent.y).expect("stepValid: parent plot");
        if self.map.plots[parent_plot].area != self.map.plots[new_plot].area {
            return false;
        }

        true
    }

    /// C++ `routeValid(parent, node, ...)` (info = player).
    pub fn route_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, finder: &FinderCtx) -> bool {
        if parent.is_none() {
            return true;
        }

        let new_plot = self.map.plot_soren(node.x, node.y).expect("routeValid: NewPlot");

        let player = finder_player(finder);
        let team = self.players[player].team;

        if !self.plot_is_owned(new_plot) || self.plot_get_team(new_plot) == Some(team) {
            if self.map.plots[new_plot].route_type == self.player_get_best_route(player, Some(new_plot)) {
                return true;
            }
        }

        false
    }

    /// C++ `borderValid(parent, node, ...)` (info = player).
    pub fn border_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, finder: &FinderCtx) -> bool {
        if parent.is_none() {
            return true;
        }

        let new_plot = self.map.plot_soren(node.x, node.y).expect("borderValid: NewPlot");

        let player = finder_player(finder);

        self.plot_get_team(new_plot) == Some(self.players[player].team)
    }

    /// C++ `areaValid(parent, node, ...)`: same water/land status as the parent.
    pub fn area_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, _finder: &FinderCtx) -> bool {
        let Some(parent) = parent else {
            return true;
        };

        let parent_plot = self.map.plot_soren(parent.x, parent.y).expect("areaValid: parent plot");
        let node_plot = self.map.plot_soren(node.x, node.y).expect("areaValid: node plot");

        self.plot_is_water(parent_plot) == self.plot_is_water(node_plot)
    }

    /// C++ `joinArea(parent, node, data, ...)`: on `ASNL_ADDCLOSED` sets the plot's area to
    /// the finder info.
    pub fn join_area(&mut self, node: &PathNode, data: i32, finder: &FinderCtx) -> i32 {
        if data == ASNL_ADDCLOSED {
            let plot = self.map.plot_soren(node.x, node.y).expect("joinArea: plot");
            self.plot_set_area(plot, Some(AreaId(finder.info as u32)));
        }

        1
    }

    /// C++ `plotGroupValid(parent, node, ...)` (info = player).
    pub fn plot_group_valid(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, finder: &FinderCtx) -> bool {
        let Some(parent) = parent else {
            return true;
        };

        let old_plot = self.map.plot_soren(parent.x, parent.y).expect("plotGroupValid: OldPlot");
        let new_plot = self.map.plot_soren(node.x, node.y).expect("plotGroupValid: NewPlot");

        let player = finder_player(finder);
        let team = self.players[player].team;

        if self.plot_get_plot_group(old_plot, player) == self.plot_get_plot_group(new_plot, player) {
            if self.plot_is_trade_network(new_plot, team) {
                if self.plot_is_trade_network_connected(new_plot, old_plot, team) {
                    return true;
                }
            }
        }

        false
    }

    /// C++ `potentialIrrigation(parent, node, ...)`.
    pub fn potential_irrigation(&self, parent: Option<&PathNode>, node: &PathNode, _data: i32, _finder: &FinderCtx) -> bool {
        if parent.is_none() {
            return true;
        }

        let plot = self.map.plot_soren(node.x, node.y).expect("potentialIrrigation: plot");
        self.plot_is_potential_irrigation(plot)
    }

    /// C++ `checkFreshWater(parent, node, data, pointer = &bFoundFreshWater, ...)`.
    pub fn check_fresh_water(&self, node: &PathNode, data: i32, found_fresh_water: &mut bool) -> i32 {
        if data == ASNL_ADDCLOSED {
            let plot = self.map.plot_soren(node.x, node.y).expect("checkFreshWater: plot");
            if self.plot_is_fresh_water(plot) {
                *found_fresh_water = true;
            }
        }

        1
    }

    /// C++ `changeIrrigated(parent, node, data, pointer = &bIrrigated, ...)`.
    pub fn change_irrigated(&mut self, node: &PathNode, data: i32, irrigated: bool) -> i32 {
        if data == ASNL_ADDCLOSED {
            let plot = self.map.plot_soren(node.x, node.y).expect("changeIrrigated: plot");
            self.plot_set_irrigated(plot, irrigated);
        }

        1
    }
}

/// C++ `(PlayerTypes)GetInfo(finder)`.
fn finder_player(finder: &FinderCtx) -> PlayerId {
    debug_assert!(finder.info >= 0, "FAStar info is expected to be a player");
    PlayerId(finder.info as u8)
}

/// Saved-game support for [`PathFinder`] (a child module so it can read the private search
/// state; the code lives with the rest of the save format in `src/save/`).
#[path = "save/path_finder.rs"]
mod persist;

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

    // ---- A* on hand-built grids ----

    /// A grid map for the generic search: `#` = blocked, digits = cost of entering the cell,
    /// `X` = cost 200, `.` = cost 1. Row 0 of `rows` is y = 0. Counts the `valid` calls.
    struct Grid {
        geom: MapGeom,
        cells: Vec<u8>,
        /// Heuristic per step (0 = Dijkstra / flood).
        h_weight: i32,
        /// Extra heuristic for one cell (to make the heuristic inconsistent in a test).
        h_bonus: Option<((i32, i32), i32)>,
        dest_ok: bool,
        valid_calls: std::cell::Cell<u32>,
    }

    impl Grid {
        fn new(rows: &[&str], wrap_x: bool) -> Grid {
            let geom = MapGeom::new(rows[0].len() as i32, rows.len() as i32, wrap_x, false);
            let cells = rows.iter().flat_map(|r| r.bytes()).collect();
            Grid { geom, cells, h_weight: 1, h_bonus: None, dest_ok: true, valid_calls: std::cell::Cell::new(0) }
        }

        fn cell(&self, x: i32, y: i32) -> u8 {
            self.cells[self.geom.plot_num(x, y) as usize]
        }

        fn finder(&self) -> PathFinder {
            let mut f = PathFinder::new(FinderKind::Step);
            f.initialize(self.geom);
            f
        }

        fn run(&self, f: &mut PathFinder, start: (i32, i32), dest: (i32, i32), cardinal: bool, reuse: bool) -> bool {
            f.search(self, start.0, start.1, dest.0, dest.1, cardinal, 0, reuse, None)
        }
    }

    impl FinderCallbacks for Grid {
        fn dest_valid(&self, _ctx: &FinderCtx, x: i32, y: i32) -> bool {
            self.dest_ok && self.cell(x, y) != b'#'
        }

        fn heuristic(&self, from_x: i32, from_y: i32, to_x: i32, to_y: i32) -> i32 {
            let mut h = self.geom.step_distance(from_x, from_y, to_x, to_y) * self.h_weight;
            if let Some((xy, bonus)) = self.h_bonus {
                if xy == (from_x, from_y) {
                    h += bonus;
                }
            }
            h
        }

        fn cost(&self, _ctx: &FinderCtx, _parent: &PathNode, node: &PathNode) -> i32 {
            match self.cell(node.x, node.y) {
                c @ b'1'..=b'9' => (c - b'0') as i32,
                b'X' => 200,
                _ => 1,
            }
        }

        fn valid(&self, _ctx: &FinderCtx, parent: Option<&PathNode>, node: &PathNode) -> bool {
            self.valid_calls.set(self.valid_calls.get() + 1);
            parent.is_none() || self.cell(node.x, node.y) != b'#'
        }

        fn notify_child(&self, ctx: &FinderCtx, parent: Option<&PathNode>, node: &mut PathNode, data: i32) {
            // Steps from the start (stepAdd) and how often the node was re-parented.
            step_add(parent, node, data);
            if data != ASNC_INITIALADD && data != ASNC_NEWADD {
                node.data2 += 1;
            }
            assert!(ctx.start_x >= 0);
        }
    }

    /// The path of the last search, start first.
    fn path(f: &PathFinder) -> Vec<(i32, i32)> {
        let mut out = Vec::new();
        let mut node = f.last_node();
        while let Some(n) = node {
            out.push((n.x, n.y));
            node = f.parent_of(n);
        }
        out.reverse();
        out
    }

    fn known_cost(f: &PathFinder) -> i32 {
        f.last_node().unwrap().known_cost
    }

    #[test]
    fn astar_finds_cheapest_path_around_walls() {
        let grid = Grid::new(&["....#....", ".##.#.##.", ".#..#..#.", ".#.###.#.", "........."], false);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (3, 0), (5, 0), false, false));
        let p = path(&f);
        assert_eq!(p.first(), Some(&(3, 0)));
        assert_eq!(p.last(), Some(&(5, 0)));
        // Around the wall through the bottom row: every step costs 1.
        assert_eq!(known_cost(&f), (p.len() - 1) as i32);
        assert_eq!(f.last_node().unwrap().data1, (p.len() - 1) as i32);
        for w in p.windows(2) {
            assert_eq!(grid.geom.step_distance(w[0].0, w[0].1, w[1].0, w[1].1), 1);
            assert_ne!(grid.cell(w[1].0, w[1].1), b'#');
        }
        assert_eq!(known_cost(&f), bfs_distance(&grid, (3, 0), (5, 0), false).unwrap());
        assert_eq!(known_cost(&f), 10);
        // The destination was closed last; the start first.
        assert_eq!(f.closed_nodes().next().map(|n| (n.x, n.y)), Some((3, 0)));
        assert_eq!(f.closed_nodes().last().map(|n| (n.x, n.y)), Some((5, 0)));
        // GetLastNode and the path walks agree.
        assert_eq!(f.path_first_xy(), Some(p[1]));
    }

    /// Plain BFS (unit costs) for cross-checking.
    fn bfs_distance(grid: &Grid, start: (i32, i32), dest: (i32, i32), cardinal: bool) -> Option<i32> {
        let n = grid.geom.num_plots() as usize;
        let mut dist = vec![-1; n];
        let mut queue = std::collections::VecDeque::new();
        dist[grid.geom.plot_num(start.0, start.1) as usize] = 0;
        queue.push_back(start);
        let steps: &[(i32, i32)] = if cardinal { &CARDINAL_STEPS } else { &ALL_STEPS };
        while let Some((x, y)) = queue.pop_front() {
            let dxy = dist[grid.geom.plot_num(x, y) as usize];
            if (x, y) == dest {
                return Some(dxy);
            }
            for &(dx, dy) in steps {
                if let Some(p) = grid.geom.plot(x + dx, y + dy) {
                    let (nx, ny) = grid.geom.plot_coords(p);
                    if grid.cell(nx, ny) != b'#' && dist[p.0 as usize] < 0 {
                        dist[p.0 as usize] = dxy + 1;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }
        None
    }

    #[test]
    fn astar_weighted_costs_and_failures() {
        // Going straight through the 9s is longer than around them (7 steps of cost 1).
        let grid = Grid::new(&["......", ".9999.", ".9..9.", ".9999.", "......"], false);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (0, 2), (5, 2), false, false));
        assert_eq!(known_cost(&f), 7);
        assert!(path(&f).iter().all(|&(x, y)| grid.cell(x, y) != b'9'));

        // Unreachable destination (enclosed): the search closes the start's whole region.
        let grid = Grid::new(&[".....", ".###.", ".#.#.", ".###.", "....."], false);
        let mut f = grid.finder();
        assert!(!grid.run(&mut f, (0, 0), (2, 2), false, false));
        assert_eq!(f.last_node(), None);
        assert_eq!(f.closed_nodes().count(), 16);

        // DestValid failure: no search at all.
        let mut grid = Grid::new(&["...", "..."], false);
        grid.dest_ok = false;
        let mut f = grid.finder();
        assert!(!grid.run(&mut f, (0, 0), (2, 1), false, false));
        assert_eq!(f.closed_nodes().count(), 0);
        assert_eq!(grid.valid_calls.get(), 0);

        // Start == destination: the start node itself, no parent, INITIALADD data.
        let grid = Grid::new(&["...", "..."], false);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (1, 1), (1, 1), false, false));
        assert_eq!(path(&f), vec![(1, 1)]);
        assert_eq!(f.path_first_xy(), Some((1, 1)));
        assert_eq!(f.last_node().unwrap().data1, 0);
    }

    #[test]
    fn astar_cardinal_and_wrapping() {
        let grid = Grid::new(&["....", "....", "...."], false);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (0, 0), (2, 2), true, false));
        assert_eq!(known_cost(&f), 4);
        for w in path(&f).windows(2) {
            assert!(w[0].0 == w[1].0 || w[0].1 == w[1].1, "diagonal step with bCardinalOnly");
        }
        assert!(grid.run(&mut f, (0, 0), (2, 2), false, false));
        assert_eq!(known_cost(&f), 2);

        // Wrapping in x: (0, 0) -> (5, 0) is one step west across the seam.
        let grid = Grid::new(&["......", "......"], true);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (0, 0), (5, 0), false, false));
        assert_eq!(path(&f), vec![(0, 0), (5, 0)]);
        let grid = Grid::new(&["......", "......"], false);
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (0, 0), (5, 0), false, false));
        assert_eq!(known_cost(&f), 5);
    }

    #[test]
    fn astar_flood_fill_closes_reachable_region() {
        let grid = Grid::new(&["..#..", "..#..", "###..", "....."], false);
        let mut f = grid.finder();
        assert!(!grid.run(&mut f, (0, 0), (-1, -1), false, false));
        let mut closed: Vec<(i32, i32)> = f.closed_nodes().map(|n| (n.x, n.y)).collect();
        assert_eq!(closed[0], (0, 0), "the start is closed first");
        closed.sort();
        assert_eq!(closed, vec![(0, 0), (0, 1), (1, 0), (1, 1)]);
        assert!(f.nodes.iter().filter(|n| n.list == Some(PathListType::Closed)).count() == 4);
        // The other region (cardinal moves connect it too) is separate.
        assert!(!grid.run(&mut f, (4, 3), (-1, -1), true, false));
        assert_eq!(f.closed_nodes().count(), 11);
        // Nodes of the previous search were reset.
        assert!(f.nodes.iter().filter(|n| n.list.is_some()).all(|n| n.x >= 2 || n.y >= 2));
        assert_eq!(f.nodes[0], PathNode { x: 0, y: 0, ..PathNode::default() });
    }

    #[test]
    fn astar_reuse_matches_fresh_search() {
        let rows = ["..........", ".####.###.", ".#......#.", ".#.####.#.", "...#..#...", ".#.#.##.#.", "...#......"];
        let grid = Grid::new(&rows, false);
        let mut reused = grid.finder();
        let start = (0, 0);
        let mut targets = Vec::new();
        for y in 0..rows.len() as i32 {
            for x in 0..10 {
                if grid.cell(x, y) != b'#' {
                    targets.push((x, y));
                }
            }
        }
        targets.reverse();
        for &t in &targets {
            let mut fresh = grid.finder();
            let ok_fresh = grid.run(&mut fresh, start, t, false, false);
            let ok_reused = grid.run(&mut reused, start, t, false, true);
            assert_eq!(ok_fresh, ok_reused, "target {t:?}");
            assert_eq!(ok_fresh, bfs_distance(&grid, start, t, false).is_some());
            if ok_fresh {
                assert_eq!(known_cost(&fresh), known_cost(&reused), "target {t:?}");
                assert_eq!(fresh.last_node().unwrap().data1, reused.last_node().unwrap().data1);
                assert_eq!(known_cost(&fresh), bfs_distance(&grid, start, t, false).unwrap());
            }
        }
        // A different start resets the kept tree even with reuse.
        assert!(grid.run(&mut reused, (9, 6), (0, 0), false, true));
        assert_eq!(path(&reused).first(), Some(&(9, 6)));
        // ForceReset: the next reuse search starts over.
        reused.force_reset();
        assert_eq!(reused.closed_nodes().count(), 0);
        assert!(grid.run(&mut reused, (9, 6), (0, 0), false, true));
        assert_eq!(known_cost(&reused), bfs_distance(&grid, (9, 6), (0, 0), false).unwrap());
    }

    #[test]
    fn astar_is_deterministic_with_lifo_ties() {
        // An open field: many equal-cost paths. The result must be identical run to run, and
        // follow the documented order (last generated child first among equal f).
        let grid = Grid::new(&["......", "......", "......", "......"], false);
        let mut f1 = grid.finder();
        let mut f2 = grid.finder();
        assert!(grid.run(&mut f1, (0, 0), (5, 2), false, false));
        assert!(grid.run(&mut f2, (0, 0), (5, 2), false, false));
        assert_eq!(path(&f1), path(&f2));
        assert_eq!(known_cost(&f1), 5);
        let order1: Vec<_> = f1.closed_nodes().map(|n| (n.x, n.y)).collect();
        let order2: Vec<_> = f2.closed_nodes().map(|n| (n.x, n.y)).collect();
        assert_eq!(order1, order2);
    }

    #[test]
    fn astar_reparents_closed_nodes_with_inconsistent_heuristic() {
        // A heuristic bonus of 100 on (1, 0) delays its expansion past the other cells (the
        // destination (3, 2) costs 200 to enter), so (2, 0) is first reached the long way
        // (cost 4) and closed, and its child (3, 0) gets cost 5. Expanding (1, 0) then gives
        // (2, 0) the cheaper route (cost 2, ASNC_CLOSEDADD_UP) and propagates it to (3, 0)
        // (cost 3, ASNC_PARENTADD_UP).
        let mut grid = Grid::new(&["....", "....", "...X"], false);
        grid.h_weight = 0;
        grid.h_bonus = Some(((1, 0), 100));
        let mut f = grid.finder();
        assert!(grid.run(&mut f, (0, 0), (3, 2), true, false));
        assert_eq!(known_cost(&f), 204);
        assert_eq!(f.last_node().unwrap().data1, 5);
        let closed: Vec<(i32, i32)> = f.closed_nodes().map(|n| (n.x, n.y)).collect();
        assert_eq!(closed.len(), 12);
        let pos = |xy: (i32, i32)| closed.iter().position(|&c| c == xy).unwrap();
        assert!(pos((2, 0)) < pos((1, 0)) && pos((3, 0)) < pos((1, 0)), "closed before (1, 0)");
        // Every other closed node ends with its true distance (re-parenting fixed the detours),
        // and the stepAdd data follows the new parents.
        for n in f.closed_nodes().filter(|n| (n.x, n.y) != (3, 2)) {
            assert_eq!(n.known_cost, n.x + n.y, "node ({}, {})", n.x, n.y);
            assert_eq!(n.data1, n.known_cost, "node ({}, {})", n.x, n.y);
        }
        let node = |x: i32, y: i32| &f.nodes[grid.geom.plot_num(x, y) as usize];
        assert_eq!(node(2, 0).parent, Some(grid.geom.plot_num(1, 0) as u32));
        assert_eq!((node(2, 0).data2, node(3, 0).data2), (1, 1), "one re-parenting each");
        assert_eq!(node(3, 0).parent, Some(grid.geom.plot_num(2, 0) as u32));

        // Without a destination there is no heuristic: a plain uniform-cost flood.
        assert!(!grid.run(&mut f, (0, 0), (-1, -1), true, false));
        assert_eq!(node_cost(&f, &grid, 2, 0), 2);
        assert!(f.closed_nodes().all(|n| n.data2 == 0), "no re-parenting needed");
    }

    fn node_cost(f: &PathFinder, grid: &Grid, x: i32, y: i32) -> i32 {
        f.nodes[grid.geom.plot_num(x, y) as usize].known_cost
    }

    // ---- The game's finders on a hand-built map ----

    /// A game on a map drawn as `rows` (row 0 = y 0): `~` water, `^` peak, `.` land. All land
    /// plots (peaks included) are in one area, all water plots in another.
    fn test_game(rows: &[&str]) -> Game {
        use crate::map::{Map, MapInitData};
        use crate::setup::GameSetup;

        let d = ffh_data::global();
        let mut g = Game::new(d, GameSetup::standard(d, 2, "WORLDSIZE_DUEL", 7));
        let (num_players, num_teams) = (g.num_players(), g.num_teams());
        let (w, h) = (rows[0].len() as i32, rows.len() as i32);
        g.map = Map::new(d, &MapInitData { grid_w: w, grid_h: h, ..Default::default() }, num_players, num_teams);
        let land = g.map_add_area(false);
        let water = g.map_add_area(true);
        for p in g.map.plot_ids() {
            let (x, y) = g.map.xy(p);
            let c = rows[y as usize].as_bytes()[x as usize];
            let plot = &mut g.map.plots[p];
            plot.plot_type = match c {
                b'~' => PlotType::PLOT_OCEAN,
                b'^' => PlotType::PLOT_PEAK,
                _ => PlotType::PLOT_LAND,
            };
            plot.area = Some(if c == b'~' { water } else { land });
        }
        g
    }

    #[test]
    fn step_finder_counts_steps_around_peaks() {
        let g = test_game(&["......", ".^^^^.", "......", "~~~..."]);
        let mut f = g.step_finder.borrow_mut();

        // CvMap::calculatePathDistance: GeneratePath(.., false, 0, bReuse = true), m_iData1.
        assert!(f.generate_path(&g, 2, 0, 2, 2, false, 0, true));
        assert_eq!(f.last_node().unwrap().data1, 4, "around the peaks at x = 0");
        assert_eq!(f.geom, g.map_geom(), "initialized from the map on first use");
        // Reuse from the same start; a target the first search did not close (via (5, 1)).
        assert!(f.generate_path(&g, 2, 0, 5, 3, false, 0, true));
        assert_eq!(f.last_node().unwrap().data1, 5);
        assert_eq!(f.path_first_xy(), Some((3, 0)));
        // Other area (water): stepDestValid fails.
        assert!(!f.generate_path(&g, 2, 0, 0, 3, false, 0, true));
        assert_eq!(f.last_node(), None);
        // The start itself.
        assert!(f.generate_path(&g, 2, 0, 2, 0, false, 0, true));
        assert_eq!(f.last_node().unwrap().data1, 0);

        // A peak is impassable, also as a destination in the same area: no path.
        f.force_reset();
        assert!(!f.generate_path(&g, 2, 0, 2, 1, false, 0, true));
    }

    #[test]
    fn area_finder_floods_land_and_cardinal_water() {
        // Water connected only diagonally; land likewise.
        let g = test_game(&["~.~", ".~.", "~.~"]);
        let mut f = g.area_finder.borrow_mut();
        let closed = |f: &PathFinder| {
            let mut v: Vec<(i32, i32)> = f.closed_nodes().map(|n| (n.x, n.y)).collect();
            v.sort();
            v
        };

        // CvMap::calculateAreas: GeneratePath(x, y, -1, -1, bCardinalOnly = isWater(), iArea).
        assert!(!f.generate_path(&g, 1, 1, -1, -1, true, 7, false));
        assert_eq!(closed(&f), vec![(1, 1)]);
        assert_eq!(f.ctx.info, 7);
        assert!(!f.generate_path(&g, 1, 1, -1, -1, false, 7, false));
        assert_eq!(closed(&f), vec![(0, 0), (0, 2), (1, 1), (2, 0), (2, 2)]);
        assert!(!f.generate_path(&g, 1, 0, -1, -1, false, 8, false));
        assert_eq!(closed(&f), vec![(0, 1), (1, 0), (1, 2), (2, 1)]);
    }
}
