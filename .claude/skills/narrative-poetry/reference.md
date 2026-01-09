# Narrative Poetry Reference

Detailed prosody reference for the narrative-poetry skill.

## Stress Notation

The CMU Pronouncing Dictionary uses:
- **0** = unstressed (schwa, reduced vowels)
- **1** = primary stress
- **2** = secondary stress

For most metrical purposes, treat 2 as flexible—it can function as stressed or unstressed depending on context.

## Common Metrical Feet

| Foot | Pattern | Example |
|------|---------|---------|
| Iamb | 01 | de-LIGHT |
| Trochee | 10 | GAR-den |
| Spondee | 11 | HEART-BREAK |
| Pyrrhic | 00 | of the |
| Dactyl | 100 | MER-ri-ly |
| Anapest | 001 | in-ter-VENE |
| Amphibrach | 010 | a-MAZE-ment |

## Line Lengths

| Name | Feet | Syllables (iambic) |
|------|------|-------------------|
| Monometer | 1 | 2 |
| Dimeter | 2 | 4 |
| Trimeter | 3 | 6 |
| Tetrameter | 4 | 8 |
| Pentameter | 5 | 10 |
| Hexameter | 6 | 12 |
| Heptameter | 7 | 14 |

## Forms in Detail

### Ballad Meter (Common Meter)

Alternating tetrameter and trimeter lines: 8/6/8/6 syllables.
Rhyme scheme typically ABAB or ABCB.

```
The king sits in Dunfermline town,    (8)
    Drinking the blood-red wine;       (6)
"O where will I get a skeely skipper   (8)
    To sail this ship of mine?"        (6)
```

**Long meter** variant: 8/8/8/8 (all tetrameter).
**Short meter** variant: 6/6/8/6.

### Blank Verse

Unrhymed iambic pentameter. The workhorse of English dramatic and narrative poetry.

```
Of Man's first disobedience, and the fruit
Of that forbidden tree whose mortal taste
Brought death into the World, and all our woe
```

Allows significant metrical variation:
- **Feminine endings** (extra unstressed syllable)
- **Initial trochaic substitution** (stressed first syllable)
- **Pyrrhic + spondee** combinations
- **Caesura** (mid-line pause)

### Heroic Couplet

Rhymed iambic pentameter in pairs. Dominant form of 18th-century poetry.

```
True wit is nature to advantage dressed,
What oft was thought, but ne'er so well expressed.
```

**Closed couplets**: Each couplet is a complete syntactic unit.
**Open/enjambed couplets**: Syntax flows across couplet boundaries.

### Ottava Rima

Eight-line stanzas in iambic pentameter, rhyming ABABABCC.
The final couplet often delivers a punchline or turn.

Byron's *Don Juan* is the canonical English example:

```
I want a hero: an uncommon want,
When every year and month sends forth a new one,
Till, after cloying the gazettes with cant,
The age discovers he is not the true one;
Of such as these I should not care to vaunt,
I'll therefore take our ancient friend Don Juan—
We all have seen him, in the pantomime,
Sent to the devil somewhat ere his time.
```

### Rhyme Royal

Seven-line stanzas in iambic pentameter, rhyming ABABBCC.
Used by Chaucer (*Troilus and Criseyde*) and later poets.

### Spenserian Stanza

Nine lines: eight iambic pentameter + one alexandrine (hexameter).
Rhyme scheme ABABBCBCC.

The interlocking rhymes create forward momentum; the closing alexandrine provides weight.

## Metrical Variation

Strict adherence to meter sounds mechanical. Good verse uses controlled variation:

### Acceptable Substitutions

1. **Initial trochee** in iambic line:
   `GATH-er ye | rose-BUDS | while YE | may`

2. **Pyrrhic-spondee pair** (redistributed stress):
   `of the | DARK WOODS`

3. **Feminine ending** (extra unstressed syllable):
   `To BE | or NOT | to BE | that IS | the QUES-tion`

4. **Elision** (synaloepha):
   "the evening" can scan as 2 or 3 syllables depending on speed

### Problematic Patterns

1. **Two consecutive stressed or unstressed syllables** in iambic verse (unless intentional spondee/pyrrhic)

2. **Consistent metrical inversion** mid-line (reads as different meter)

3. **Syllable count off by more than 1** from expected

## Rhyme Types

### Full Rhyme (Perfect Rhyme)
Identical sounds from the stressed vowel onward: light/night, desire/fire

### Slant Rhyme (Near Rhyme)
Close but not identical: love/move, eyes/voice

### Eye Rhyme
Looks like it should rhyme but doesn't: love/prove, wind/mind

### Rich Rhyme (Rime Riche)
Homophones: their/there, night/knight

### Feminine Rhyme
Two-syllable rhyme with stress on first: running/cunning, nation/station

### Dactylic Rhyme
Three-syllable rhyme: merrily/verily, tenderly/slenderly

## The 5000 Character Limit

For reference, approximate lengths:
- Sonnet (14 lines): ~600-800 characters
- Ballad stanza (4 lines): ~150-250 characters
- 20 stanzas of ballad: ~4000-5000 characters

A narrative poem of 16-24 quatrains is a reasonable target. Plan accordingly in Phase 1.

## Workflow Checklist

### Phase 1: Planning
- [ ] Narrative arc summary written
- [ ] Stanza-by-stanza breakdown complete
- [ ] Form/meter chosen
- [ ] Character estimate calculated
- [ ] User has approved plan

### Phase 2: Per-Stanza Generation
- [ ] Exemplar selected and in context
- [ ] Rhyme options queried for line-ending words
- [ ] Stanza drafted
- [ ] Meter verified (all lines pass)
- [ ] Proceed to next stanza

### Phase 3: Assembly
- [ ] All stanzas concatenated
- [ ] Total under 5000 characters
- [ ] No flabby stanzas remaining
- [ ] Final poem output
