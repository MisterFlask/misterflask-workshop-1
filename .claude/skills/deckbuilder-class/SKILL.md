# Deckbuilder Roguelike Character Class Creator

Create complete character classes for Slay the Spire-style deckbuilder roguelikes, including flavor, mechanics, starter deck, and full card pool. Emphasizes dark humor in the Fallen London tradition.

**Depends on**: `card-creation` skill for card design protocol

---

## Critical Design Principles

Before generating concepts, internalize these hard-won lessons:

### 1. Use Universal Mechanics, Not Bespoke Tracking

**The test**: Can a card from this class be picked up by another class and still make sense?

**Bad**: "Gain 3 Insight" (requires class-specific Insight tracking)
**Good**: "Apply 2 Vulnerable" (universal debuff, works in any deck)

Classes should be defined by *how they use universal mechanics*, not by unique resources:
- **Universal effects**: Strength, Dexterity, Weak, Vulnerable, Poison, Block, HP
- **Universal deck manipulation**: Draw, Discard, Exhaust, Retain, Ethereal
- **Universal card properties**: Cost, card type (Attack/Skill/Power), multi-hit

Study how Ironclad and Silent work: their identities come from *leaning into* universal systems (Exhaust, Strength, Poison, Weak) rather than inventing new tracking.

### 2. Design for Mixed Decks

If the game involves multiple characters whose cards combine (like a party system), every card must function even when most of the deck is from other classes. Synergies should emerge from shared universal keywords, not parasitic class-specific mechanics.

**Packmaster lesson**: Each "pack" is a self-contained mini-archetype using universal keywords, so packs can combine freely.

**Silent lesson**: Poison, Shivs, and Discard are three separate archetypes within one class—they can hybrid (Shiv+Poison via Envenom) because they share common ground (playing many cards).

### 3. Give Each Class a Character Flaw

Flaws create:
- **Narrative identity**: The character is a *person*, not a stat block
- **Mechanical direction**: The flaw suggests what the class is *bad* at
- **Team composition logic**: Classes cover each other's weaknesses
- **Dark humor hooks**: The flaw is the source of comedy

Examples:
- **Rage**: Hits hard but can't stop, burns out (Exhaust, self-damage)
- **Cowardice**: Excellent support, struggles alone (Draw, Retain, low damage)
- **Alcoholism**: Powerful but inconsistent (high variance, curses/downsides)
- **Obsession**: Single-target master, punished by AoE fights
- **Pride**: All offense, minimal defense (pays in HP)

### 4. Ground Attacks in Concrete Actions

Attacks should represent *what this person physically does in combat*. Skills and Powers can be more abstract (wealth, connections, knowledge), but attacks need a clear physical or direct source.

**Bad**: "Hired Muscle" — implies third parties doing the fighting
**Good**: "Fencing Lessons" — the character personally trained, personally strikes

This means each class needs a **concrete combat skill**:
- The Heiress fences (épée, precise lunges, ripostes)
- The Veteran brawls (rifle butt, trench knife, bare hands)
- The Occultist speaks words of power (incantations, banishments)

---

## Workflow Overview

### Phase 1: Class Concept Generation

#### Step 1a: Generate Class Concepts
Generate **exactly 10 distinct character class concepts**. Each concept:

```
[Number]. **[Class Name]** — *Flaw: [Character Flaw]*
- Flavor: [2-3 sentences. Dark humor angle, narrative identity, who IS this person?]
- Identity: [What universal mechanics do they favor? What's their combat skill?]
- The flaw in play: [How does the flaw manifest as mechanical weakness?]
```

**Distinctness Requirements**:
- No overlap with Slay the Spire classes (Ironclad's exhaust/self-damage, Silent's shivs/poison, Defect's orbs/focus, Watcher's stances/scry)
- No straight fantasy archetypes unless significantly twisted
- Each concept should suggest a genuinely different play pattern
- Embrace Fallen London sensibility (see Tone Guide below)
- **Use only universal mechanics** — no bespoke tracking systems
- **Include a character flaw** that creates mechanical weakness

#### Step 1b: User Selection
**STOP. Wait for user to select a concept** or request modifications/new concepts.

---

### Phase 2: Class Development

#### Step 2a: Full Class Identity

Develop the selected concept into:

