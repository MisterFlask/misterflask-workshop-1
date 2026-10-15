# AI design (ffh-ai)

The C++ AI (CvPlayerAI/CvCityAI/CvUnitAI/CvTeamAI, ~60k lines, plus the Python AI hooks
in CvGameUtils.py such as `AI_chooseTech`/`AI_chooseProduction`) is not ported. It is the
main cause of FFH2's late-game turn times (per-unit re-evaluation of every plot, repeated A*
searches, Python callbacks). `ffh-ai` is a new design with three hard requirements:

- **Legal play only.** It acts through the same rules APIs a human would: missions pushed on
  selection groups (`group_push_mission`), city orders (`city_push_order`), research
  (`player_push_research`), civics/religion/sliders, deals (`implement_deal`), spells
  (`unit_cast`), upgrades/promotions, scrapping. The rules decide what is allowed; a rules
  bug is reported, never worked around in the AI.
- **Bounded cost per turn.** No per-unit full-map scans and no path search per candidate
  target: at most a few bounded floods per group, one real path for the chosen target.
- **Deterministic.** Decisions are pure functions of the game state. The AI never draws from
  the game RNG (its "random" war rolls hash the turn and player), never iterates a
  HashMap/HashSet, never reads the clock. Its only memory outside `Game` is a shortcut a fresh
  controller rebuilds identically: the site cache lives for one `update()` call (saves happen
  between updates), and "already played this turn" is also derived from the game state (the
  rules call again within a turn only while some unit is moving, i.e. has spent moves). So a
  game saved and resumed with a new controller plays on exactly (`tests/save_resume.rs`).

## Where the rules call the AI

`Ai` implements `ffh_core::Controller`; the rules call it where the C++ calls the AI:

| C++ | Controller | Module |
|---|---|---|
| `CvPlayerAI::AI_unitUpdate` (from `CvGame::updateMoves`) | `play_turn` | `lib.rs` |
| `CvCityAI::AI_chooseProduction` (from `CvCity::doProduction`) | `choose_production` | `production.rs` |
| `CvPlayerAI::AI_chooseResearch` / `AI_chooseFreeTech` | `choose_research` | `research.rs` |
| `CvPlayerAI::AI_conquerCity` | `raze_conquered_city` | `diplomacy.rs` |
| `CvPlayerAI::AI_considerOffer` | `consider_offer` | `diplomacy.rs` |
| `CvPlayerAI::AI_doAdvancedStart` (from `CvPlayer::setTurnActive`) | `play_turn` while start points remain | `advanced_start.rs` |
| `CvPlayerAI::AI_chooseEvent`, elections, votes, captured/gifted units | defaults in `controller.rs` | -- |
| `CvCityAI::AI_assignWorkingPlots` (citizens) | `Game::city_assign_working_plots` | `ffh-core/src/governor.rs` |

`CvPlayerAI::AI_foundValue` (and its bonus valuation) is ported faithfully into the rules
(`ffh-core/src/found_value.rs`) because starting plots and barbarian cities depend on it; the AI
reuses it to score city sites. The rules also keep the C++ team AI counters (war/peace
counters, war success, war plans) that the diplomacy below reads.

## A turn (`play_turn`)

