//! Port of `CvPlotGroup` (CvPlotGroup.h / CvPlotGroup.cpp): a player's set of plots
//! connected by the trade network, with the bonuses available to it.
//!
//! Plot groups live in the global slab `Game::plot_groups`; the owner's id list is
//! `Player::plot_groups` (maintained by the CvPlayer port: `initPlotGroup` /
//! `deletePlotGroup`). C++ `init(iID, eOwner, pPlot)` is `PlotGroup::new(d, id, owner)`
//! followed by `Game::plot_group_add_plot(pg, plot)`.
//!
//! The C++ `CLinkList<XYCoords>` is `plots: Vec<XYCoords>`; list nodes are indices:
//! `headPlotsNode` = 0, `nextPlotsNode(i)` = i + 1, `getLengthPlots` = `plots.len()`,
//! `insertAtEndPlots` = `plots.push`, `deletePlotsNode` = `plot_group_delete_plots_node`
//! (which deletes the group when it becomes empty).

#![allow(unused_variables)]
#![allow(unused_imports)]

use crate::game::Game;
use crate::ids::*;
use crate::structs::XYCoords;
use ffh_data::*;

/// C++ `CvPlotGroup`.
#[derive(Clone, Debug)]
pub struct PlotGroup {
    /// C++ `m_iID`.
    pub id: PlotGroupId,
    /// C++ `m_eOwner` (always set for a live group; C++ `NO_PLAYER` only before `init`).
    pub owner: PlayerId,
    /// C++ `m_paiNumBonuses` (`getNumBonuses`). Change: `plot_group_change_num_bonuses`.
    pub num_bonuses: IdxVec<BonusType, i32>,
    /// C++ `m_plots` (`CLinkList<XYCoords>`), in list order.
    pub plots: Vec<XYCoords>,
}

impl PlotGroup {
    /// C++ `CvPlotGroup::init` minus its `addPlot` (= `reset(iID, eOwner)`), sized from the
    /// game data. Follow with `Game::plot_group_add_plot`.
    pub fn new(d: &GameData, id: PlotGroupId, owner: PlayerId) -> PlotGroup {
        PlotGroup::with_counts(id, owner, d.bonuses.len())
    }

    /// C++ `CvPlotGroup::reset` (as `reset(iID, eOwner)`) with an explicit bonus count.
    pub fn with_counts(id: PlotGroupId, owner: PlayerId, num_bonuses: usize) -> PlotGroup {
        PlotGroup { id, owner, num_bonuses: IdxVec::from_elem(0, num_bonuses), plots: Vec::new() }
    }
}

impl Default for PlotGroup {
    /// C++ `CvPlotGroup()` (`reset(0, NO_PLAYER, true)`; the owner placeholder is player 0).
    fn default() -> Self {
        PlotGroup::with_counts(PlotGroupId(0), PlayerId(0), 0)
    }
}

impl Game {
    /// C++ `CvPlotGroup::addPlot`: appends the plot and points the plot at this group.
    pub fn plot_group_add_plot(&mut self, pg: PlotGroupId, p: PlotIdx) {
        let (x, y) = self.map.xy(p);
        let owner = self.plot_groups[pg].owner;
        self.plot_groups[pg].plots.push(XYCoords { x, y });
        self.plot_set_plot_group(p, owner, Some(pg));
    }

    /// C++ `CvPlotGroup::removePlot`: unlinks the plot (deleting the group if it becomes empty).
    pub fn plot_group_remove_plot(&mut self, pg: PlotGroupId, p: PlotIdx) {
        let (x, y) = self.map.xy(p);
        if let Some(i) = self.plot_groups[pg].plots.iter().position(|xy| xy.x == x && xy.y == y) {
            let owner = self.plot_groups[pg].owner;
            self.plot_set_plot_group(p, owner, None);
            self.plot_group_delete_plots_node(pg, i); // can delete this plot group
        }
    }

    /// C++ `CvPlotGroup::recalculatePlots`: if the group is no longer fully connected
    /// (C++ counts it with the plot group finder), dissolves it and re-runs
    /// `updatePlotGroup` on each former plot.
    pub fn plot_group_recalculate_plots(&mut self, pg: PlotGroupId) {
        todo!("CvPlotGroup::recalculatePlots")
    }

    /// C++ `CvPlotGroup::hasBonus` (`getNumBonuses` is the `num_bonuses` field).
    #[inline]
    pub fn plot_group_has_bonus(&self, pg: PlotGroupId, bonus: BonusType) -> bool {
        self.plot_groups[pg].num_bonuses[bonus] > 0
    }

    /// C++ `CvPlotGroup::changeNumBonuses`: updates the owner's cities in the group and
    /// (FFH) every owner unit's bonus affinity.
    pub fn plot_group_change_num_bonuses(&mut self, pg: PlotGroupId, bonus: BonusType, change: i32) {
        todo!("CvPlotGroup::changeNumBonuses")
    }

    /// C++ `CvPlotGroup::deletePlotsNode`: removes list entry `index` and returns the index
    /// of the next entry (None at the end). When the list becomes empty the group is deleted
    /// (C++ `GET_PLAYER(owner).deletePlotGroup(id)`), so the id is dangling afterwards.
    pub fn plot_group_delete_plots_node(&mut self, pg: PlotGroupId, index: usize) -> Option<usize> {
        todo!("CvPlotGroup::deletePlotsNode")
    }
}
