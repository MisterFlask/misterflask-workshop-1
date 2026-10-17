// The 3D renderer's asset modules (docs/R3D.md "Asset recipes"). Importing this registers every
// recipe and fills the catalog. The fallbacks (basic.js) come first; each art-directed module
// after them overrides the catalog entries it fills in. A module that fails to load is reported
// and skipped: what it would have modelled falls back to the basic recipes.

import "./basic.js";
import "./fallbacks.js";

function failed(name, e) {
  console.error(`r3d assets: ${name} did not load:`, e);
}

try { await import("./vegetation.js"); } catch (e) { failed("vegetation", e); }
try { await import("./props.js"); } catch (e) { failed("props", e); }
try { await import("./architecture.js"); } catch (e) { failed("architecture", e); }
try { await import("./units.js"); } catch (e) { failed("units", e); }
try { await import("./fx.js"); } catch (e) { failed("fx", e); }