0. **Advanced start** (`advanced_start.rs`, `GAMEOPTION_ADVANCED_START`): while the player has
   start points, C++ `AI_doAdvancedStart`'s plan with fixed choices instead of its random
   draws: the capital on the starting plot, ~10% of the points on revealing the land around
   it, the leader's unit share (10% + `iBuildUnitProb`/3) on defenders, workers and field
   units, the rest on cities (best found value within 8 plots, 500 + 250 x cities minimum)
   with culture, improvements for the citizens they can keep happy and population to work
   them, technologies (research's pick; a quarter of the city points while placing cities),
   an explorer and buildings by value per point; then `ADVANCEDSTARTACTION_EXIT`.
1. **Empire** (`empire.rs`): research when the queue is empty (a broke empire first
   researches its way out, see Finance); civics every 10 turns when the gain beats the
   anarchy (sooner under a structural deficit); state religion; the highest research slider
   that keeps the treasury from running dry; unit support that holds research below its
   target is cut first (see Finance); a bankrupt empire (slider at the bottom, still losing
   gold) scraps surplus garrison units, never below what production wants in that city; an
   empire paying unit support with research at a fifth or less disbands its weakest field
   units (never heroes) down to a mobile force of 1 + cities/4 while at peace.
2. **Emergency production** (`production::emergency`): a city whose visible threat exceeds
   its garrison switches to a defender (the rules keep the hammers); a capital or a city near
   barbarians that has only one defender after turn 30 puts a second one first.
3. **Diplomacy** (`diplomacy.rs`), see below.
4. **View** (`view.rs`): enemies, a *danger map* (each visible enemy unit's attack power
   added to the plots it can reach next turn; animals only on unowned plots since they cannot
   enter owned land; city garrisons ignored), field-army targets, cities in peril, the road
   plan, escort requests. O(enemy units x reach^2 + our cities).
5. **Units** (`units.rs`): promotions and upgrades first (also for fortified garrisons that
   get no orders), then every group that needs orders gets one decision by its head unit's
   role (`Unit::ai_type`, set when the city trains it):
   - **settlers** (`expansion.rs`, see "Settlers and escorts" below) go with an escort to the
     best reachable site whose danger the escort covers; the first city is founded in place
     unless a clearly better site is reachable this turn; with no site left on its landmass a
     settler goes overseas (see Naval);
   - **workers** (`workers.rs`), see below;
   - **garrisons** fill each city's quota;
   - **field armies** (`military.rs`), see below;
   - **explorers** reveal the nearest unknown area and goody huts, then join the army;
   - **great people** discover, join as specialists or start a golden age;
   - **missionaries** go to our cities without the state religion; **casters** move with the
     army or to the most threatened city; barbarians and animals attack what they can beat,
     else roam; **ships** see Naval. Units aboard a ship get no orders (the ship lands them).
     Any group still idle is skipped so the turn always ends.
6. **End-of-turn spells** (`casting.rs`): FfH's own AI casts spells at the end of every turn
   (`CvGame::updateMoves`, "BETTER AI (Spellcasting end of turn)"): `CvUnit::chooseSpell`, then
   buffs and summons, ported term by term with this AI's values.

## Cities

**Production** (`production.rs`) is a priority list, evaluated when a city's queue empties
(a process at the head is re-decided every turn, so a city is never idle):

1. no defender, or a garrison overwhelmed by the visible threat -> best defender;
2. the first worker once there is something to improve; a transport for a settler waiting in
   port (also forced to the head of that city's queue by `emergency`), a sea explorer while
   our land is full and nothing is known overseas, work boats for unimproved sea resources;
3. a second defender after turn 30 in the capital and in cities near barbarians;
4. an explorer from the capital while a fifth of the home landmass is unknown;
5. settlers while sites remain on the city's landmass and nothing threatens it: at most
   (cities+1)/2 at a time (half at war), none while bankrupt, from cities of size 3+ (4+ with
   a food surplus of 3; size 1 if the city is at its happy cap: a settler eats the food
   surplus as production, so a size-2 city needs 30+ turns for one), not from a city more
   than twice as slow as our fastest one on that landmass; with no site left at home, one
   overseas settler at a time from a coastal city of size 3+ that has or can build a
   transport; while growing toward a settler the city builds only short items (warriors
   that explore and escort); a settler 3 turns from done (or waiting in the city) with no
   fighter to spare gets the cheapest adequate escort queued behind it (`expansion.rs`);
   then civilization-specific items (Kuriotates settlement settlers, Lanun cove boats; see
   Civilization-specific below);
6. FfH tier-2 units need a building in the training city (Archery Range for archers,
   Training Yard for axemen, ...): when one would open a defender or attacker a third
   stronger, about half the cities build it;
7. extra defenders up to the quota (capital, threat, nearby barbarians, war, leader);
8. a small mobile force (1 + cities/2) for barbarians and animals; more workers as worked
   plots need improving; heroes (not at the cost of the first expansion);
9. the siege operation's force (`campaign::army_choice`, from `needed_force`: about two
   attackers per defender of its target, siege engines -- or the Siege Workshop that trains
   them -- for walls or 3+ defenders; city attackers only if they have at least half the
   strength of the target's best defender; every other slot in peacetime); then the army
   target (larger for military-minded leaders, doubled at war or while preparing one; at war
   the attackers are chosen for strength against cities -- city attack bonus, first strikes,
   collateral -- not mobility), a third of it siege engines at war, and a couple of siege
   engines whenever a stack is camped at a city it cannot take; in peacetime army and
   buildings alternate;
10. buildings by marginal value per production turn (yields, commerce, culture for a new
    city, happiness and health when capped, maintenance savings, specialists, experience,
    ...); casters; missionaries; projects that do not need dedicated logic; a process.

Nothing left to build: workers, then field units (never garrisons a broke treasury would
scrap); when no more units are affordable, the strongest one whatever it costs (support is
the same for any unit and the weakest are disbanded first, so idle hammers buy quality).
Units are chosen by power squared per production turn, so stronger units win unless much
dearer.

**Unit support** (`economy.rs`): FfH units beyond the free ones (handicap, civics,
population) cost gold every turn. Every optional unit (extra defenders beyond one per city
and two in the capital, the mobile force beyond its first unit, the army target, siege
engines for a camped stack, cheap units while growing, casters) is built only while one more
unit is affordable with research at 60% (30% at war or while preparing one): the estimate
takes the marginal unit cost (handicap/AI/era percentages and inflation as they apply now)
against the gold the empire would make at that slider. Without it, AIs that had run out of
buildings trained units until research was at zero and nothing but units was left to
build.

**Citizens** (`governor.rs`, in ffh-core; replaces `AI_assignWorkingPlots` and is used by the
rules wherever the C++ asks the city AI): a greedy assignment plus a swap pass. Food is valued
by what it does: a deficit is expensive; growth is worth more the more happy and healthy room
the city has, the first two points of surplus count double (a city at +0 never grows), more
than the growth target counts a third; at the happy cap a surplus is slightly unwelcome;
food building a settler counts as production. Hammers below a minimum count double;
commerce follows the sliders and is worth up to twice as much when the research slider has
been pushed down to pay the bills, or when the costs measured from the rules would leave
less than 60% for research (3 points of weight per point of shortfall in percent of the
commerce; merchants and scientists gain with it over poor tiles); a culture-starved new city
values culture.

## Finance (`finance.rs`)

The zero-research death spiral: an empire whose costs (city maintenance, civic upkeep, unit
support, inflated) eat its commerce pushes the slider down to pay them, and at zero research
never reaches the techs that cut those costs (City States: Cartography; Aristocracy and
Courthouses: Code of Laws), so it stays there for the rest of the game. `finance::state`
reads the empire's finances from the rules' numbers (no cache): the gold rate at the
research share we want to keep (60% at peace, 40% at war), the cost breakdown, whether the
deficit is **structural** (maintenance and civic upkeep alone eat what that share leaves for
gold) or temporary (unit support), and the turns of reserve left. The responses, each a
single hook tagged `bankruptcy-guard`:

- **Escape research** (`empire::research`): while research is down to 30% or the deficit is
  structural, the tech (prerequisites included) with the most gold saved per beaker: one that
  unlocks a civic we would adopt that costs 3+ gold a turn less than ours, or a building that
  cuts maintenance by as much (Courthouse, counted at half). The current research keeps its
  beakers. This is the decisive response: without it the others leave twice as many civs at
  20% research or less.
- **Civics** (`empire::civic_value`, `empire::civics`): upkeep and maintenance are valued at
  the broke rate (40 points per gold) as soon as the deficit is structural, maintenance from
  the cities' actual distance and number-of-cities parts; Aristocracy is charged for the
  cottages FfH's AI-only `canBuild` bans under it once Education is near; a structural
  deficit re-checks civics outside the 10-turn cadence when a cheaper civic would win.
- **Unit support** (`empire::disband`): while support takes a tenth of the commerce and keeps
  research below its target (at peace), up to two units a turn go, obsolete ones first
  (weakest relative to what we can train now): field units beyond 1 + cities/4, garrison
  units beyond one per city (two in the capital and near barbarians) where no threat is in
  sight, idle workers beyond twice the need. Disbanding more (from 5% of the commerce) lost
  more cities to barbarians.
- **Citizens**: the governor's commerce weight above.
- `finance::city_affordable` (a new city's maintenance and upkeep against the gold left at
  the target share) is for the settler logic and not wired in. Wired into `settler_choice`
  it removed nearly all the remaining traps (on seeds 1-12 one civ at 20% research or less at
  turn 200, none at turn 300) at the cost of 7% of the cities and no better score.

Measured (6 civs, standard map, 300 turns, seeds 1-12; seeds 13-24 in brackets): civs at 20%
research or less at turn 200 24 -> 4 [24 -> 8], at turn 300 17 -> 4 [16 -> 8], at 0% at turn
300 10 -> 3 [11 -> 4]; mean research slider at turn 300 53% -> 64% [50% -> 59%]; mean techs
19.8 -> 22.5 [20.2 -> 22.7], score 803 -> 887 [846 -> 920]; unit support 11.8% -> 9.5% of the
commerce [11.7% -> 8.5%]; no measurable change in turn time.

Tried and dropped (no measurable gain on seeds 1-12): scaling the value of
maintenance and commerce buildings (Courthouse, Market, Library, Harbor) by the share of
commerce lost to costs in `production::decide`; a fallback for cities with nothing left to
build that prefers a worker with work waiting or a garrison upgrade over the strongest unit
(the fallback is only reached by cities that know no process: `decide` picks a process
whenever one is available); capping optional units at 12% of the commerce.

## Research (`research.rs`)

Score = value / (turns to research + 4), with one level of lookahead (children at a quarter of
their static value). Value terms: the leader's favorite early techs and religion, flavors,
AI weights; buildings the civ can use (a static value, plus production's marginal value of
the building in the six cities that gain most: libraries where there is research to multiply,
markets where there is gold); land improvements counted on our cities' plots (growing
improvements such as cottages at two thirds of their grown yield, commerce worth more when
broke); a better defender or attacker weighted by the military pressure we see (barbarians
near our cities, war, time), with an early "get past warriors" bonus, also through the next
tech (Mining -> Bronze Working); the first arcane unit; happiness buildings when cities are
capped; better civics than the ones we run (City States and Aristocracy cut maintenance);
technology and open-borders trading with the civs we know; the first transport when our
landmass is full or a settler waits in port (and the tech before it); processes (Writing,
Mathematics) when cities have nothing left to build. `FFH_AI_TRACE_TECH=1` with
`FFH_AI_TRACE` lists every candidate's value and score.

## Settlers and escorts (`expansion.rs`)

`settler_choice` (when a city builds settlers, item 5 above) and the settler role live here.
A settler never leaves a city alone: a city founded without a defender trains one at one or
two hammers a turn, and the barbarians took such cities within a few turns of founding.

- **Escort**: the settler takes a fighter from its plot (field units first, else a garrison
  unit the city can spare: two defenders stay in the capital, one elsewhere, and the rest
  must still hold the visible threat), a second one when the best site needs more. Once per
  turn (`prepare_escorts`), a city 3 turns or less from a settler, or holding one that waits,
  with no fighter to spare queues the cheapest unit that defends at least 70% as well as its
  best one, so both leave together; the escort becomes the new city's first garrison. Waiting
  settlers also ask single field units within 6 turns to come (`View::escort_requests`).
- **Risk**: each reachable site's risk is the worst danger on the straight line there, or
  twice the danger at the site (the new city has to hold it). A settler goes only where its
  escort's defense covers the risk; with none it waits in the city (up to 10 turns, then it
  goes only where nothing can reach it; an escorted one then accepts twice its escort). A
  settler in the field whose sites all turned dangerous walks back to the nearest city.