```
## [Class Name]

### Lore
[1-2 paragraphs. Who is this person? What's their deal? Why are they in the Spire?
Dark humor, specific weird details, understated horror.]

### Character Flaw: [Flaw Name]
[How does this flaw manifest in their personality? How does it create problems?
This should inform both narrative flavor and mechanical weakness.]

### Combat Identity: [Concrete Skill]
[What does this person PHYSICALLY DO in combat? Be specific.
- The Heiress: Fences. Trained in épée, proper form, athletic precision.
- The Veteran: Brawls. Rifle butt, trench knife, brutal efficiency.
- The Archivist: Recites. Protective wards, banishing words, scholarly incantations.

This grounds all Attack cards. Skills/Powers can be more abstract.]

### Mechanical Identity
[What universal mechanics does this class favor?
- Which debuffs? (Weak, Vulnerable, Poison)
- Which scaling? (Strength, Dexterity, Powers-in-play)
- Which deck manipulation? (Draw, Exhaust, Retain)
- What cost profile? (Cheap spam, expensive bombs, mixed)
- What card type ratio? (Attack-heavy, Skill-heavy, Power-heavy)]

### Play Pattern
[How does combat typically flow for this class?
- Early turns: What are you doing?
- Mid-combat: What decisions matter?
- Late combat/scaling: How do you close out fights?]

### Intended Feel
[What emotions/experiences should playing this class evoke?
Is it methodical? Chaotic? Risk/reward? Puzzle-like?]
```

#### Step 2b: Define Target Archetypes

Before creating any cards, define **3-4 distinct deck archetypes** this class should support:

```
### Archetype 1: [Name]
- Strategy: [How does this deck try to win?]
- Key mechanics: [What effects does it want?]
- Play pattern: [Fast/slow? Defensive/aggressive?]
- Critical mass needs: [What cards MUST exist for this to work?]

### Archetype 2: [Name]
[etc.]
```

These archetypes guide card creation. Each card should support at least one archetype (or be flexible "goodstuff").

#### Step 2c: Starter Relic

```
### Starter Relic: [Name]
[Effect text]

Design notes: [Why this relic? How does it shape early decisions?]
```

#### Step 2d: User Checkpoint
**STOP. Present class identity, archetypes, and starter relic for user approval** before proceeding to cards.

---

### Phase 3: Starter Deck

Create the starter deck. **This can deviate from the Strike/Defend template** if the class mechanic demands it.

**Default template** (use as baseline, not requirement):
- 4-5 basic attacks
- 4-5 basic defensive cards
- 0-2 signature cards showcasing the class mechanic

**Alternative structures** (if mechanically appropriate):
- Asymmetric (more attacks, less defense or vice versa)
- Mechanic-heavy (multiple signature cards, fewer basics)
- Unusual card types (starting Powers, starting tokens in hand)

For each starter card:

```
### [Card Name]
- **Cost**: [X] Energy
- **Type**: Attack / Skill / Power
- **Rarity**: Starter
- **Effect**: [Effect text]
- **Upgrade**: [What changes]
```

**Starter Deck Checkpoint**: Show user the complete starter deck before proceeding to card pool.

---

### Phase 4: Card Pool Creation

Use the **Card Creation skill** (`card-creation`) iteratively to build the pool.

#### Card Flavor Constraints

**Attacks** must be grounded in the class's concrete combat skill:
- If the class fences, attacks are lunges, ripostes, feints, combinations
- If the class brawls, attacks are punches, headbutts, improvised weapons they personally wield
- NO "Hired Muscle" or "Summoned Creature" attacks—the character does the fighting

**Skills** can be more abstract:
- Social advantages, wealth, knowledge, preparation
- Defensive maneuvers, evasion, misdirection
- Setup effects, positioning, resource manipulation

**Powers** can be fully abstract:
- Lasting advantages from background, training, connections
- Ongoing effects from personality traits or knowledge
- Passive benefits that "just happen" due to who this person is

#### Target Pool Size
- **Common**: 6 cards
- **Uncommon**: 6 cards
- **Rare**: 6 cards
- **Tokens**: Created inline with parent cards (not a separate target)

After completing all 18 cards, conduct a gap review with the user. Additional cards can be added to address identified weaknesses.

