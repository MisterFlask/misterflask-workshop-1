# Claude Code Project Instructions

## Model Hierarchy and Advisor Policy

At the start of a session, check which model you are running as (e.g. via the
model ID in your system prompt or environment): specifically, determine whether
you are a **Fable/Mythos-class** model (`claude-fable-*`, `claude-mythos-*`) or
a **non-Fable** model (Opus, Sonnet, Haiku, etc.). Then follow the matching
policy below.

### If you are NOT Fable (Opus, Sonnet, Haiku, ...)

Consult a stronger model as an **advisor** only when you are experiencing
**implementation or design uncertainty** — you're unsure which approach is
right, a diagnosis isn't converging, competing designs have real trade-offs
you can't resolve, or you'd be guessing. Consult before committing to the
uncertain choice (e.g. spawn an advisor subagent via the Agent tool with a
model override, asking it to review your plan or diagnosis):

1. **Try Fable first** as the advisor. Don't attempt to pre-check availability,
   quota, or spend limits — just make the call.
2. If the Fable call **fails** (for any reason: capacity, spend limits, model
   not offered, error), **fall back to Opus** as the advisor.
3. **Exception — no self-advising:** Opus must never use Opus as its advisor.
   If you are Opus and the Fable call fails, proceed without an advisor rather
   than consulting another Opus instance.

If you are confident in the approach — even on a large or important task — do
not consult an advisor. Uncertainty is the trigger, not task size.

### If you ARE Fable

- **Never consult an advisor.** Fable must not call Fable or Opus (or any other
  model) as an advisor — you are the top of the hierarchy; decide directly.
- **Do delegate:** continue delegating well-scoped subtasks (searches, bulk
  mechanical edits, isolated implementations with clear specs, verification
  runs) to cheaper models (Sonnet, Haiku) via subagents, keeping design
  judgment and final review to yourself.

### Summary

| You are | Advisor when uncertain (implementation/design) | Delegation |
|---|---|---|
| Fable | None (never Fable or Opus) | Delegate well-scoped subtasks to cheaper models |
| Opus | Try Fable; on failure, none (never Opus) | As normal |
| Sonnet / Haiku / other | Try Fable; on failure, Opus | As normal |

## FFH Simplified Game Project

### Design Document Synchronization

**IMPORTANT**: When making any changes to gameplay mechanics in the code, you MUST also update the corresponding design documentation to reflect those changes.

Design documents are located in:
- `ffh-simplified-design/FFH_SIMPLIFIED_MASTER_DESIGN.md` - Main game design document
- `projects/ffh-simplified/docs/UNIT_DESIGN.md` - Unit design details

When modifying gameplay mechanics such as:
- Unit stats, costs, or abilities
- Building effects or requirements
- Terrain features and their effects
- Combat mechanics
- Economy/resource values
- Armageddon counter thresholds
- Movement costs
- Any other game balance or mechanics

Always update the relevant design document section to match the code changes. This ensures the design documentation remains accurate and serves as a reliable reference.

### Key Files

- `projects/ffh-simplified/src/data/soldiers.ts` - Unit definitions
- `projects/ffh-simplified/src/data/buildings.ts` - Building definitions
- `projects/ffh-simplified/src/data/terrainFeatures.ts` - Terrain feature definitions
- `projects/ffh-simplified/src/game/Game.ts` - Core game logic
- `projects/ffh-simplified/src/game/Combat.ts` - Combat system