- **Target**: sites are scored value / (turns + 3) and re-planned every turn; the site the
  settler was walking to (kept as the group's `MISSIONAI_FOUND` plot, saved with the game)
  scores 1.5x, so settlers no longer flip between sites of similar value.

Measured over seeds 1-48 (6 civs, standard, 300 turns) against the version without them:
settlers destroyed 50 -> 28, cities razed 38 -> 32, cities captured by barbarians 77 -> 71,
techs at turn 300 19.6 -> 19.8; each part matters (without the forced wait captures were
104, without escort production 112). Cities per civ are unchanged (2.1 at turn 100).

Tried and dropped (same seeds): two settlers at a time for one or two cities, sites within 5
plots of our cities, the capital giving up its second defender as an escort, settlers at size
2 or before the first worker, deferring buildings while a city grows toward a settler, and
chopping forests while a settler is built (C++ `bChop`). The fastest mix had 2.4-2.5 cities
per civ at turn 100 (4th city at turn 131 instead of 150) but 0.8-1.4 fewer techs and fewer
points at turn 300, the same number of cities by then, and more cities lost to barbarians,
also on top of the finance and research changes. The rules couple barbarian cities to ours:
`createBarbarianCities` starts once civs average two cities and its target grows with the
number of civ cities, so faster expansion brings more barbarian cities sooner.

## Workers (`workers.rs`)

A worker floods 8 turns of reach and scores every (plot, build) in our cities' radius:
yield change weighted by the working city's needs (food while it can grow, production,
commerce; cottages at two thirds of their grown yield), plus the value of connecting a
resource; worked plots count fully, others at 60%; existing improvements are replaced only
for a clear gain and never one that connects a resource. Roads come from a per-turn plan:
the next missing plot between two of our cities on the same landmass, and links from improved
resources. Score / (travel + build time). Workers skip plots under danger and flee (below).
FfH's AI-only build bans (`CvGameUtils.canBuild`) are respected: no forts, no farms for Fallow
civs, no workshops before Guilds, no cottages under Aristocracy, no windmills under Arete.

