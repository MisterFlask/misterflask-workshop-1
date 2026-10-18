//! Svartalfar. Their `MaintainFeatures` keeps the forest under every improvement they build on
//! their own land (`CvPlot::setImprovementType` with FfH's check), and workers only build there,
//! so they never clear forest. Veil of Night stays with the generic play (cast the first turn the
//! AI-only gate opens): never casting it measured worse (win 20% -> 10%, docs/civ-ai-d.md).

use super::CivPlay;

pub struct Svartalfar;

impl CivPlay for Svartalfar {}