#### Creation Order
1. Commons first (establish baseline tools)
2. Uncommons second (archetype enablers)
3. Rares last (payoffs and build-arounds)

#### Batch Structure
Create all 18 cards (6 per rarity) in sequence. User review happens after the full pool is complete, during the Pool Coherence Review.

#### Pool Coherence Review

After all cards are drafted, review the complete pool:

```
### Pool Health Check

**Archetype Support**:
- [Archetype 1]: [Well-supported / Needs work / Critical gap]
  - Key cards: [list]
  - Missing: [if any]
- [Archetype 2]: [etc.]

**Functional Coverage**:
- Single-target damage: [Adequate / Light / Heavy]
- AOE damage: [Adequate / Light / Heavy]
- Block: [Adequate / Light / Heavy]
- Draw/cycling: [Adequate / Light / Heavy]
- Scaling: [Adequate / Light / Heavy]
- Class mechanic interaction: [Adequate / Light / Heavy]

**Trap Card Check**:
[Any cards that look appealing but have no support? Flag them.]

**Redundancy Check**:
[Any cards that are too similar? Flag for differentiation or cutting.]
```

Address any issues identified before final assembly.

---

### Phase 5: Final Assembly

Compile everything into the output document:

```markdown
# [Class Name]

## Lore
[Full lore text]

## Core Mechanic: [Name]
[Full mechanic explanation]

## Play Patterns
[Play pattern description]

## Archetypes
[List of 3-4 archetypes with brief descriptions]

---

## Starting Loadout

### Starter Relic: [Name]
[Effect]

### Starter Deck ([N] cards)
[Full card list with stats]

---

## Card Pool

### Common Cards ([N] cards)
[Full card list, alphabetical]

### Uncommon Cards ([N] cards)
[Full card list, alphabetical]

### Rare Cards ([N] cards)
[Full card list, alphabetical]

### Tokens
[Token cards with notes on what creates them]

---

## Design Notes

### Key Synergies
[Notable card combinations]

### Archetype Breakdown
[Which cards support which archetypes]

### Balance Considerations
[Any known concerns, things that need playtesting]

### Cards That Didn't Make It
[Optional: interesting ideas that were cut and why]
```

**Save to**: `drafts/character_classes/[class-name-slug].md`

---

## Tone Guide: Dark Humor Done Right

Channel Fallen London's specific flavor of gothic absurdism.

### Do This

**Understatement**: Horror described in polite, bureaucratic, or mundane terms.
> "Polite Exsanguination" - A skill that drains enemy health
> "The Committee's Regards" - An attack implying sinister unseen forces

**Absurdist Logic**: Systems that make internal sense but are fundamentally unhinged.
> A class that gains power from accumulated regrets
> A mechanic where you're literally arguing with your own cards

**Specific Weird Details**: Concrete oddities rather than vague spookiness.
> "Your Aunt's Disappointed Shade" not "A Haunting Spirit"
> "The Incident at the Admiralty" not "A Dark Secret"

**Black Comedy**: Finding humor in the macabre through tone, not content.
> Card flavor text: "The Ministry assures you this is survivable. The Ministry has been wrong before."

### Don't Do This

- **Grimdark edginess**: No "BLOOD DEATH PAIN" energy
- **Random humor**: No "holds up spork" nonsense
- **Generic names**: No "Dark Blast" or "Shadow Strike"
- **Exclamation-point comedy**: If it needs "!" to be funny, it isn't
- **Trying too hard**: The humor comes from restraint, not escalation

### Exemplar Card Names (For Reference)

Good names that hit the tone:
- "A Firm Handshake" (attack that also applies Weak)
- "Reasonable Accommodations" (block + draw)
- "The Appropriate Paperwork" (Power that generates bureaucratic tokens)
- "Someone Else's Problem" (redirect damage)
- "Professional Courtesy" (attack that's stronger vs. elites)
- "The Bill Comes Due" (delayed damage effect)
- "Sincere Condolences" (kill effect with polite flavor)
- "Prior Engagement" (Retain + bonus if not played)

Bad names to avoid:
- "ULTIMATE DEATH STRIKE"
- "Spooky Ghost Attack"
- "lol random card"
- "Dark Darkness of the Dark"

---

## Output Location

Save final class document to: `drafts/character_classes/[class-name-slug].md`