**Workers and barbarians** (`security.rs`, the worker part of round 3's barbarian-security
branch). The barbarians captured about 125 workers per game (6 civs, 300 turns), most of them
taken back by the civs a few turns later, and kept the rest as units. Two causes:
- *Fleeing failed.* A worker an enemy can reach next turn goes to the nearest city. For an AI
  group that cannot fight, the rules' path finder refuses any destination an enemy can reach
  (C++ `pathDestValid` with `AI_getPlotDanger`), and the city next to the raider is such a
  destination, so the worker stayed where it was taken. The flee now moves with
  `MOVE_IGNORE_DANGER`. A worker on a plot our fighters hold (at least as strong as what can
  reach it) stays.
- *Trips past the raider.* A worker in its city chose a safe job a few plots away and stopped
  for the night next to the raider. Once per view, `security.rs` builds a secure-zone map: the
  attack power of visible hostile units that can reach each plot within two turns (2 x moves + 1
  plots, animals only outside borders, a nominal threat near revealed lairs and enemy cities),
  and the plots within one step of our fighters (garrisoned cities included); a plot is secure
  when nothing reaches it within two turns or it is that close to a refuge. While an enemy can
  reach a plot the worker could step on this turn, a job it does not reach this turn is taken
  only if its overnight stop (the plot reached this turn nearest the job) is out of the enemy's
  reach and secure.

`ffh stats`, 6 civs, standard map, 300 turns, per game or per civ at turn 300; t is the paired
t-statistic over seeds; cities lost = civ cities the barbarians took or razed
(`captured_or_razed_by_barb`, equal to the game log's count):

| | seeds 1-24: before | + flee | + overnight rule | fresh seeds 25-48: before -> after | all 48: t |
|---|---|---|---|---|---|
| workers captured by barbarians | 114.6 | 12.3 | 5.7 | 134.3 -> 5.4 | -15.8 |
| civ cities lost to barbarians | 3.88 | 3.29 | 1.96 | 2.54 -> 2.54 | -1.5 |
| barbarian units / cities | 106 / 7.1 | 86 / 6.3 | 70 / 5.3 | 112 / 7.3 -> 79 / 6.2 | -4.3 / -3.0 |
| improvements (turn 200 / 300) | 22.3 / 49.0 | 23.0 / 47.6 | 24.5 / 49.8 | 23.4 / 46.4 -> 23.8 / 49.2 | 1.7 / 0.9 |
| legacy score (alive civs) | 899 | 908 | 917 | 865 -> 897 | 1.2 |
| techs / population | 22.1 / 42.4 | 22.5 / 41.7 | 22.7 / 43.0 | 21.6 / 39.7 -> 22.3 / 42.0 | 1.5 / 1.1 |
| research / commerce | 92 / 126 | 86 / 121 | 93 / 123 | 81 / 111 -> 90 / 122 | 1.0 / 0.7 |

CPU per turn (seeds 1-6, process time): 44.2 -> 42.0 ms; the map costs visible enemy units x
11^2 plots + our fighters x 9 + one pass over the plots for lairs, and fewer captured workers
mean fewer barbarian units to move. What is left (about 6 a game): workers sheltering in a city
that falls, settlers (captured as workers), and workers that leave a city on a several-turn trip
just after the raider has gone back into the fog.

Tried and not kept: applying the overnight rule whenever an enemy can reach a plot the worker
could step on within two turns, not one (seeds 1-24: captures 5.7 -> 5.9, techs at turn 300
-0.8, t -2.2). Round 3 (48 seeds each, no gain or worse): jobs only on secure plots, leaving a
plot threatened within two turns, fleeing to the least threatened reachable plot instead of the
nearest city. Not ported from that branch: its defender choice for an overwhelmed garrison of
two or more (a counterattack unit, a stronger defender, Palisade/Walls, Archery Range): it
fired rarely, also ran for barbarian cities, skipped `building_allowed`, and did not measurably
reduce the cities lost.

## Field armies (`military.rs`, `campaign.rs`)

Each army group is a stack; per decision, in order:

1. follow through a group attack the rules queued (`ai_group_attack`);
2. become the garrison of an undefended city of ours it stands in;
3. merge the army units on its plot (up to 8; only units with full moves);
4. attack an adjacent target worth it (never a civ city outside a war plan, nor the target of
   our siege operation, which is left to its strike);
5. heal when hurt and safe (not a stack about to strike);
6. defend a city of ours in peril within 5 turns (not a stack marching on a siege);
7. escort a settler that asks for one (single units);
8. its siege operation (below), if the stack is committed to one: at war (or while preparing
   one) always; against a barbarian city once the force is gathered, or to step back from it;
9. hunt the best barbarian/animal/lair target within 6 turns it can take (entering an empty
   lair destroys it);
10. join a bigger stack within 3 turns, else hold in the nearest city.

**Siege operations** (`campaign.rs`): one per player, rebuilt from the game state on the first
army decision of each `play_turn` call (nothing outside `Game` is remembered).

- *Target*: at war (or preparing one), a revealed enemy city within 20 plots of ours on a
  shared landmass; in peacetime a barbarian city within 14 plots. Every order of an operation
  carries its target as the group's mission AI (`MISSIONAI_ASSAULT` and the plot, as C++
  `AI_setMissionAI`; saved with the game). The city with the most committed units keeps its
  place while the force it needs stays within reach (twice our force at war, our force + 4
  against barbarians); otherwise the best value (400 + 100 per population, wonders, holy city,
  resources, a barbarian town near our cities) for its cost (the force it needs, the
  distance). Without that stickiness the target changed whenever a stack came close enough to
  see a garrison, and stacks walked back and forth between two or three cities.
