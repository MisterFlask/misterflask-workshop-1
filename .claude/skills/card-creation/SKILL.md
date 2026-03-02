# Deckbuilder Card Creation

A focused skill for iteratively designing individual cards for Slay the Spire-style deckbuilder roguelikes. Can be invoked standalone or as part of a larger class creation workflow.

## Invocation

This skill can be invoked with context:
- **Standalone**: `/card-creation` - will ask for class context
- **With context**: Called from another skill with class mechanics and existing card pool provided

When invoked, expect to receive (or ask for):
1. Class mechanical identity (unique resource, gimmick, play pattern)
2. Current card pool (to avoid duplication)
3. Target rarity for this batch
4. Number of cards needed

---

## Card Creation Protocol

### Step 1: Ideation (Generative)

Generate **5 card ideas** that fit the target rarity and class theme. For each idea:

```
[Number]. **[Card Name]** - [One-line mechanical description]
   Rationale: [Why this card is interesting. What decision does it create?
   How does it interact with the class mechanic? What role does it fill?]
```

Ideas should vary across:
- Offensive / defensive / utility
- Immediate value / setup+payoff
- Class mechanic synergy / general goodstuff
- Different target archetypes within the class

**If a card concept requires a token** (a card created by another card, not drafted), note this in the rationale. Tokens will be designed inline with their parent card.

### Step 2: Selection

Select **1-2 cards** to develop based on:
- Fills a gap in the current pool
- Creates interesting combat decisions
- Has synergies without being parasitic (works alone, better with support)
- Appropriate power for rarity

State selection and brief reasoning.

### Step 3: Full Card Design

For each selected card:

```
### [Card Name]
- **Cost**: [X] Energy
- **Type**: Attack / Skill / Power
- **Rarity**: Common / Uncommon / Rare
- **Effect**: [Full effect text]
- **Upgrade ([Card Name]+)**: [What changes]

**Scaling Analysis**:
- Primary scaling: [How does this get stronger? Strength? Draw? Class mechanic?]
- Scaling curve: [Linear / Multiplicative / Threshold-based]
- Floor: [Minimum value in bad situations]
- Ceiling: [Maximum value with good setup]
- Sweet spot: [When is this card at its best?]
```

**If the card creates tokens**, design them immediately:

```
#### Token: [Token Name]
- **Cost**: [X] Energy
- **Type**: Attack / Skill
- **Effect**: [Effect text]
- **Notes**: [Created by [Parent Card]. Not added to deck/draft pool.]
```

### Step 4: Balance Critique

#### Reference Baselines (Slay the Spire)
| Cost | Attack Damage | Block | Notes |
|------|---------------|-------|-------|
| 0 | 3-4 | 3-4 | Very conditional or downside |
| 1 | 6-8 | 5-6 | Common baseline |
| 2 | 10-14 | 8-12 | Uncommon baseline |
| 3 | 16-20 | 14-18 | Rare baseline |

| Effect | Approximate Value |
|--------|------------------|
| Draw 1 card | ~0.5-1 energy |
| Apply 1 Vulnerable | ~0.5 energy/turn |
| Apply 1 Weak | ~0.5 energy/turn |
| Exhaust (cost) | ~-0.5 energy |
| Exhaust (benefit) | Varies by card |
| Retain | ~0.25 energy |

#### Critique Checklist

**1. Floor Check**
> Is this ever strictly worse than Strike/Defend with no compensating upside?
- If YES: Card needs a buff or situational upside
- If NO: Pass

**2. Ceiling Check**
> Can this trivially break the game with common synergies?
- Consider: What if you have 3 copies? What with obvious relics? What with the class's core mechanic at high stacks?
- If BROKEN: Needs a cap, cost increase, or redesign
- If STRONG BUT FAIR: Pass

**3. Parasitism Check**
> Is this card dead without specific other cards?
- Mild synergy requirements (e.g., "better with discard") = OK
- Hard requirements (e.g., "does nothing without [specific rare]") = Problem
- If PARASITIC: Add baseline functionality or redesign
- If SELF-SUFFICIENT: Pass

**4. Comparison Test**
> What existing StS card is this most similar to? Is it fairly costed?
- Name the comparison card
- Note differences in cost/effect
- If OVERTUNED or UNDERTUNED: Adjust
- If COMPARABLE: Pass

**5. Fun Check**
> Does this create interesting decisions?
- "Always play this" = Boring, needs downside or cost increase
- "Never play this" = Weak, needs buff
- "Depends on board state" = Good
- "Interesting deckbuilding tension" = Great

#### Verdict

- **PASS**: Card is balanced and interesting. Record it.
- **SALVAGE**: Core idea is good but numbers/implementation need adjustment. Revise in Step 3 and re-critique.
- **REJECT**: Fundamental design problem (unfun, unfixable parasitism, doesn't fit class). Return to Step 1 with notes on what to avoid.

### Step 5: Record

Add approved card(s) to the card pool. Format:

```
✓ [Card Name] ([Rarity]) - [One-line summary]
```

Then either:
- Continue to next batch (return to Step 1)
- Report completion if target count reached

---

## Batch Flow

For efficient card creation, work in batches:

1. **Generate** 5 ideas (Step 1)
2. **Select** 1-2 to develop (Step 2)
3. **Design** full cards (Step 3)
4. **Critique** each card (Step 4)
5. **Record** approved cards (Step 5)
6. **Repeat** until batch target reached

**User Checkpoint**: After each batch of 3-5 approved cards, pause and show the user:
- Cards created this batch
- Running total for this rarity
- Any concerns or patterns noticed

User can then:
- Approve and continue
- Request adjustments to direction
- Reject specific cards for revision

---

## Rarity Guidelines

### Common Cards
- Simple, clear effects
- Form the backbone of basic strategies
- Should include: attacks, blocks, draw, basic class mechanic interaction
- Avoid: Complex conditionals, multi-step combos, build-around effects

### Uncommon Cards
- More complex or conditional effects
- Enable specific archetypes
- Can have mild build-around elements
- Should include: Archetype payoffs, interesting decisions, class mechanic depth

### Rare Cards
- Powerful, often build-defining
- Can be complex
- Should feel exciting to see in a reward screen
- Should include: Splashy effects, strong class mechanic synergy, potential win conditions

### Tokens
- Created by other cards, never drafted
- Usually simpler than their rarity would suggest
- Often temporary (Exhaust, Ethereal)
- Power level: Can be efficient since they cost a card to generate

---

## Output Format

When creating cards for another skill, return them in this format:

```
## [Rarity] Cards (Batch [N])

### [Card Name]
[Full card block from Step 3]

### [Card Name 2]
[Full card block]

---
Batch Summary:
- Cards approved: [list]
- Cards rejected: [list with reasons]
- Pool gaps identified: [any holes noticed]
- Suggested next batch focus: [if applicable]
```
