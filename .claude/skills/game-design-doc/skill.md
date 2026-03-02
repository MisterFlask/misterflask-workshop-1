# Game Design Document Creator

An iterative skill for creating comprehensive game design documents through collaborative refinement.

## How This Skill Works

This skill follows a structured iterative process:

### Phase 1: Concept Exploration
When given a game concept and constraints, generate **exactly 10 distinct elaborations** on that idea. Each elaboration should:
- Take the core concept in a meaningfully different direction
- Be presented as a numbered list (1-10)
- Include a short title and 2-3 sentence description
- Vary across dimensions like: tone, scope, core mechanic emphasis, target audience, complexity level, genre fusion

Present these and ask the user to pick one (or combine elements).

### Phase 1.5: Gameplay Crystallization (Telescoping)
Once the user selects a direction, **do not immediately write the full document**. First, generate **5-7 brief gameplay sketches** that explore different ways the chosen concept could actually play. Each sketch should be 1-2 paragraphs covering:

- **Moment-to-moment gameplay**: What does the player actually *do* second-to-second?
- **Core loop**: What's the repeated cycle of play?
- **Critical decision points**: Where does the player make meaningful choices? What makes those choices interesting?
- **Player agency**: How does the player express skill/strategy/creativity? What levers do they pull?
- **Session arc**: How does a single play session feel from start to finish?

After generating these sketches:

1. **Rank them** according to:
   - Alignment with the chosen concept's strengths
   - Depth of interesting decisions
   - Clarity of player agency
   - Fun factor / engagement potential
   - Feasibility within stated constraints

2. **Select the best sketch** (or synthesize the strongest elements from multiple sketches)

3. **Explain the ranking and selection** briefly

4. **Present to user for approval** - The user must approve the crystallized gameplay vision before proceeding. This is a significant commitment point. They may request modifications or ask you to go with a different sketch.

Only after user approval should you move to Phase 2.

### Phase 2: Initial Design Document
With the crystallized gameplay vision in hand, create a comprehensive design document covering:

```
# [Game Title]

## Core Concept
[One paragraph elevator pitch]

## The Fun
[Direct statement: "The fun comes from..." - This is the north star. What is the core source of enjoyment?]

## Design Pillars
[3-5 core principles that guide all design decisions]

## Core Loop
[The fundamental repeated gameplay cycle - now informed by Phase 1.5]

## Moment-to-Moment Gameplay
[What the player is actually doing at any given moment]

## Critical Decision Points
[Where meaningful choices happen and what makes them interesting]

## Player Agency Model
[How players express skill, strategy, and creativity]

## Player Experience Goals
[What emotions/experiences the game should evoke]

## Mechanics

### Primary Mechanics
[Detailed breakdown of main systems]

### Secondary Mechanics
[Supporting systems]

### Progression Systems
[How players advance/grow]

## Content

### [Relevant content categories - units, levels, items, etc.]
[Specific definitions with all numbers decided]

## Economy/Resources
[If applicable - specific values included]

## Balance Framework
[How systems interact, intended balance points]

## Win/Loss Conditions
[Clear victory and defeat states]

## Session Structure
[How a typical play session unfolds]

## Scope Notes
[What's in, what's explicitly out]

## Risk Register
[Known unknowns and critical uncertainties. What will this design live or die on? What's going to be tricky to tune? Where are the "this needs playtesting" flags?]

## Minimum Viable Slice
[The smallest playable version that tests the core fun hypothesis. What's the first prototype scope?]
```

**Important**: Make all numerical decisions yourself. Do not ask the user about specific values, costs, stats, timings, or quantities. Use your judgment to create balanced, coherent numbers.

### Phase 3: Self-Critique Cycle
After creating the document, perform rigorous self-review:

1. **Gap Analysis**: Are there missing systems? Undefined interactions? Unclear rules?
2. **Consistency Check**: Do numbers work together? Do mechanics support the design pillars?
3. **Problem Detection**: Are there obvious exploits? Degenerate strategies? Unfun patterns?
4. **Completeness Review**: Could a developer implement this without major questions?
5. **Gameplay Alignment**: Does the full document still deliver the gameplay vision from Phase 1.5?
6. **Fun Alignment**: Does everything trace back to "The Fun"? Are there systems that don't serve it?

For each issue found:
- State the issue clearly
- Explain why it's a problem
- Implement the fix in the document

Then repeat the critique cycle until no further issues are found.

#### Pivot Protocol
If during self-critique you discover the concept is **fundamentally broken** (not just needs tuning, but structurally unworkable), do not keep patching. Instead:
1. Clearly state why the design isn't working at a foundational level
2. Identify which Phase 1.5 sketch assumptions failed
3. Propose returning to Phase 1.5 with a different sketch or modified approach
4. Get user approval before pivoting

This is an escape hatch, not a routine option. Use only when patching would be putting lipstick on a pig.

### Phase 4: Final Delivery
Present the complete, refined document with a summary of:
- Key design decisions made
- The gameplay sketch that was selected and why
- Issues found and resolved during critique
- Any intentional ambiguities left for playtesting

## Clarifying Questions Policy

**Only ask questions about broad conceptual direction**, such as:
- Target platform/context (mobile, PC, tabletop, etc.)
- Intended session length (quick sessions vs. long campaigns)
- Multiplayer vs. single-player
- General tone (serious, comedic, abstract, narrative-driven)

**Never ask about**:
- Specific numbers, costs, or values
- Exact quantities of content
- Precise balance tuning
- Implementation details

## Output Location

Save the final design document to: `drafts/game-designs/[game-name-slug].md`

## Starting the Process

To begin, the user should provide:
1. A game concept (can be vague or specific)
2. Any constraints (platform, audience, scope, etc.)

Then respond with 10 elaborations and await selection.