- *Force*: the strike is played out in expected values with the rules' odds (fortification,
  city defense, first strikes, the attacker's health): collateral attackers first, then the
  strongest; a lost fight still wounds (the rounds won before dying), collateral damage hits
  the other defenders, siege engines that can bring the walls down in three turns count as
  having done so. The force covers the city when the defenders fall with a fifth of the
  attackers (at least one) to spare. Unseen cities: 2 + population/5 defenders for a civ,
  1 + age/25 for a barbarian town.
- *Commitment*: at war every stack on the landmass within 24 plots; against barbarians the
  nearest stacks until they hold twice the force needed (plus those already on their way).
- *Phases*: **Gather** (the force falls short, or the war is not declared yet): at war the
  stacks wait in the rally city (ours nearest the target); against a barbarian town they stay
  where they are, hunting raiders and holding our cities (gathering them in one city bared the
  others); a stack next to the city steps back, so single units no longer camp next to towns
  they cannot take. **March**: to an assembly plot two plots out (one that sees the city, then
  the best defense), and wait there. **StepIn**: the force within two plots covers the city:
  the stacks on the assembly ring step next to it (those still on their way come to the ring
  first). **Strike**: the force next to it covers the city: every committed stack there
  attacks in the same turn -- siege engines bombard, collateral attackers first while several
  defenders stand, then the best odds when 68%+, else the unit cheapest to lose for its
  strength (C++ `AI_getBestGroupSacrifice`) -- while the attackers left still cover the
  defenders left (or the defenders are down to half their health and outnumbered). Up to three
  units follow the victor into the city.
- Campaign moves go one turn at a time (the rest of the path is dropped once the moves are
  spent): an AI group on a multi-turn mission gets no orders until it ends, and stacks walked on
  to plots chosen for an earlier phase.
- Stacks past gathering do not turn back for our cities in peril (the threat usually comes
  from the target; turning back halved the cities taken at war, measured).

Measured on seeds 1-96 (6 civs, standard, 300 turns) against the stack-at-a-time `war_move`
(baseline -> now): conquests of civ cities (game log) 31 -> 73, per war declared (`ffh stats`,
which also counts cities traded or flipped) 0.99 -> 1.85; conquests of barbarian towns 99 ->
199 (41 of them size-1 towns razed on capture, so `captured_from_barb` rises only 135 -> 172);
civ cities captured or razed by barbarians 221 -> 217 (`captured_by_barb`, which leaves out the
razed ones, 147 -> 165: fewer of the lost cities were size-1 towns); barbarian cities at turn
300 6.45 -> 6.24; lone units next to a city of 3+ defenders (seeds 1-12, `FFH_AI_TRACE` "next to"
lines) 43 -> 0; cities, population, techs and score unchanged; same cost per turn.
`CvGame::createBarbarianCities` founds a barbarian city, in the fog, whenever an area holds
fewer than its unowned tiles / `iUnownedTilesPerBarbarianCity`, and at turn 300 about half of
them stand within 6 plots of a civ city with 6 units inside: out of reach of the armies the
civs field. `FFH_AI_TRACE` prints each turn's operation ("campaign vs ..."), every stack next
to its target ("next to ...", with our army units around the city) and each strike ("strike
on ...").

**Assault estimate**: for each attacker, the rules' combat odds against the defender it would
face; the stack's expected "kills" are the sum of the odds plus a third of the rest (a lost
fight still wounds) plus collateral. Attack when the best odds are 65%+, or when the expected
kills cover the defenders (plus a margin for cities) with decent odds or a numbers advantage.
`assault` then attacks repeatedly through `MISSION_MOVE_TO` with `MOVE_DIRECT_ATTACK` until the
plot falls or the odds turn bad, and moves in.

## Naval (`naval.rs`)

- **Overseas sites**: when no site is left on a landmass where we have cities, `sites.rs` also
  scores the coastal plots of other landmasses within 14 plots of our coastal cities.
