//! [`Persist`] for the path finders (`crate::path::PathFinder`, C++ `FAStar`).
//!
//! Declared from `path.rs` as its child module `persist` so that it can read the private search
//! state. The finders are saved because their kept search tree is observable: a later
//! `generate_path(.., reuse = true)` with the same start and key continues the kept tree (C++
//! `bReuse`) instead of searching afresh, and the step finder is only reset when plot types
//! change. Only nodes that differ from their freshly initialized state are written.

use super::*;
use crate::save::io::persist_struct;
use crate::save::{LoadError, Persist, Reader, Writer};

persist_struct!(MapGeom { width, height, wrap_x, wrap_y });
persist_struct!(FinderCtx { start_x, start_y, dest_x, dest_y, info });
persist_struct!(PathNode {
    x, y, total_cost, known_cost, heuristic_cost, num_children, data1, data2, on_stack, list, parent,
    children,
});
persist_struct!(OpenEntry { total_cost, seq, node });
persist_struct!(SearchKey { start_x, start_y, info, cardinal_only, group });

impl Persist for PathListType {
    fn write(&self, w: &mut Writer) {
        w.u8(match self {
            PathListType::Open => 0,
            PathListType::Closed => 1,
        });
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        match r.u8()? {
            0 => Ok(PathListType::Open),
            1 => Ok(PathListType::Closed),
            b => Err(LoadError::Corrupt(format!("path list type {b}"))),
        }
    }
}

const FINDER_KINDS: [FinderKind; 7] = [
    FinderKind::Path,
    FinderKind::Step,
    FinderKind::Route,
    FinderKind::Border,
    FinderKind::Area,
    FinderKind::PlotGroup,
    FinderKind::Irrigation,
];

impl Persist for FinderKind {
    fn write(&self, w: &mut Writer) {
        let i = FINDER_KINDS.iter().position(|k| k == self).expect("FINDER_KINDS lists every kind");
        w.u8(i as u8);
    }
    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let b = r.u8()?;
        FINDER_KINDS.get(b as usize).copied().ok_or_else(|| LoadError::Corrupt(format!("finder kind {b}")))
    }
}

/// Sanity bound on the node count of a saved finder (one node per plot).
const MAX_NODES: usize = 1 << 24;

/// Node `i` as `PathFinder::initialize` creates it.
fn fresh_node(i: usize, width: i32) -> PathNode {
    let i = i as i32;
    PathNode { x: i % width, y: i / width, ..PathNode::default() }
}

impl Persist for PathFinder {
    fn write(&self, w: &mut Writer) {
        let PathFinder {
            kind,
            geom,
            ctx,
            cardinal_only,
            group,
            nodes,
            last_node,
            open,
            open_seq,
            next_seq,
            touched,
            closed_order,
            stack,
            search_key,
            unexpanded,
        } = self;
        kind.write(w);
        geom.write(w);
        ctx.write(w);
        cardinal_only.write(w);
        group.write(w);

        // Nodes: count, then the ones a search left in a non-initial state.
        let width = geom.width.max(1);
        w.len(nodes.len());
        let changed: Vec<(u32, &PathNode)> =
            nodes.iter().enumerate().filter(|&(i, n)| *n != fresh_node(i, width)).map(|(i, n)| (i as u32, n)).collect();
        w.len(changed.len());
        for (i, n) in changed {
            i.write(w);
            n.write(w);
        }
        last_node.write(w);

        // The open list in a canonical (sorted) order: only its pop order is observable, and
        // `OpenEntry` is totally ordered, so any heap holding these entries behaves the same.
        open.clone().into_sorted_vec().write(w);

        w.len(open_seq.len());
        let set: Vec<(u32, u32)> = open_seq.iter().enumerate().filter(|&(_, &s)| s != 0).map(|(i, &s)| (i as u32, s)).collect();
        set.write(w);

        next_seq.write(w);
        touched.write(w);
        closed_order.write(w);
        stack.write(w);
        search_key.write(w);
        unexpanded.write(w);
    }

    fn read(r: &mut Reader<'_>) -> Result<Self, LoadError> {
        let kind = FinderKind::read(r)?;
        let geom = MapGeom::read(r)?;
        let ctx = FinderCtx::read(r)?;
        let cardinal_only = bool::read(r)?;
        let group = Option::<GroupId>::read(r)?;

        let width = geom.width.max(1);
        let num_nodes = r.varint()? as usize;
        if num_nodes > MAX_NODES || num_nodes != geom.num_plots().max(0) as usize {
            return Err(LoadError::Corrupt(format!("path finder with {num_nodes} nodes for a {}x{} map", geom.width, geom.height)));
        }
        let mut nodes: Vec<PathNode> = (0..num_nodes).map(|i| fresh_node(i, width)).collect();
        let num_changed = r.len()?;
        for _ in 0..num_changed {
            let i = u32::read(r)? as usize;
            let n = PathNode::read(r)?;
            *nodes.get_mut(i).ok_or_else(|| LoadError::Corrupt(format!("path node {i} out of range")))? = n;
        }
        let last_node = Option::<u32>::read(r)?;

        let open = BinaryHeap::from(Vec::<OpenEntry>::read(r)?);

        let num_open_seq = r.varint()? as usize;
        if num_open_seq != num_nodes {
            return Err(LoadError::Corrupt("path finder open_seq size".into()));
        }
        let mut open_seq = vec![0u32; num_open_seq];
        for (i, s) in Vec::<(u32, u32)>::read(r)? {
            *open_seq.get_mut(i as usize).ok_or_else(|| LoadError::Corrupt(format!("open_seq {i} out of range")))? = s;
        }

        Ok(PathFinder {
            kind,
            geom,
            ctx,
            cardinal_only,
            group,
            nodes,
            last_node,
            open,
            open_seq,
            next_seq: Persist::read(r)?,
            touched: Persist::read(r)?,
            closed_order: Persist::read(r)?,
            stack: Persist::read(r)?,
            search_key: Persist::read(r)?,
            unexpanded: Persist::read(r)?,
        })
    }
}
