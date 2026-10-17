// The 3D renderer's asset modules (docs/R3D.md "Asset recipes"). Importing this registers every
// recipe and fills the catalog; a module listed later overrides the catalog entries of the ones
// before it, so the fallbacks (basic.js) come first.

import "./basic.js";
import "./fallbacks.js";
// The art-directed recipe modules, after the fallbacks (their catalog entries win).
import "./vegetation.js";
import "./props.js";
import "./architecture.js";
import "./units.js";
import "./fx.js";
