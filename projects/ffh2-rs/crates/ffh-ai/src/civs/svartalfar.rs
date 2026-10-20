//! Svartalfar. Their `MaintainFeatures` keeps the forest under every improvement they build on
//! their own land (`CvPlot::setImprovementType` with FfH's check), and workers only build there,
//! so they never clear forest. Veil of Night stays with the generic play (cast the first turn the
//! AI-only gate opens): never casting it measured worse (win 20% -> 10%, docs/AI_LOG.md). They keep
//! casters so their Illusionists (Mages upgraded from level-4 Adepts) come:
//! [`super::unique_casters`], the owner's rule that civilizations use their unique units.

use ffh_core::game::Game;
use ffh_core::ids::*;

use super::CivPlay;
use crate::production::Choice;
use crate::Ai;

pub struct Svartalfar;

impl CivPlay for Svartalfar {
    fn production(&self, _ai: &Ai, g: &Game, c: CityId, threat: i32) -> Option<Choice> {
        super::unique_casters(g, c, threat)
    }
}
