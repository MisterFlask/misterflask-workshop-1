---
name: narrative-poetry
description: Generate narrative poetry with proper rhyme and meter. Use when asked to write poetry, verse, ballads, or song lyrics. Includes metrically-aware rhyming dictionary, meter verification, and curated exemplars.
allowed-tools: Read, Bash(python:*), Glob, Grep
---

# Narrative Poetry Generation

Generate high-quality narrative poetry with verified rhyme and meter. Output must be under 5000 characters (suitable for song lyrics, though generate as poetry, not "lyrics").

## Workflow Overview

### Phase 1: Planning (User Checkpoints Required)

#### Step 1a: Gather Input
Gather from user:
- Subject/story
- Optional: form preference (ballad, blank verse, sonnet sequence, etc.)
- Optional: exemplar hint (tone, era, poet style)

#### Step 1b: Concept Generation
Generate **10 narrative concepts** for the subject matter. Each concept is 2-3 sentences describing:
- The narrative angle/approach
- The central tension or movement
- The emotional trajectory

No stanza structure at this stage—just the storytelling idea.

When generating concepts, explicitly explore different:
- Narrative structures (linear, in medias res, frame story, revelation)
- Emotional tones (comic, tragic, wistful, triumphant)
- Points of view (protagonist, observer, omniscient)
- Temporal approaches (single moment, journey, before/after)

#### Step 1c: Concept Evaluation
Evaluate each concept 1-10 against these criteria:

| Criterion | Weight | Description |
|-----------|--------|-------------|
| Narrative tension | High | Conflict, stakes, movement |
| Verse suitability | High | Natural stanza segmentation, key moments landing on strong beats |
| Emotional arc | Medium | Clear shift (hope→despair, mystery→revelation, etc.) |
| Originality | Medium | Avoids well-trodden paths for this subject |
| Imagery potential | Medium | Suggests concrete, evocative scenes |

Present ranked concepts with scores and brief rationale for top 3.

#### Step 1d: User Selection
**STOP and wait for user to select concept** (or request modifications/new concepts).

#### Step 1e: Full Outline Development
Develop selected concept into full outline:
- **Narrative arc summary** (2-3 sentences)
- **Form/meter choice**: Select using the Form Selection Guide in [reference.md](reference.md). Verify against the Form Selection Checklist before proceeding. Explain why this form suits the content, speaker, and tone.
- **Stanza breakdown**: What each stanza accomplishes narratively
- **Rough character estimate**: Warn early if plan exceeds ~4500 chars

#### Step 1f: User Approval
**STOP and wait for user approval** before proceeding to generation. User may:
- Approve
- Modify (add/cut stanzas, change emphasis, redirect narrative)
- Change form
- Abort

### Phase 2: Generation (Two-Pass with Critique Loop)

#### Pass 1: Initial Generation (Preserve Momentum)

For each stanza sequentially:

1. **Select exemplar(s)** from `exemplars/` folder matching form/tone, inject into context
2. **Pull rhyme options** using the rhyme dictionary script with metrical filtering
3. **Draft stanza** with full phonetic option space visible
4. **Verify meter** using the meter checker script
5. **Revise loop** until meter passes
6. Proceed to next stanza

Complete all stanzas before moving to Pass 2.

#### Pass 2: Contextual Critique-Rewrite Loop

**Step 2a: Full Poem Read**
Read the complete poem as a whole to understand flow and context.

**Step 2b: Per-Stanza Critique**
For each stanza, critique against these criteria:

| Criterion | What to Look For |
|-----------|------------------|
| Narrative effectiveness | Does it advance the story? Does it earn its place? Does it connect to what comes before/after? |
| Emotional impact | Does it land? Is the feeling earned or forced? Is tone consistent with arc position? |
| Cliche detection | Stock phrases ("heart of gold", "dark as night", "stood tall"), tired imagery, predictable turns |
| Forced phrasing | Inverted syntax without poetic purpose, padding words ("so", "very", "did"), unnatural word order solely to hit rhyme |

Rate each criterion: **None** / **Minor** / **Substantial**

**Step 2c: Rewrite Based on Critique**
- If critique finds issues, rewrite the stanza addressing them
- Re-verify meter after rewrite
- Show critique reasoning and rewrite rationale visibly (useful for understanding refinement)

**Step 2d: Assess Rewrite Scope**
- **Minimal** (<25% of words changed, same rhyme endpoints, same core imagery): Done with this stanza
- **Substantial** (new rhyme words, restructured imagery, >25% rewritten, different emotional beat): Return to Step 2b for another critique pass

**Step 2e: Pass Limit**
Maximum **5 critique-rewrite passes** per stanza. After 5, accept current version and note any remaining concerns.

### Phase 3: Assembly