- **Settling by sea**: a settler with no site it can walk to goes to the nearest coastal city
  and boards a transport there (`loadUnit`, its escort too when there is room); production
  builds the transport (`UNITAI_SETTLER_SEA`; Trireme, Caravel, Galleon...) in that port.
  The transport floods the sea (25 turns), sails next to the best site it can reach and
  moves the cargo onto it (what C++ `groupAmphibMove` orders); the settler founds on its next
  turn. An empty transport sails to a port where a settler waits, else waits in port; cargo
  it has no plan for is unloaded in port.
- **Sea explorers** (`UNITAI_EXPLORE_SEA`, one at a time) reveal coasts while our land is full
  and nothing overseas is known; when nothing is left to reveal, one that can carry becomes a
  transport.
- **Work boats** improve sea resources in our borders (one per unimproved resource, at most
  one per city at a time).

## Diplomacy (`diplomacy.rs`)

Relations come from alignment, state religion, war history and open borders. One war at a
time; wars follow C++ `AI_doWar`'s three kinds, each with the leader's own odds (a
deterministic roll every 5 turns): total war (their defensive power x 2/3 below ours x
`iMaxWarNearbyPowerRatio`), limited war (`iLimitedWarRand`, `iLimitedWarPowerRatio`) and
dogpiling on a civ already at war (`iDogpileWarRand`), only against a civ we have met, do not
like, can reach over land within 16 plots, and whose weakest known city our best attacker can
take (siege engines lower the bar). A broke empire needs a quarter more margin; a bankrupt one
does not start wars. The war plan starts as `WARPLAN_PREPARING_*` (production builds the army,
stacks gather on the rally city) and is declared when the army is ready (or after 40 turns).
Peace is sought when the war goes badly (war success and power), or drags on without gains;
it is offered as a real peace-treaty deal the other AI must accept. Every 5 turns an AI swaps
technologies one for one at similar cost (once `TECH_TRADE` allows it) and signs open borders
with civs it likes; `consider_offer` accepts tech swaps worth at least 80% of what it gives,
never with the civ it prepares a war against.

## Civilization-specific (`civs.rs`)

Rules a few civilizations have that the generic logic does not see. Each branch checks the
player's civilization (by type name), so games without these civilizations play exactly as
before (same checksums).

- **Kuriotates**: past their city limit (3 on a standard map) a new city is a *settlement*
  (`CvCity::init`): radius 2, -75% yields, nothing to build, but no maintenance or civic
  upkeep, and its borders claim land and resources. The settler step stops at the limit;
  `civs::production`, asked right after it, builds one settler at a time for a settlement
  site on the city's landmass (from a city of size 5+ or at its happy cap, at peace, not
  broke). The sites are `sites.rs`'s: the ported `AI_foundValue` already scores
  settlement-sized sites (9 plots) for a sprawling player at its limit and favours sites more
  than 4 plots from its cities. 15 games (52 seeds): cities and settlements at turn 300
  3.2 -> 8.9 (14/14 changed games better), score +16% (+137 +- 49), pop and techs a little up.
- **Lanun**: a work boat can turn an empty coast plot of theirs into a Pirate Cove
  (`SPELL_PIRATE_COVE`: +2 commerce; it grows into a Pirate Harbor and a Pirate Port, 3/2/6
  on coast; no other cove within 2 plots). As in C++ `AI_neededSeaWorkers` /
  `AI_buildPirateCove`, a city that works a plot where one can go trains a work boat (one at a
  time), and a Lanun work boat with no sea resource to improve in reach sails to the best site
  (worked plots of big cities first) and casts there. 23 games (52 seeds): score at turn 300
  +23% (+134 +- 51), pop +24%, techs +22% (Lanun that never research Fishing are unchanged).
