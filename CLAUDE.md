# Claude Code Project Instructions

## Model Hierarchy: Escalation and Delegation

At the start of a session, check which model you are running as (via the model
ID in your system prompt or environment): a **Fable/Mythos-class** model
(`claude-fable-*`, `claude-mythos-*`) or a **non-Fable** model (Opus, Sonnet,
Haiku, etc.). Then follow the matching policy.

The principle in both directions: **Fable-level ability is for judgment —
design decisions, ambiguity resolution, diagnosis of stuck problems, and
review of concrete work.** It is not for grind, and it is not an oracle to
chat with. Escalations hand over artifacts (a diff, a failing test output,
two competing sketches), never just a narrative summary — the reviewer must be
able to disagree with your framing, not merely answer your question.

### If you are Fable

Reserve yourself for judgment; delegate the grind.

- **Delegate** well-scoped subtasks — codebase searches, bulk/mechanical
  edits, isolated implementations with a clear spec, running tests and
  verification — to cheaper models (Sonnet by default, Haiku for trivial
  mechanical work) via subagents.
- **Keep for yourself:** writing the spec, resolving ambiguity, design and
  balance decisions, and reviewing delegated work before accepting it.
- **Never consult an advisor** (not Fable, not Opus, not anyone). You are the
  top of the hierarchy; decide directly.

### If you are NOT Fable (Opus, Sonnet, Haiku, ...)

Do the work yourself. Escalate for a **review of concrete work product** when
a gate fires — gates are objective so they trigger whether or not you *feel*
uncertain:

1. **Core-logic diff, before commit:** your change touches core game logic
   (`Game.ts`, `Combat.ts`) or alters game balance values (unit stats, costs,
   building effects, economy numbers). Send the actual diff for review.
2. **Stuck, after two failed attempts:** you've tried to fix the same bug or
   test failure twice and it still fails. Send the failing output and what
   you tried.
3. **Ambiguity, before implementing:** requirements are ambiguous, or they
   conflict with the design docs. Send the specific conflict and your
   proposed resolution.
4. **Genuine fork in the road:** you cannot choose between approaches with
   real trade-offs. Send both options as concrete sketches, not descriptions.

Feeling lost still counts as a trigger — but it is the extra trigger, not the
only one. Confident routine work outside these gates proceeds without
escalation, regardless of size.

**Who reviews** (attempt in order; don't pre-check quota or availability —
just make the call and fall back on failure):

1. **Fable**, via an Agent-tool subagent with a model override.
2. On failure, **Opus**.
3. On failure, a **fresh instance of your own model** (skip duplicates — for
   Opus, step 2 and 3 are the same thing). A clean-context review of a
   concrete artifact still catches real errors; the value of review is mostly
   fresh eyes on the artifact, not extra raw capability. Reviewing your own
   work inside your own context does NOT count as a review.

Apply the reviewer's feedback or explicitly note why you disagree, then
proceed. One review round per gate — do not loop.

### Summary

| You are | Escalation (only at a gate) | Delegation |
|---|---|---|
| Fable | Never | Grind goes to Sonnet/Haiku; judgment and review stay with you |
| Opus | Fable → fresh Opus instance | As normal |
| Sonnet / Haiku / other | Fable → Opus → fresh instance of yourself | As normal |

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
