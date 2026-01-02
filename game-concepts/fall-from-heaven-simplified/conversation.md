
what do you suppose you would do or look into if you wanted to build a radically simplified version of Fall From Heaven 2?

Civilization 4 already has enormous issues with conceptually simple decisions ("I want this to be a Production-heavy city for military units) requiring an enormous number of clicks to execute (worker actions, buildings, each unit needs to be both built and commanded), and FFH inherits these.  Also there are standard issues where if you're trying to make a fudnamentally symmetrical 4x game then you're giving way more complciated tools to teh player than the AI will use in an interesting fashion, leading to "cheating" (which is itself boring because the player cannot interact meaningfully with the AI's internal economy.) 

These all seem fixable.

Dec 27, 2025

Synthesized design solutions addressing micromanagement, AI asymmetry, and strategic simplification.
23s












This is a great design problem. Let me think through some directions:

The core tension you've identified:

FFH2's appeal comes from its asymmetric civilizations, the Armageddon Counter, heroes, magic, and the sense that each game tells a story. But it inherits Civ4's fundamental structure where strategic intent → dozens of micro-decisions → results, and where the AI can't leverage complexity so it just gets bonus hammers.

Possible approaches to investigate:

1. Collapse the production/worker layer

Cities could just have sliders or modes ("military focus," "growth focus," "gold focus") rather than individual building queues and worker commands. Your decision is what the city does, not the 40 clicks to make it happen. Endless Legend moved partially in this direction with its district system.

2. Armies instead of units

Build and command armies as single entities with composition choices, not 30 individual swordsmen. Massively cuts clicks, and the AI can actually handle army-level tactics reasonably well.

3. Give the AI genuinely different rules rather than the same rules with bonuses

This is the interesting one. What if the Infernal civilization actually worked differently for the AI—spawning units through a demonic ritual system the player can scout and disrupt? The Illians' expansion could be weather-pattern-based rather than settler-based. You're not trying to make the AI play Civ4 well; you're giving each AI a bespoke behavior model that creates interesting problems.

4. Look at King of Dragon Pass / Six Ages

These games have 4X-adjacent scope but event-driven structure. Strategic decisions are presented to you rather than you micromanaging everything constantly.

What aspects of FFH2 do you most want to preserve?

 3. Give the AI genuinely different rules rather than the same rules with bonuses

Love it, agreed entirely.  The rules the AI gets need to be both implementable as, basically, state machines, and ALSO need to be player-interactable.   AI sucsk in civ games because the designers gave themselves an impossible design task (read: make an AI that is as smart as a human.  Excuse me, if you could do that you would have upended the world economy already.)

2.  Armies instead of units.

Fundamentally civ games tend to simplify combat to such a degree that it's no longer interesting.  "numerical strength in X context" is the only lever they have.  This bloooooows.

I propose using a system more along the lines of Ogre Battle 64.  Arrange soldiers into units and have those units be able to slug it out each turn according to predetermined rules.  Not quite sure about the full mechanics of this, but the bones of it I think make sense and should work.

1.  Collapse production/worker layer.  

Makes sense, but i think the core needs some refinement.  I think the Total War games actually do a really good job with this-- a city has some number of slots based on population; build buildings in those slots.  But you can have up to 3 so by necessity you're getting limited selection.

I think "tiles have production/food/gold values and you assign citizens to them" is a terrible system that's made of slog.  Probably we just say that buildings which gain resources (production, gold, food, whatever) gain resources each turn and scale up in efficacy based on population.

Show more
Dec 27, 2025

Synthesized design proposals and identified implementation questions.
17s







Yes, these fit together well. Let me push on a few seams:

On the OB64-style combat:

The interesting design space here is what decisions you're making versus what's automated. In OB64, you're choosing:

Unit composition (class types, who's in front row vs back)
Tactics setting (aggressive, defensive, etc.)
Leader abilities
Then combat auto-resolves based on those choices. The satisfying part is that you can watch your decisions play out and learn "oh, mages in back rank get murdered by flying units, I need to rethink this."

For a FFH-derived game, this could map nicely to things like: melee front line, ranged back line, mage support, hero abilities. The magic system becomes about equipping your armies with spell loadouts rather than casting 30 fireballs manually.

Question: How does this interact with the strategic map?

Do armies collide and resolve instantly? Do they fight over multiple turns in a contested hex? OB64 had a separate tactical layer with unit movement. Do you want that, or is the strategic map the only space?

On the building/population system:

If population scales building output and you're removing tile assignment, where does population come from? Does it just grow at a rate modified by buildings? Or is there still a food resource that you're generating somehow?

This might be where simplification gets tricky—Civ's food/growth system is tedious but it does create decisions about tall vs. wide.

Honestly, i'm not quite sure about how this should interact with the strategic map.  Crucial to the ogre battle system was that "losing" a battle meant the unit physically retreats (meaning that fights aren't de facto to the death); we could do something similar here ("unit flees to the closest empty square in X direction.")

Good point about population.  Probably population growth is mostly static but particular buildings can amp it up in some static fashion, similar to total war?  I'm not quite sure about this one.  

The question is, does food literally do anything except get your cities to being productive faster?

Dec 27, 2025

Interrogated food's strategic purpose beyond accelerating city productivity.
10s












This is the right question to ask. Let me think through what food/population could do that creates actual decisions:

Option A: Population is your recruitment pool

Soldiers come from your population. A city of 10,000 can field more troops than a city of 2,000, but fielding them costs population (temporarily or permanently). Now there's genuine tension: do you grow tall and then draft a huge army, or do you expand wide with many small cities that can each contribute modest forces?

This also makes raiding/siege warfare meaningful—you're not just reducing a number, you're degrading their military capacity.

Option B: Population gates building slots (Total War style)

Food basically vanishes as a concept. Population grows mostly automatically, and the decision is just "what buildings do I put in my limited slots." Food buildings might exist but only as "this makes the city grow faster so you get your third slot sooner."

This is simpler but arguably food has stopped being a resource and is just a tech tree with a different coat of paint.

Option C: Food is a global/regional logistics resource

Armies need supply. Cities produce surplus food. Armies operating far from your food-producing infrastructure suffer attrition or move slower. Now food is about projection of power rather than city growth.

This could interact nicely with FFH's flavor—the Sheaim don't need food because their armies are demons, the Elves forage, etc.

Which of these feels closer to what you want the game to be about?

We could also just ignore population for the most part.  It's just gating building slots for new cities, making it so that you can't plop down a settlement and have it be a metropolis in three turns. That's it, that's the whole thing.

Dec 27, 2025

Evaluated elegant population-as-timer mechanic for design viability.
15s











Honestly? This might just be correct.

What you're saying is: population is a timer on new cities, not a resource to manage. Found a city, it's small. Wait, it gets bigger. Bigger means more building slots. Done.

The virtue of this is that it completely kills an entire layer of "decisions" that were mostly fake decisions anyway. In Civ, the question "how do I grow this city faster" usually has a solved answer (build granary, work food tiles), so you're not making interesting choices—you're just executing the known-correct sequence.

What this does to the rest of the design:

City differentiation now comes entirely from building slot choices and geography. If a city is on a river with nearby iron, it's a good military production city because you put military buildings in it, not because you fiddled with citizen assignments for 50 turns.

This probably means you want geography to be more meaningful than in base Civ. Resources, terrain, defensibility—these become the primary reasons one city differs from another.

One remaining question:

Does conquering/razing cities matter? If population is just a timer, capturing an enemy city gives you a "more mature" city than you could have built yourself in that time. Is that enough incentive to wage war for cities rather than just razing them? Or do you want some other mechanic there?

Eh, razing a city just means you have to regrow it from scratch.  not much of an advantage there.

Dec 27, 2025

Weighed razing's limited utility against capturing, explored asymmetric faction design.








Right, so capturing is just strictly better than razing unless the city is in a genuinely bad location. That's... probably fine? It simplifies the decision tree: take cities, they're valuable, done.

Though this is a place where asymmetric faction design could do work. In FFH2, the Infernals/Sheaim wanted destruction because it advanced the Armageddon Counter. You could have:

Factions that benefit from capture (most of them—empires want cities)
Factions that benefit from razing (gain resources, summon units, advance some apocalyptic track, or simply don't use cities the same way)
**Factions that benefit from neither—**maybe the Elves equivalent just wants everyone to leave them alone and their victory condition is about controlling specific sacred groves, not empire-building at all
This gets back to your earlier point about giving the AI different rules. An AI faction that wants to raze your cities is playing a fundamentally different game than one trying to out-economy you. And "stop the faction that's burning everything" is a much more tractable AI problem than "execute a 200-turn economic optimization strategy."

Question that might be worth pinning down soon: What does the Armageddon Counter equivalent look like in this system, if anything? That was FFH2's most distinctive strategic layer—the shared global state that all players were affecting and responding to.

Oh, i rather think the armageddon counter should be explicitly about "this is when the endgame faction comes and tries to stomp you.  If you can take its capital or something then you win.  That's the game."

Basically i'm thinking the endgame is straight up based on the armageddon counter.

Dec 27, 2025

Validated elegant endgame mechanic solving multiple 4X design problems.




Oh, this is sharp. You've just solved like three 4X problems at once:

The late-game slog: No more "I've clearly won but need 2 hours to mop up." The game has an actual climax.

Victory condition convergence: Everyone's playing toward the same endgame, just with different strategies for getting there and being positioned well when it triggers. No more "oops, they won a culture victory while I was building tanks."

AI purpose: The AI factions don't need to "win" in the optimize-everything sense. They need to survive, jockey for position, and either hasten or delay the counter depending on their alignment. Much more tractable behaviors.

Questions this opens:

What advances the counter? Some options:

Certain factions advance it through their natural play (the Sheaim equivalent just ticks it up by existing aggressively)
War and destruction generally
Time, inevitably, so turtling doesn't work forever
Deliberate player/AI actions (dark rituals, etc.)
And critically: can you be aligned with the endgame faction? The Infernals in FFH2 were kind of playing for Team Apocalypse. If one player/AI is helping the final boss, that creates a very different diplomatic landscape than "everyone versus the monster."

Really my idea is that the AI factions aren't trying to "win", as such, they're just doing whatever it is they're programmed to do.  The Sheiam run rituals that beef up the endgame boss, the hippus rove around and raze things wtih their horses.  Standard stuff.)

Dec 27, 2025

Recognized elegant design paralleling asymmetric games and environmental systems.
10s









This is the right frame. The AI factions aren't opponents in the traditional sense—they're dynamic terrain. Each one is a problem with a particular shape that you need to manage, exploit, or eliminate.

What this means for faction design:

Each AI faction needs:

A behavior loop (what it does left alone)
A threat profile (how it makes your life harder)
A player interaction point (what you can DO about it)
So the Sheaim: behavior is "build up and run rituals," threat is "accelerates endgame before you're ready," interaction point is... what? Raid their ritual sites? Assassinate their priests? Diplomacy to slow them down?

The Hippus: behavior is "roam and raid," threat is "economic damage and harassment," interaction point might be "bribe them to raid someone else" or "destroy their horse supply" or just "wall up and be a less appealing target than your neighbor."

The interesting design question:

How much can the player push back on these behavior loops versus just playing around them? If you can fully shut down the Sheaim, they become a puzzle to solve once. If you can only slow them, they're persistent pressure. The latter seems more interesting but requires tuning so it doesn't feel futile.

Does the player faction have a "behavior" too, or is the player uniquely flexible?

Uniquely flexible. The player gets tools they can use with cleverness. The ai is a state machine. (it was always that, but saying so explicitly lets us actually design them.)

Really I think the enemies are by far the simpler to design; the design difficulty is in the player economy

Dec 27, 2025

Interrogated player economy design tensions and resource allocation mechanics.
21s












Yes. The AI factions are basically content authoring—you're writing interesting enemies with legible behaviors. That's craft, but it's known craft.

The player economy is where the actual design problem lives. Let me try to frame what it needs to do:

It needs to produce decisions that are:

Few (not 30 clicks per turn)
Genuine (not solved optimization problems)
Contextual (the right answer depends on game state, not just "build granary first always")
What you've already eliminated:

Tile/citizen management
Per-unit commands
Population as a managed resource
What remains unclear:

What does the player actually do each turn? Some options:

Allocation model: You have income (gold? mana? generic "power"?), you spend it on things. The decision is prioritization. Simple but maybe too thin.
Attention model: You can only take N actions per turn. Moving an army is an action, building something is an action, casting a ritual is an action. The decision is where to focus. This naturally limits micro.
Stance/posture model: Cities and armies have modes. You set them, they execute. You're not micromanaging, you're directing. Most turns are "continue current plan" with occasional pivots.
What does your instinct say about where decisions should live for the player?

Per unit commands are fine, but we have to have VERY LIMITED units to command. Maybe five at game end.

Eaxh commandable unit is a legion composed of maybe 5-8 soldiers, ogre battle style. Combat auto resolves.

Dec 27, 2025

Examined strategic implications of severe unit scarcity constraints.






This is a strong constraint that clarifies a lot. Five legions means:

Each legion is a strategic asset, not a disposable unit. You care about its survival. Losing one is a major setback, not "queue another swordsman."

Composition is where the decisions live. The OB64 loop: you're fiddling with who's in each legion, what their formation is, maybe what spells/abilities they have equipped. This is the "interesting complexity" that replaces Civ's tedious complexity.

Positioning becomes a genuine puzzle. With 5 legions and presumably more than 5 places you'd like a legion to be, you're constantly making tradeoffs. Defend this city or raid that ritual site? Can't do both.

The AI problem simplifies dramatically. An AI faction might have 2-4 legions with preset compositions that fit their faction flavor. Hippus have fast cavalry legions. Sheaim have demon-heavy legions with magical support. You can hand-author these.

Questions this raises:

Where do legions come from? Options:

You start with 1, gain new legion slots at certain thresholds (city count? tech? armageddon counter stage?)
Legions are permanent but soldiers within them are recruited/replaced from cities
And: can legions be destroyed, or just beaten and retreated? If they can be fully wiped, that's devastating. If they always retreat when losing, you need some other attrition mechanism.

When I say legion I mean a grouping of soldiers. Each individual soldier can die. Each legion can be forced to retreat by beating it in combat (beating in combat just means deals more damage than it receives)

Dec 27, 2025

Synthesized combat mechanics into cohesive economic and strategic gameplay loop.
10s









Got it. So the attrition loop is:

Soldiers die in combat
Legions retreat when they lose a round
Legions get worn down over multiple engagements
Legions need replenishment
This might be where the economy actually lives.

Your cities aren't producing "units" in the Civ sense. They're producing soldier capacity or recruitment access. A military-focused city with the right buildings lets you recruit heavy infantry. A magical city lets you recruit mages. Etc.

So the player economy loop becomes:

Cities unlock soldier types (via building slot choices)
Resources let you recruit (gold? maybe just time/population?)
You slot soldiers into legions (the OB64 composition game)
Combat wears legions down
Back to step 2
The decisions are: what buildings (soldier types available), what composition (how do I configure my legions), what positioning (where are my five legions right now).

This also gives cities distinct identity. Your capital might be your heavy infantry source. The frontier city produces scouts/cavalry. The mountain city produces mages. Losing a city isn't just "one fewer city," it's "I can't recruit battle mages anymore."

Does that track with what you're imagining? And: is replenishment instant (spend gold, add soldiers) or does it take time/require the legion to be somewhere specific?

Probably anywhere in your borders is fine for replacing soldiers.  For healing soldiers, gotta be garrsioned.

Probably this should work acceptably. Any clear unclear bits? Was considering feeding a general outline to Claude code to whip up a prototype.

Dec 27, 2025

Identified twelve design gaps blocking prototype development.
20s





Biggest gaps I see for a prototype:

Combat resolution: What actually happens when two legions fight? You need at least a sketch of: front row hits front row, ranged/mages hit from back, some kind of damage calculation, some threshold for "you lost this round, retreat." Doesn't need to be balanced, but needs to exist.

Soldier type palette: How many types, roughly? Classic split might be: melee (tough, front row), ranged (back row, fragile), mage (back row, AoE or special effects), and maybe cavalry (fast, flanking?). Plus heroes as elite soldiers or legion commanders?

Resource model: Is there just gold? Gold plus mana? What do non-military buildings actually produce, and what do you spend it on? Recruitment costs? Building costs? Both?

Turn structure: Player moves all legions, then each AI faction takes its turn? Or something else?

Map basics: Hex grid? How far does a legion move per turn? Do different soldier compositions affect movement speed?

Endgame boss behavior: When the counter fills, what spawns and what does it do? Does it just send legions at everyone? Does it have a capital you need to reach?

The rest can probably be figured out during prototyping. These are the ones where Claude Code will stall waiting for answers.

Ah, right. We steal combat resolution and mechanics from ogre battle directly. Wholesale theft. Steal soldier types similarly (without gender coding, though, that's not really a cultural thing today). Resource model is that we start with just gold and per province population.

For other questions, we would do the simplest thing that could possibly work. Then we would iterate.

Dec 27, 2025

Synthesized design framework for iterative game prototyping approach.



That's a viable starting point. Here's a summary you could hand to Claude Code:

Core loop:

Player controls up to 5 legions (OB64-style unit groups, 5-8 soldiers each)
Cities have building slots gated by population; population grows automatically over time
Buildings unlock soldier types and/or produce gold
Soldiers recruited with gold, slotted into legions
Combat auto-resolves per OB64 rules (front row/back row, class abilities, loser retreats)
Soldiers die, legions retreat; healing requires garrison, replenishment requires being in borders
AI factions:

State machines with defined behaviors, not trying to "win"
Each has a behavior loop, threat profile, and player interaction point
Example: Sheaim runs rituals advancing armageddon counter; Hippus roams and raids
Armageddon counter:

Advances via faction actions, possibly time
When full, endgame boss faction activates
Player wins by taking its capital (or similar objective)
Steal from OB64:

Combat resolution
Soldier types (fighter, knight, archer, mage, cleric, etc.)
Formation/tactics system
Simplest-thing-that-works for:

Hex movement, one legion move per turn
Turn order (player then AI factions)
Map generation
Endgame boss just sends legions toward players
Anything you'd amend before handing it off?

what do you recall of the combat system's mechanics in ob64

Dec 27, 2025

Reconstructed fragmented memories of tactical combat mechanics and acknowledged uncertainty.






Honestly, my memory is fuzzy on specifics. Here's what I think I recall:

Formation:

3x3 grid, characters placed front/middle/back row
Position determined what attacks they could use (melee needs front, ranged/magic from back)
Attack flow:

Characters acted based on class and position
Each character got a certain number of attacks per combat round
Faster/higher level characters might get more attacks?
Tactics setting:

You set a general stance per unit (attack leader, attack weakest, attack strongest, etc.)
This governed targeting priority
Resolution:

Combat ran for a fixed number of exchanges, then paused
Whoever dealt more damage "won" the round
Loser retreated in the direction they came from
Units could re-engage by moving back into contact
Class specifics:

Fighters/knights: melee, front row
Archers: back row physical damage
Mages: back row, elemental AoE
Clerics: healing
Flying units had special properties
But I'm uncertain on: exact damage formulas, how many attacks each class got, how the exchange count worked, what all the tactics options did precisely.

Worth pulling up a FAQ or mechanics guide before handing this to Claude Code, or just telling it to implement "OB64-like combat" and see what it hallucinates, then iterate?

ah, okay.  the way it worked was characters each had, depending on whether they were in front/mid/back lines, different attacks that woudl themselves either hit the BACK unit in their column (or the closest column) preferentially or the FRONT.   ranged units tended to hit the back, melee the front. 

Depending on position these attacks could hit 1, 2 or 3 times.  The exact order in which attacks would occur were based ont he unit's speed stat.

Dec 28, 2025

Deciphered combat mechanics and assessed implementation feasibility.



Ah, that's much cleaner than I was imagining. So the system is:

Per soldier:

Has attacks that vary by their row position
Each attack specifies: damage, target preference (front vs back of enemy column), and hit count (1/2/3) based on position
Speed stat determines action order
Combat round:

Sort all soldiers by speed
Each soldier executes their attack(s) in order
Attacks target enemy column, preferring front or back depending on attack type
Tally damage dealt to each side
Loser retreats
The design space for soldier types becomes:

A knight in front row might get 3 melee attacks hitting enemy front. Same knight in back row gets 1 weak attack or nothing.

A mage in back row gets 2 attacks hitting enemy back (killing their mages/archers). Same mage in front row gets murdered before acting because low speed and no real attack.

An archer in back row hits enemy back 2x. In mid row maybe hits 1x. In front row, useless.

This makes legion composition genuinely tactical: you're trying to build a formation that kills their back row while protecting yours, or kills their front row fast and then mops up, etc.

Is there anything else to the system, or is that the core of it?

ah, so a bit different: I think the actual algorithm was something like after an attack would hit, the unit in question would wait for a time (during which other units would be attakcing.)  The net effect was that units would take turns, but never take more than their allocated number of attacks in the combat.

Dec 28, 2025

Reconciled time-based combat mechanics with attack frequency caps.



Ah, so it's more like an ATB (active time battle) system:

Combat runs as a timeline
Each unit has a "next attack at" timestamp based on speed
When their time comes up, they attack, then their next timestamp is set to current + delay (based on speed)
This continues until combat round ends (fixed duration? fixed number of total attacks?)
Unit's row position caps their total attacks for the combat (1/2/3 max)
So a fast unit in front row might hit 3 times quickly and then sit idle. A slow unit in front row might only get 2 of their 3 potential attacks off before the round ends.

This means speed does two things:

Determines whether you get your attacks off first (killing enemies before they act)
Determines whether slow units even reach their position-based cap
That's more interesting than pure turn order—a glass cannon mage with high speed might nuke the enemy back line before the enemy fighters ever swing.

Is the round a fixed time duration, or does it end after some number of total exchanges, or when one side is dead/fleeing?

hold up, EVERY unit gets their attacks off.  it's just that the slowest units might be taking 2 or even all 3 of their attacks at the end of the round, after everyone has acted.

Dec 28, 2025

Reconciled attack mechanics with speed-based sequencing dynamics.




Oh, that's cleaner. So:

Every unit WILL get their 1/2/3 attacks (based on row)
Speed determines when in the timeline those attacks fire
Fast unit: attacks spread out through the round
Slow unit: attacks clumped at the end
Why speed matters:

Dead units don't attack. So a fast mage who kills an enemy fighter in the first moments of combat means that fighter never gets their 3 attacks off. But if the fighter survives the round, they're getting all 3 swings eventually.

This makes speed a "priority damage" stat. It's not about getting more attacks, it's about getting kills before retaliation.

So a speed-focused glass cannon strategy tries to wipe key targets before they act. A tanky slow strategy accepts you'll take hits but you'll still get your damage in, and you'll survive to the next round.

For the prototype: this could be simulated pretty simply. Run through the timeline, resolve attacks in speed order, remove dead units (who lose remaining attacks), tally final damage/survivors, determine retreat.

That seems implementable. Anything else core to the system, or is that the skeleton?

*mostly* speed wasn't a hugely important stat; an exception is with units that specifically debilitate units as part of their kit (Witches did this)

Dec 28, 2025

Synthesized speed's limited impact except for debuff-focused units.





Right, that makes sense. In practice:

Damage alone usually didn't kill fast enough for speed to matter. You'd both deal damage, both survive the round, retreat happens based on total damage dealt. The attacks all happen, just in different order.

Debuffs break this because a paralyzed/petrified/slept unit loses their remaining attacks. So a fast Witch who disables an enemy fighter at the start of the round does deny those 3 attacks. That's a big swing.

Design implication for soldier types:

Most units: speed is a dump stat, focus on damage/survivability. Order doesn't matter if everyone's just trading blows.

Debuff units: speed is critical, they need to land disables before enemy damage dealers act.

For the prototype this suggests:

Start with straightforward damage-dealing types (fighter, archer, mage)
Speed can be mostly uniform initially
Debuff units (witch/status mage) are a later layer of complexity once base combat works
When you add them, speed suddenly matters and creates a new tactical dimension
Does that capture it? And is there anything about the retreat/damage comparison logic—was it literally "side that dealt more total damage wins" or was there nuance there?



Please 


Claude is AI and can make mistakes. Please double-check responses.
Claude