1. Concatenate all stanzas
2. **Length check**: Must be under 5000 characters
3. If over, identify flabby stanzas and tighten (compress, don't truncate)
4. **Save final poem** to `drafts/poetry/` in the repo root (NOT inside `.claude/skills/`)

## Output Directory

**CRITICAL**: Save all poem drafts to `drafts/poetry/` in the **repository root directory**.

- Correct path: `drafts/poetry/poem_name.md`
- WRONG path: `.claude/skills/narrative-poetry/drafts/` (this is inside the skill folder - DO NOT USE)

The skill folder (`.claude/skills/narrative-poetry/`) is for skill configuration only, not for output. Writing to the skill folder requires approval for each write and clutters the skill directory.

## Tools

### Rhyme Dictionary

Query rhymes with metrical constraints:

```bash
python .claude/skills/narrative-poetry/scripts/rhyme_lookup.py <word> [--syllables N] [--stress-pattern PATTERN]
```

Examples:
```bash
# All rhymes for "night"
python .claude/skills/narrative-poetry/scripts/rhyme_lookup.py night

# 2-syllable rhymes with stress on first syllable (trochaic)
python .claude/skills/narrative-poetry/scripts/rhyme_lookup.py night --syllables 2 --stress-pattern 10

# Iambic (stress on second syllable)
python .claude/skills/narrative-poetry/scripts/rhyme_lookup.py desire --syllables 2 --stress-pattern 01
```

Stress patterns: 0 = unstressed, 1 = primary stress, 2 = secondary stress

### Meter Verification

Check a line's scansion:

```bash
python .claude/skills/narrative-poetry/scripts/meter_check.py "<line>" [--expected-pattern PATTERN]
```

Examples:
```bash
# Check scansion of a line
python .claude/skills/narrative-poetry/scripts/meter_check.py "The curfew tolls the knell of parting day"

# Verify against expected iambic pentameter
python .claude/skills/narrative-poetry/scripts/meter_check.py "The curfew tolls the knell of parting day" --expected-pattern 0101010101
```

Returns: syllable count, stress pattern, and whether it matches expected meter.

## Exemplar Selection

Exemplars are stored in `exemplars/` subdirectory, organized by form:

```
exemplars/
├── ballad/           # Common meter (8686), narrative focus
├── blank-verse/      # Unrhymed iambic pentameter
├── heroic-couplet/   # Rhymed iambic pentameter pairs
├── ottava-rima/      # 8-line stanzas, ABABABCC
└── misc/             # Other forms, miscellaneous
```

Select exemplars based on:
- **Form match**: Pull from the relevant form directory
- **Tone match**: Darker subjects → different poems than comic subjects
- **Era hint**: If user mentions "Victorian" or "medieval", weight accordingly

Inject 1-2 short exemplars (or excerpts) into context before drafting. This shifts vocabulary and syntactic patterns away from modern cliches.

## Stock Rhyme Warnings

The file `stock_rhymes.txt` contains cliched rhyme pairs. These are soft warnings, not hard blocks - context matters. A rhyme that's tired in love poetry may be fine in satire.

Hard-avoid pairs (almost always groan-inducing):
- moon / June / spoon / croon
- love / dove / above
- heart / apart (in romantic context)

Contextual warnings (cliched in specific genres):
- fire / desire
- night / light / bright / sight
- eyes / skies / lies
- kiss / bliss / miss
- dreams / seems
- tears / fears / years

If you find yourself reaching for these, query the rhyme dictionary for alternatives first.

## Key Principles

1. **Rhyme dictionary comes FIRST, not last.** Query options before drafting so you compose with known-good endpoints. This prevents cliche by expanding visible option space.

2. **The plan does real work.** Committing to narrative structure before wrestling with rhyme/meter prevents sound from dictating story.

3. **Exemplars shift the distribution.** Older poetry primes away from modern amateur patterns. Victorian stock phrases sound fresh because they've fallen out of use.

4. **Meter check per stanza.** Catch problems before they compound. Each stanza is metrically independent.

5. **Tighten, don't truncate.** If over 5000 chars, find flabby stanzas and compress them. Cutting whole stanzas breaks narrative arc.

## Form Quick Reference

| Form | Meter | Rhyme Scheme | Best For |
|------|-------|--------------|----------|
| Ballad (common meter) | 8/6/8/6 iambic | ABAB or ABCB | Folk narrative, dark irony |
| Ballad (long/fourteeners) | 14+ syllables | AABB | Tall tales, yarn-spinning, comedy |
| Anapestic tetrameter | da-da-DUM x4 (12 syl) | AABB | Breathless comedy, hucksters, manic energy |
| Blank verse | Iambic pentameter | None | Serious drama, meditation |
| Heroic couplet | Iambic pentameter | AA BB CC | Serious satire, authority (NOT light comedy) |
| Ottava rima | Iambic pentameter | ABABABCC | Comic epic, witty narrative |
| Trochaic tetrameter | DUM-da x4 | varies | Spells, myths, incantation |
| Patter | Rapid trochaic/mixed | AABB | Gilbert & Sullivan, absurdist lists |

**Critical:** Form carries tonal information. A mismatch between form and content creates friction. See [reference.md](reference.md) for the full Form Selection Guide, including:
- Meter vibes (what each meter *feels like*)
- Matching form to comedy vs. serious content
- Character voice and meter
- Form/content mismatches to avoid
- Form Selection Checklist
