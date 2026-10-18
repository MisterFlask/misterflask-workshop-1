//! Ljosalfar. Their `MaintainFeatures` keeps the forest under every improvement they build on
//! their own land (`CvPlot::setImprovementType` with FfH's check), and workers only build there,
//! so they never clear forest. But the build time still adds the forest's removal time
//! (`CvPlot::getBuildTime`: a cottage 400 + 250 on Forest, + 500 on Ancient Forest) and the Elf
//! promotion takes 20% off the work rate, so an improvement on forest costs them about twice the
//! worker-turns. [`Ljosalfar::production`] trains workers to twice the generic target's share for
//! unimproved worked plots (`production` step 6: one per three such plots; here two per three),
//! from a garrisoned city at peace.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_data::*;

use super::CivPlay;
use crate::production::{best_unit, count_ai, garrison, has_worker_builds, unimproved_worked_plots, Choice, Role};
use crate::Ai;

pub struct Ljosalfar;

impl CivPlay for Ljosalfar {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        elf_worker(g, c, threat)
    }
}

/// A worker while workers are fewer than the forest build time asks for (see the module doc).
fn elf_worker(g: &Game, c: CityId, threat: i32) -> Option<Choice> {
    let p = g.cities[c].owner;
    if threat > 0 || g.cities[c].population < 2 || g.city_is_occupation(c) || !has_worker_builds(g, p) {
        return None;
    }
    if garrison(g, p, g.city_plot(c)).0 == 0 || crate::production::financial_trouble(g, p) {
        return None;
    }
    let team = g.players[p].team;
    let barb = g.barbarian_team();
    if g.teams.keys().any(|t| t != team && t != barb && g.teams[t].alive_count > 0 && g.at_war(team, t)) {
        return None;
    }
    let n = g.players[p].cities.len() as i32;
    let unimproved = unimproved_worked_plots(g, p);
    let target = (n.min(3) + (unimproved * 2 + 1) / 3).min(n * 3).max(1);
    let workers = count_ai(g, p, UnitAIType::UNITAI_WORKER);
    if workers >= target {
        return None;
    }
    let u = best_unit(g, c, Role::Worker)?;
    crate::trace!(g, p, "{}: elf worker ({} of {}, {} unimproved worked plots)", g.cities[c].name, workers, target, unimproved);
    Some(Choice::Train(u, UnitAIType::UNITAI_WORKER))
}