- **Ljosalfar / Svartalfar**: nothing needed. Their `MaintainFeatures` keeps the forest under
  every improvement they build on their own plots (FfH's check in `setImprovementType`), and
  workers build only there: `FFH_LOG` shows no forest cleared by them (the clearings in the
  logs are other civilizations'; seed 5: 79 clearings, none by the Svartalfar).
- **Khazad** (tried, not kept): the Dwarven Vault gives every city -2 to +3 happiness (and up
  to +40% production) by the treasury's gold per city (`cf_do_turn_khazad`); the AI holds
  11-20 gold per city, the Empty Vault (-2). A vault target (the level whose happiness and
  production repay its gold within 30 turns, also the reserve for the sliders, hurrying and
  upgrades) was saved for by lowering research. From the start (20-30 points, 6 games):
  population +10%, techs -15% to -20%, lower score. Once commerce reached 8 per city, research
  down at most 10 points (40 Khazad games against the same code without it): gold per city at
  turn 300 17 -> 61, population +2% (+0.9 +- 2.4), techs -3% (-0.6 +- 0.9), score -5 +- 37
  (other limits, 0 or 20 points, gave the same picture). 100 gold per city costs 5-15 turns of
  the empire's whole commerce and the extra citizens repay it in 25+ turns more: by turn 300
  the hoarded gold is research the empire has not got back.

## Measured together

Finance, settlers and escorts, siege operations and the civilization-specific code were
developed separately and merged without changes: their hooks sit in different steps
(`empire::manage`, `production::decide` items 5 and 9, the settler role, `military::army`).
6 civs, standard map, 300 turns, `ffh stats` and the game log (`FFH_LOG`); before = the AI
without the four, after = with them; t is the paired t-statistic over seeds.

| at turn 300 | seeds 1-12, 41-52 | fresh seeds 331-426 |
|---|---|---|
| score (alive civs) | 782 -> 891 (t 3.4) | 818 -> 894 (t 6.2) |
| score (dead civs as 0) | 752 -> 841 (t 2.4) | 789 -> 856 (t 4.9) |
| techs | 18.7 -> 22.2 | 19.4 -> 22.0 (t 7.6) |
| cities / population | 6.14 -> 7.13 / 38.9 -> 41.9 | 6.41 -> 7.07 / 41.1 -> 42.7 |
| research / commerce | 58 -> 87 / 92 -> 120 | 67 -> 88 / 101 -> 122 |
| research slider | 46% -> 63% | 49% -> 62% |
| civs at 20% research or less (at 0%) | 46 -> 7 (28 -> 5) | 166 -> 46 (95 -> 29) |
| unit support, % of commerce | 12.5 -> 9.3 | 12.1 -> 9.3 |
| cities taken civ <- civ / civ <- barbarian / barbarian <- civ | 16 / 29 / 34 -> 26 / 54 / 70 | 65 / 123 / 175 -> 110 / 232 / 240 |
| civs eliminated | 6 -> 8 | 21 -> 26 |
| settlers killed (log) | 33 -> 14 | 106 -> 64 |
| civ-founded cities lost to barbarians (log, first loss) | 45 -> 78 | 220 -> 243 (5.8% -> 6.0% of those founded) |
| CPU ms per turn | 46.5 -> 47.6 | 47.4 -> 47.6 |

Each part alone against the whole (fresh seeds 331-378; score alive / dead as 0 / techs, against
the AI without the four): finance +61 / +67 / +2.2, settlers and escorts -27 / -15 / -0.9,
siege operations -4 / -18 / -0.5, civilization-specific -2 / -2 / 0.0 (only Kuriotates and
Lanun games change), all four +72 / +66 / +2.8. Leaving one out (same seeds, against all four):
without settlers and escorts -6 / 0 / -0.2 and settlers killed 36 -> 82 (alone it costs
research; with the finance module it no longer does); without siege operations +7 / +19 (t 1.3)
/ 0.0, with civ cities conquered 26 -> 10, barbarian towns taken 50 -> 29, civ cities lost to
barbarians 118 -> 88 and civs eliminated 11 -> 7. The siege operations cost about as many cities
to the barbarians as they take from them, as they did alone (towns taken and lost again: civ <->
barbarian recaptures 56 -> 141 over seeds 331-426).

Tried and not kept: stacks committed to a barbarian town turn back for our cities in peril
unless they strike this turn (`campaign::on_the_move` at war only). Seeds 331-378 and 1-12,
41-52: score (dead as 0) +15 (t 2.0), 16% fewer civ cities lost to barbarians; seeds 379-426:
-7, 4% more lost; all 120 seeds: +6 (t 1.2), techs +0.2 (t 1.7), eliminations unchanged.

## Cost control

- **Reachability**: `reach.rs` floods once from a unit (Dijkstra on movement points, bounded
  by a turn budget, unrevealed plots assumed passable) and scores every reached plot; only the
  chosen target gets a real path through the rules' `MISSION_MOVE_TO`.
- **City sites** (`sites.rs`): scored at most once per `update()` per player, only within 7
  plots of our cities (overseas: coastal plots within 14 of our coastal cities, only when
  the home landmasses are full), greedily picked with city spacing.
- Targets, peril, the road plan and the workers' secure-zone map are computed with each view
  (one per AI call of a turn, plus one after diplomacy on the first).
- Measured with `ffh stats` (6 civs, standard map, 300 turns, 20 seeds): about 24 ms per
  turn on average (four games in parallel on four cores), two thirds of it inside the
  controller (including the moves it orders).

## Tools

- `ffh stats --seed N --at T1,T2,.. [--every N]` plays one game and prints one JSON line:
  per-civ snapshots after the given numbers of turns played (and always at the end), game-wide
  values at the same turns, whole-game events (captures by and from barbarians, wars, peace,
  eliminations), the outcome and the turn each civ died. Barbarian losses: `captured_by_barb`
  counts the cities they took and kept, `captured_or_razed_by_barb` also those they razed on
  capture (which otherwise only show in `destroyed`), `workers_captured_by_barb` the units
  they captured as workers (workers, and settlers, which FfH captures as workers; read from the
  game log, which `ffh stats` turns on; logging never changes the game). Snapshots only read the game (the sync checksum is the same with or without them).
  Fields of a snapshot row (one per ever-alive player, barbarians included; all computed from
  game state at the end of that turn):
  - identity: `p` player id, `team`, `civ`, `leader`, `alignment`, `barb`, `alive`, `elim_turn`
    (turns played when found dead), `era`, `religion` (state religion), `vassal`;
  - size: `cities`, `pop`, `land` (plots owned), `land_scored`, `area_tiles`, `area_civs`
    (other civs on the capital's landmass), `overseas` cities;
  - score (top-level `score_version: 2`; docs/SCORE.md): `score` (share of the world in basis
    points, 10000 = all of it) and its shares `score_pop_share`, `score_land_share`,
    `score_know_share` (parts per million); `legacy_score` (FFH2 `calculateScore`, which was
    `score` in output without `score_version`) and its raw inputs `score_pop_raw`,
    `score_land_raw`, `score_tech_raw` (sum of era+1 over known techs), `score_wonders_raw` (5
    per limited wonder); Civ4 `power` and `assets`;
  - technology: `techs` known, `tech_cost` (beakers of known techs at this game's settings),
    `tech_progress` (beakers into unfinished techs), `research` (beakers/turn), `pct` (research
    slider), `research_commerce`;
  - economy: `prod` (hammers/turn), `food`, `food_surplus` (sum of city food differences),
    `yield_commerce` (gross commerce), `commerce` (gold + research commerce), `commerce_total`
    (net of expenses, plus culture and espionage), `gold`, `gold_commerce`, `gold_rate` (net
    gold/turn), `maint`, `ucost` (unit cost + supply), `civ_upkeep`, `culture` (total, /100),
    `culture_rate`, `espionage_rate`, `max_culture_level`, `legendary_cities`,
    `golden_age_turns`, `anarchy_turns`, `impr` (worked improved plots);
  - buildings and people: `wonders` (world wonders), `team_wonders`, `nat_wonders`, `buildings`
    (other buildings), `projects` (team project count), `holy_cities`, `gp_created`,
    `gg_created`, `gp_units` (great-person units alive), `gp_settled` (settled great specialists);
  - units: `units`, `mil`, `workers`, `settlers`, `animals`, `ships`, `heroes` (PROMOTION_HERO),
    `world_units`, `summons`, `casters` (adept + disciple units), `ucombat` (count by unit combat
    type, `NONE` for workers/settlers/great people), `levels` and `xp` (summed over fighting
    units); `mil_str` = sum over non-naval units that can fight of max(attack, defence) base
    strength x (100 + promotion combat %) / 100 x hit-point fraction, `mil_best` (strongest
    such unit), `naval_str` (same over ships);
  - happiness/health: `happy_margin` (sum over cities of happy - unhappy), `min_happy_margin`,
    `unhappy_cities`, `angry_pop`, `disorder_cities`, `health_margin` (good - bad health),
    `min_health_margin`, `unhealthy_cities`;
  - diplomacy and conquest (cumulative, from comparing city owners turn by turn): `met`, `wars`
    (alive civ teams), `taken_civ`, `taken_barb` (cities taken and kept), `razed` (cities that
    vanished with this player's units on the ruins), `lost_civ`, `lost_barb` (taken or razed by
    them), `lost_other`, `founded`, `cities_lost_rule` (Civ4 `getCitiesLost`), `players_killed`.
  Game-wide (`world_at`): `game_turn`, `alive_civs`, `war_pairs`, `armageddon` (counter %),
  `land_plots`, `estimate_end_turn`, and the legacy score's `max_*`/`init_*` (population, land,
  tech, wonders). Players can appear mid-game (the Mercurians join the team of the civ that built the
  Mercurian Gate; the Infernals and empires formed by events get a new team), so aggregate by
  `team`.
  `crates/ffh-cli/scripts/ai_metrics.py --seeds 1-12 [--every 10] [--out runs.json] [--compare
  old.json]` runs it over many seeds in parallel and summarizes.
- `ffh autoplay --report T` prints, per civ, costs, unit roles, improvements, civics and every
  city's yields, trade, happiness and production; `ffh inspect <player>` shows each city's
  plots (worked, yields, improvements).
- `FFH_AI_TRACE=all|<player>` traces AI decisions to stderr; `FFH_LOG=all|<player>` prints the
  game log.
- `--option GAMEOPTION_X` (repeatable) turns game options on for any of these commands (for
  example `GAMEOPTION_ADVANCED_START`).

## Known gaps (future work)

- Naval: no amphibious invasions (wars only target cities on landmasses we share; good
  transports come late in the tech tree), no naval combat, escorts or blockades; ships only
  settle, explore and fish. Overseas settling sends one settler at a time.
- Sparse worlds (2 civs on a standard map): barbarians and their cities dominate the empty
  land; expansion is slow because settlers wait until no enemy is near their city. Letting
  them go out under a held threat founded more cities but lost many more to the barbarians
  (measured), so the gate stays.
- Expansion is slow everywhere in the first 100 turns (about two cities; second city at
  turn 70, third at 110): settlers cost 220 hammers on normal speed (120 + the new-city
  production value) and cities wait for size 3-4 and for the barbarians to leave. Faster
  openings were measured and cost research by turn 300 (see "Settlers and escorts").
- Economy: research at about 60% of the commerce by turn 300; cottages need
  Education, libraries Writing; maintenance and civic upkeep take a quarter of the commerce.
  The civs still stuck at low research expanded to 7-8 cities in Despotism before reaching
  Cartography: only the settler gate (`finance::city_affordable`, not wired in) prevents that,
  and gating expansion costs cities (no better score by turn 300).
- Sieges: one operation at a time; armies are small (at war the operation spends half its turns
  gathering, typically 2-3 units against a need of 4-6), so a barbarian town of 5+ archers is
  out of reach and is left alone; casters do not join the strike (they only follow the army); a
  city emptied of defenders is entered even when a counterattack will retake it. Tried and
  dropped (no measurable effect on seeds 1-48): raiding (pillaging) while a war operation
  gathers, a "can we hold it" check before the strike, every unit following the victor into the
  city, stacks marching on a barbarian town turning back for cities in peril (fewer towns
  taken, no fewer cities lost), barbarian targets out to 20 plots, a larger strike margin.
- No hurrying beyond the civic-gated gold/population hurries, no wonder racing, no vassals or
  defensive pacts; the vote sources (Overcouncil / Undercouncil) use the defaults.
- Casters use FfH's own end-of-turn spell choice; there is no planned use of summons or
  terraforming, and no dedicated logic for most civ-specific mechanics (Sheaim planar gates,
  Infernal and Mercurian projects, Khazad vaults, Armageddon rituals; see `civs.rs` for the
  ones handled).
