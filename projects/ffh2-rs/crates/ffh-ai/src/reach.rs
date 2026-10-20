//! Bounded reachability flood for target selection.
//!
//! The C++ AI runs a full A* (`generatePath`) for every candidate target of every unit,
//! which dominates late-game turn time. Here a unit floods outward once (Dijkstra on
//! movement points, bounded by a turn budget) and scores every reached plot; only the
//! chosen target gets a real path, through the rules' `MISSION_MOVE_TO`.
//!
//! The flood is an estimate: it uses the unit's own passability (`canMoveInto` without
//! attacking) and plot movement costs, treats unrevealed plots as passable at one move,
//! and does not model zones of control or stacking limits. The rules' path finder has the
//! final word when the mission starts.

use std::cmp::Reverse;
use std::collections::BinaryHeap;

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

/// One reached plot: `turns` is the estimated number of turns to arrive (0 = this turn).
#[derive(Copy, Clone, Debug)]
pub struct Reached {
    pub plot: PlotIdx,
    pub turns: i32,
}

/// Reusable flood buffers (one per AI turn; sized to the map lazily).
#[derive(Default)]
pub struct Flood {
    cost: Vec<i32>,
    touched: Vec<PlotIdx>,
    heap: BinaryHeap<Reverse<(i32, u32)>>,
}

impl Flood {
    fn reset(&mut self, num_plots: usize) {
        if self.cost.len() != num_plots {
            self.cost = vec![i32::MAX; num_plots];
            self.touched.clear();
        } else {
            for &p in &self.touched {
                self.cost[p.idx()] = i32::MAX;
            }
            self.touched.clear();
        }
        self.heap.clear();
    }

    /// Flood from unit `u`'s plot for at most `max_turns` turns. `allow_attack_last`
    /// lets the last step enter a plot holding enemies (for attack targets).
    /// Returns reached plots in nondecreasing cost order (deterministic tie-break on index).
    pub fn run(&mut self, g: &Game, u: UnitId, max_turns: i32, allow_enemy_target: bool) -> Vec<Reached> {
        let max_moves = g.unit_max_moves(u).max(1);
        // Moves already spent this turn count against the first turn.
        let spent = (max_moves - g.unit_moves_left(u)).max(0);
        self.run_moves(g, u, max_moves, spent, max_turns, allow_enemy_target)
    }

    /// [`Flood::run`] for a unit of `max_moves` moves (in `MOVE_DENOMINATOR` units), `spent` of
    /// them used this turn, that moves as `u` does (its passability and movement costs): a unit
    /// `u` is about to create on its plot, such as a summon.
    pub fn run_moves(&mut self, g: &Game, u: UnitId, max_moves: i32, spent: i32, max_turns: i32, allow_enemy_target: bool) -> Vec<Reached> {
        let num_plots = g.map.num_plots();
        self.reset(num_plots);
        let start = g.unit_plot(u);
        let team = g.unit_get_team(u);
        let max_moves = max_moves.max(1);
        let budget = max_moves * (max_turns + 1);
        let mut out = Vec::new();

        self.cost[start.idx()] = spent;
        self.touched.push(start);
        self.heap.push(Reverse((spent, start.idx() as u32)));

        while let Some(Reverse((c, pi))) = self.heap.pop() {
            let p = PlotIdx::from_usize(pi as usize);
            if c > self.cost[p.idx()] {
                continue;
            }
            out.push(Reached { plot: p, turns: c / max_moves });
            // Do not expand through plots with visible enemies (they are targets, not paths).
            if p != start && g.plot_is_visible_enemy_unit_for_unit(p, u) {
                continue;
            }
            let (x, y) = g.map.xy(p);
            for dir in 0..DirectionType::NUM {
                let Some(q) = g.plot_direction(x, y, DirectionType::from_usize(dir)) else { continue };
                if self.cost[q.idx()] != i32::MAX && self.cost[q.idx()] <= c {
                    continue;
                }
                let revealed = g.map.plots[q].revealed[team];
                let step = if !revealed {
                    if g.map.plots[q].plot_type == PlotType::PLOT_OCEAN && g.unit_get_domain_type(u) == DomainType::DOMAIN_LAND {
                        continue;
                    }
                    g.d.defines.move_denominator
                } else {
                    let enemy = g.plot_is_visible_enemy_unit_for_unit(q, u);
                    if enemy {
                        if !allow_enemy_target {
                            continue;
                        }
                    } else if !g.unit_can_move_into(u, q, false, false, false) {
                        continue;
                    }
                    g.plot_movement_cost(q, u, p)
                };
                // A unit can always enter a plot with any moves left this turn: moves past the
                // end of a turn are rounded up to the next turn boundary.
                let moves_left_this_turn = max_moves - (c % max_moves);
                let nc = if step >= moves_left_this_turn { c + moves_left_this_turn } else { c + step };
                if nc > budget {
                    continue;
                }
                if nc < self.cost[q.idx()] {
                    if self.cost[q.idx()] == i32::MAX {
                        self.touched.push(q);
                    }
                    self.cost[q.idx()] = nc;
                    self.heap.push(Reverse((nc, q.idx() as u32)));
                }
            }
        }
        out
    }
}
