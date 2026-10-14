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

use crate::game::Game;
use crate::ids::*;
use crate::path::{count_plot_group, FinderCtx, ASNL_ADDCLOSED};
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
        let owner = self.plot_groups[pg].owner;

        if let Some(&head) = self.plot_groups[pg].plots.first() {
            let plot = self.map.plot_soren(head.x, head.y).expect("plot group plot");

            let mut count = 0;

            // SetData(&GC.getPlotGroupFinder(), &iCount) +
            // GeneratePath(&GC.getPlotGroupFinder(), x, y, -1, -1, false, eOwner)
            let ctx = FinderCtx { start_x: head.x, start_y: head.y, dest_x: -1, dest_y: -1, info: owner.i() };
            let closed = self.map_flood_fill(plot, false, |g, parent, node| {
                let parent = parent.map(|q| g.finder_node(q));
                g.plot_group_valid(parent.as_ref(), &g.finder_node(node), 0, &ctx)
            });
            for q in closed {
                count_plot_group(&self.finder_node(q), ASNL_ADDCLOSED, &mut count);
            }

            if count == self.plot_groups[pg].plots.len() as i32 {
                return;
            }
        }

        let mut old_plot_group: Vec<XYCoords> = Vec::new();

        // Empties the list head first; the last deletion deletes this plot group. (Each
        // `setPlotGroup` still sees the plots not yet removed, as in C++.)
        let mut node = if self.plot_groups[pg].plots.is_empty() { None } else { Some(0) };
        while let Some(i) = node {
            let xy = self.plot_groups[pg].plots[i];
            let plot = self.map.plot_soren(xy.x, xy.y).expect("Plot is not assigned a valid value");

            old_plot_group.push(xy);

            self.plot_set_plot_group(plot, owner, None);

            node = self.plot_group_delete_plots_node(pg, i); // will delete this PlotGroup...
        }

        for xy in old_plot_group {
            let plot = self.map.plot_soren(xy.x, xy.y).expect("Plot is not assigned a valid value");

            self.plot_update_plot_group_player(plot, owner, true);
        }
    }

    /// C++ `CvPlotGroup::hasBonus` (`getNumBonuses` is the `num_bonuses` field).
    #[inline]
    pub fn plot_group_has_bonus(&self, pg: PlotGroupId, bonus: BonusType) -> bool {
        self.plot_groups[pg].num_bonuses[bonus] > 0
    }

    /// C++ `CvPlotGroup::changeNumBonuses`: updates the owner's cities in the group and
    /// (FFH) every owner unit's bonus affinity.
    pub fn plot_group_change_num_bonuses(&mut self, pg: PlotGroupId, bonus: BonusType, change: i32) {
        let owner = self.plot_groups[pg].owner;

        if change != 0 {
            // C++ also reads the old count (`iOldNumBonuses`) and never uses it.
            self.plot_groups[pg].num_bonuses[bonus] += change;

            //FAssertMsg(m_paiNumBonuses[eBonus] >= 0, ...); XXX (commented out in C++)

            // `CvCity::changeNumBonuses` does not touch plot groups, so a copy of the list
            // walks the same nodes.
            for xy in self.plot_groups[pg].plots.clone() {
                let plot = self.map.plot_soren(xy.x, xy.y).expect("plot group plot");
                if let Some(city) = self.map.plots[plot].plot_city {
                    if self.cities[city].owner == owner {
                        self.city_change_num_bonuses(city, bonus, change);
                    }
                }
            }
        }

        // FfH Damage Types: Added by Kael 08/23/2007 (also when `change == 0`; the owner is
        // never NO_PLAYER for a live group)
        for u in self.players[owner].units.clone() {
            if self.units.contains(u) {
                self.unit_update_bonus_affinity(u, bonus);
            }
        }
        // FfH: End Add
    }

    /// C++ `CvPlotGroup::deletePlotsNode`: removes list entry `index` and returns the index
    /// of the next entry (None at the end). When the list becomes empty the group is deleted
    /// (C++ `GET_PLAYER(owner).deletePlotGroup(id)`), so the id is dangling afterwards.
    pub fn plot_group_delete_plots_node(&mut self, pg: PlotGroupId, index: usize) -> Option<usize> {
        let plots = &mut self.plot_groups[pg].plots;
        plots.remove(index);
        let next = if index < plots.len() { Some(index) } else { None };

        if plots.is_empty() {
            let owner = self.plot_groups[pg].owner;
            self.player_delete_plot_group(owner, pg);
        }

        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::map::tests::game_from_ascii;

    fn new_group(g: &mut Game, owner: PlayerId, plots: &[(i32, i32)]) -> PlotGroupId {
        let d = g.d;
        let pg = g.plot_groups.insert_with(|id| PlotGroup::new(d, id, owner));
        g.players[owner].plot_groups.push(pg);
        g.plot_groups[pg].plots = plots.iter().map(|&(x, y)| XYCoords { x, y }).collect();
        pg
    }

    #[test]
    fn delete_plots_node_deletes_the_emptied_group() {
        let mut g = game_from_ascii(&["###"], false, false);
        let owner = PlayerId(0);
        let pg = new_group(&mut g, owner, &[(0, 0), (1, 0), (2, 0)]);

        assert_eq!(g.plot_group_delete_plots_node(pg, 1), Some(1));
        assert_eq!(g.plot_groups[pg].plots, vec![XYCoords { x: 0, y: 0 }, XYCoords { x: 2, y: 0 }]);
        assert_eq!(g.plot_group_delete_plots_node(pg, 1), None);
        assert!(g.plot_groups.contains(pg));
        assert_eq!(g.plot_group_delete_plots_node(pg, 0), None);
        assert!(!g.plot_groups.contains(pg));
        assert!(g.players[owner].plot_groups.is_empty());
    }

    #[test]
    fn change_num_bonuses_without_cities() {
        let mut g = game_from_ascii(&["###"], false, false);
        let owner = PlayerId(0);
        let pg = new_group(&mut g, owner, &[(0, 0), (1, 0)]);
        let bonus = BonusType(0);

        g.plot_group_change_num_bonuses(pg, bonus, 2);
        assert_eq!(g.plot_groups[pg].num_bonuses[bonus], 2);
        assert!(g.plot_group_has_bonus(pg, bonus));
        g.plot_group_change_num_bonuses(pg, bonus, 0);
        g.plot_group_change_num_bonuses(pg, bonus, -2);
        assert!(!g.plot_group_has_bonus(pg, bonus));

        // An empty group: no finder run, nothing to dissolve.
        let empty = new_group(&mut g, owner, &[]);
        g.plot_group_recalculate_plots(empty);
        assert!(g.plot_groups.contains(empty));
    }
}
