---
name: narrative-poetry
description: Generate narrative poetry with proper rhyme and meter. Use when asked to write poetry, verse, ballads, or song lyrics. Includes metrically-aware rhyming dictionary, meter verification, and curated exemplars.
allowed-tools: Read, Bash(python:*), Glob, Grep
---

# Narrative Poetry Generation

Generate high-quality narrative poetry with verified rhyme and meter. Output must be under 5000 characters (suitable for song lyrics, though generate as poetry, not "lyrics").

## Workflow Overview

### Phase 1: Planning (User Checkpoint Required)

1. Gather from user:
   - Subject/story
   - Optional: form preference (ballad, blank verse, sonnet sequence, etc.)
   - Optional: exemplar hint (tone, era, poet style)

2. Generate and present to user:
   - **Narrative arc summary** (2-3 sentences)
   - **Stanza breakdown**: What each stanza accomplishes narratively
   - **Form/meter choice**: Explain the prosodic structure
   - **Rough character estimate**: Warn early if plan exceeds ~4500 chars

3. **STOP and wait for user approval** before proceeding. User may:
   - Approve
   - Modify (add/cut stanzas, change emphasis, redirect narrative)
   - Change form
   - Abort

### Phase 2: Per-Stanza Generation

For each stanza:

1. **Select exemplar(s)** from `exemplars/` folder matching form/tone, inject into context
2. **Pull rhyme options** using the rhyme dictionary script with metrical filtering
3. **Draft stanza** with full phonetic option space visible
4. **Verify meter** using the meter checker script
5. **Revise loop** until meter passes
6. Proceed to next stanza

### Phase 3: Assembly

1. Concatenate all stanzas
2. **Length check**: Must be under 5000 characters
3. If over, identify flabby stanzas and tighten (compress, don't truncate)
4. Output final poem

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

| Form | Meter | Rhyme Scheme | Notes |
|------|-------|--------------|-------|
| Ballad (common meter) | 8/6/8/6 syllables | ABAB or ABCB | Strong narrative tradition |
| Ballad (long meter) | 8/8/8/8 | ABAB | More room per stanza |
| Blank verse | Iambic pentameter | None | Shakespeare, Milton |
| Heroic couplet | Iambic pentameter | AA BB CC | Pope, Dryden |
| Ottava rima | Iambic pentameter | ABABABCC | Byron's Don Juan |
| Rhyme royal | Iambic pentameter | ABABBCC | Chaucer, Wyatt |
| Spenserian | Iambic pentameter + alexandrine | ABABBCBCC | Spenser, Keats |

For detailed reference on forms and prosody, see [reference.md](reference.md).
