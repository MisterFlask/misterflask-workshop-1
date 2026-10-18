# AI design (ffh-ai)

The C++ AI (CvPlayerAI/CvCityAI/CvUnitAI/CvTeamAI, ~60k lines, plus the Python AI hooks
in CvGameUtils.py such as `AI_chooseTech`/`AI_chooseProduction`) is not ported. It is the
main cause of FFH2's late-game turn times (per-unit re-evaluation of every plot, repeated A*
searches, Python callbacks). `ffh-ai` is a new design with two hard requirements -- legal play
and determinism -- and a standing cost preference:

- **Legal play only.** It acts through the same rules APIs a human would: missions pushed on
  selection groups (`group_push_mission`), city orders (`city_push_order`), research
  (`player_push_research`), civics/religion/sliders, deals (`implement_deal`), spells
  (`unit_cast`), upgrades/promotions, scrapping. The rules decide what is allowed; a rules
  bug is reported, never worked around in the AI.
- **Cost is a deferred concern, not a hard gate.** Prefer cheap shapes -- a few bounded floods
  per group, one real path for the chosen target, no per-unit full-map scans -- but do not reject
  a feature on cost alone: richer deterministic computation is allowed, and optimized (a
  behaviour-preserving bounding pass that computes the same decisions faster) only once a
  benchmark shows a real problem at the scale the game reaches. Determinism below is still hard.
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
| `CvPlayerAI::AI_chooseResearch` / `AI_chooseFreeTech` | `choose_research` | `research.rs`, `tech_plan.rs` |
| `CvPlayerAI::AI_conquerCity` | `raze_conquered_city` | `diplomacy.rs` |
| `CvPlayerAI::AI_considerOffer` | `consider_offer` | `diplomacy.rs` |
| `CvPlayerAI::AI_doAdvancedStart` (from `CvPlayer::setTurnActive`) | `play_turn` while start points remain | `advanced_start.rs` |
| `CvPlayerAI::AI_chooseEvent` | `choose_event` | `events.rs` |
| elections, votes, captured/gifted units | defaults in `controller.rs` | -- |
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
   researches its way out, see Finance; the victory goal's techs beeline, see Victory pursuit);
   civics every 10 turns when the gain beats the anarchy (sooner under a structural deficit);
   state religion; the culture slider a cultural victory wants, then the highest research
   slider that keeps the treasury from running dry; unit support that holds research below its
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
   plots need improving; the victory goal's items (`victory::production`: towers, culture in
   the cities nearest legendary; before expansion, item 4, when the goal is 30 turns away);
   heroes (not at the cost of the first expansion);
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
  20% research or less. It overrides the research plan (`tech_plan.rs`; see Research for the
  combinations measured).
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

**Conquest gets no XML weight (`empire::civic_value`).** `civic_value` starts from the civic's
`iAIWeight` per city. Conquest's is 700, more than any economy civic's other terms, so at turn 300
29 of 56 civs ran it (seeds 1-8; Agrarianism 12, Foreign Trade 4). Under Conquest
(`bMilitaryFoodProduction`) a city training a military unit puts its whole food surplus into the
unit and stops growing (`city_food_difference`), and production trains units most of the time: 71
of the 107 Conquest cities training units showed food +0 over a positive raw surplus (288 food a
turn into hammers), against none outside Conquest. The weight is dropped for such civics, which are
then valued by their other terms like any civic (FfH's own `AI_civicValue` penalises the food rule
only below 30 population; this AI's production ladder keeps the rule active far more often).

Measured against the AI before it (6 civs, standard map; t is the paired t over seeds):

| | 300 turns, seeds 801-944 (144 games) | full games, seeds 901-916 (`--until-supremacy`) |
|---|---|---|
| legacy score | +3.6% (t 7.8) | +5.0% at turn 300 (t 4.9), +1.7% at turn 450 |
| techs | +1.4% (t 2.7) | +3.4% (t 2.5), +4.9% at turn 450 (t 1.7) |
| research / commerce / production | +3.8% / +3.3% / +3.8% (t 2.8 / 3.5 / 3.6) | |
| population | +9.8% (t 12.9) | +10.8% (t 5.5) |
| power / military | -0.7% / -0.8% (t -0.9) | +2.2% / +4.0% at turn 300; flat at 450 |
| civ cities captured | -5.7% (t -1.3) | +6% |
| civs eliminated | -13% (t -2.5) | +8% |
| outcome | | supremacy 7 -> 9 of 16; decided 30 turns earlier (548 -> 518, t -1.1) |

Variants measured on the same seeds and dropped: the weight only for cities at their happy cap
(legacy +2.6%, pop +7.7%: the same direction, weaker everywhere), the weight kept while at war or
preparing one (legacy +1.5%, pop +4.6%; no power gained by running Conquest at war).

## Research (`research.rs`, `tech_plan.rs`)

Score = value / (turns to research + 4), with one level of lookahead (children at a quarter of
their static value and of the improvements they bring: cottages behind Education). Value terms:
the leader's favorite early techs and religion, an unfounded religion (1500, the leader's
favorite 1200 more), flavors, AI weights; buildings the civ can use (a static value, plus
production's marginal value of the building in the six cities that gain most: libraries where
there is research to multiply, markets where there is gold); land improvements counted on our
cities' plots (growing improvements such as cottages at two thirds of their grown yield,
commerce worth more when broke); a better defender or attacker weighted by the military
pressure we see (barbarians near our cities, war, time), with an early "get past warriors"
bonus, also through the next tech (Mining -> Bronze Working); the first arcane unit; happiness
buildings when cities are capped; better civics than the ones we run (City States and
Aristocracy cut maintenance); technology and open-borders trading with the civs we know; the
first transport when our landmass is full or a settler waits in port (and the tech before it);
processes (Writing, Mathematics) when cities have nothing left to build. `FFH_AI_TRACE_TECH=1`
with `FFH_AI_TRACE` lists every candidate's value and score, and the goals below.

**Goals (`tech_plan.rs`).** On top of the score, research follows an ordered list of goals
adapted from FfH's own AI (`CvGameUtils.AI_chooseTech`), recomputed at every choice (no
cache): a tier-2 military tech until one of Bronze Working, Archery, Hunting or Horseback
Riding is known (the first we can ever research of the leader's favorite early military tech,
the civ's default, Hunting for the elves and Construction for the Luchuirp, and Bronze
Working); Exploration, Mining, Calendar, Ancient Chants, Education; an early religion nobody
has founded yet, unless we know one of the three early religion techs or have a holy city
(Sanitation for agnostics); Code of Laws once there are 4+ cities or maintenance takes a fifth
of the commerce; Writing (not for `TRAIT_BARBARIAN`); Mysticism, Festivals, Cartography,
Sailing for coastal empires; Trade, Sanitation, Currency, Construction, Taxation; the per-civ
beelines (Amurites Bowyers/Sorcery, Bannor Fanaticism, Calabim Feudalism, Hippus and
Kuriotates Stirrups, Sheaim Corruption of Spirit/Necromancy/Sorcery, Sidar and Svartalfar
Poisons, Luchuirp Construction then Elementalism/Sorcery, Elohim and Malakim Priesthood, Iron
Working for the Khazad, Clan of Embers, Doviello, Lanun, Grigori, Illians and Balseraphs, then
Medicine for the Grigori); the magic spheres once tier 3 and economy tier 2 are reached. The
religion is the first unfounded one of the player's preferences, most preferred first: the
civ's own (Runes of Kilmorph for the Khazad, Fellowship of Leaves for the elves), the leader's
favorite early and favorite religions (data; FfH leaves these unset), then -- data-driven from
each religion's own `<Alignment>` -- the religions whose alignment (or best alignment) fits the
player's, cheapest to found first, so a good civ still grabs the cheap Runes of Kilmorph early
while the deeper aligned faiths (The Order, the Ashen Veil) follow as fallbacks; then the three
early religions as a last resort. Reading the alignment from the data (rather than a hardcoded
good->Earthmother / evil->Message-from-the-Deep map) keeps the choice correct and makes the
later religions reachable as content changes; measured economy-neutral (identical through turn
150) with the later religions now founded where before only the early three appeared.
This is not `AI_chooseTech`'s order: that function returns one tech from tier-gated blocks in
which the last assignment wins (the tier-2 military tech, then the early economy, then once
both are known the religion, Cartography, Festivals, Mysticism, Writing, Animal Husbandry,
Code of Laws for financial leaders with 6+ cities, Archery, Sailing, the leader's early techs
and wonder, then economy tier 2, the civ beelines and the tier-3 block). The list keeps those
main precedences without the tier gates, takes Writing to Cartography in the reverse order,
and leaves out Archery, Animal Husbandry, the leader's early techs and wonders, the
starting-resource techs (the score's improvement terms cover them) and the tier-3 extras
(victory techs, favorite wonders, religion heroes). The barbarians have no goals. The
researchable techs on the prerequisite path (cheapest OR branch) of the first two unmet goals,
the second at half weight, get the goal's own value plus 6000, shared by each step's part of
the path's turns (+4), and at least 150 per turn of the step: every step then scores like
researching the whole path, so a cheap gateway (Ancient Chants) inherits Education's value.
The situational terms are not scaled down and still win when large (military pressure,
transports, processes).

**City defense first (`research::defense_goal`).** Under military pressure 4+ (barbarians
near our cities past turn 60, or a war) while the best city defender we could train is below
an archer's (5 defense, +25% in cities), the cheapest researchable tech that brings one
(Archery) is researched next, whatever the plan says. Without it, civs that met the tier-2
goal without a city defender (Bronze Working without copper, Hunting, Horseback Riding, or a
leader whose "military" favorite is Festivals) never researched Archery, and established civs
fell to the barbarians mid-game (seeds 25-72: 9 civs with cities at turn 200 eliminated by
turn 300, 2 before the plan, 0 with the defense goal).

**Finance first.** The deficit escape of `finance.rs` runs before the choice and replaces it
while it applies (research at 30% or less, or a structural deficit). Measured on seeds 1-24
against that order, before the defense goal (civs at 20% research or less at turn 200, techs
at turn 300): the escape
tech as the plan's first goal instead of an override 7 -> 11, +0.1; an override only at 30%
research or less, a goal otherwise, 7 -> 13, -0.6; no escape at all 7 -> 16, +0.1.

Measured (6 civs, standard map, 300 turns, `ffh stats`; seeds 1-72, the AI before the plan
against the plan with the religion fix and the defense goal; t is the paired t-statistic over
seeds):

| at turn 300 | before | after |
|---|---|---|
| techs | 21.5 | 28.7 (t 13.2) |
| research (beakers a turn) | 85 | 144 (t 12.3) |
| legacy score (alive civs / dead as 0) | 874 / 819 | 1090 / 1033 (t 11.9 / 8.7) |
| cities / population | 7.1 / 41.4 | 7.6 / 50.1 |
| religions founded per game (turn 200 / 300) | 0.1 / 1.2 | 2.2 / 3.7 |
| civs at 20% research or less | 37 | 25 |
| cities lost to barbarians / to civs | 215 / 82 | 149 / 97 |
| civs eliminated (with cities at turn 200) | 27 (4) | 23 (4) |
| ms per turn (`ffh bench`, seeds 1-8, one game at a time on a shared machine) | 48.1 | 51.8 |

The time per turn follows the larger empires (more cities, people and units; per seed -16% to
+47%, the games being different, on a machine shared with other jobs).

The religion fix: before it, every civ headed for Way of the Earthmother while Runes of
Kilmorph was unfounded, then Way of the Forests, then Message from the Deep (the fallbacks
were searched first), so the leaders' preferences were never used; the totals barely moved
(seeds 1-24: techs 30.9 -> 30.5, religions per game 3.9 either way), but the civs now head
for their own (seeds 1-8: the Octopus Overlords founded by evil Balseraphs, Calabim and
Doviello, where the first-come order gave it to whoever was left). Without the defense goal the
plan reached 30.0 techs and 1135 legacy score (alive civs) but lost 378 cities (236 to
barbarians, 142 to civs) and 37 civs (12 with cities at turn 200).

Tried and not kept (seeds 25-72, against the plan without the defense goal: civs eliminated,
techs): Archery as a fixed goal right after the tier-2 goal (23 -> 19, 29.8 -> 26.8), after the
early religion (24, 29.0), after Cartography (25, 29.5); the plan's bonus scaled down by
military pressure, to 75% / 50% / 35% at pressure 4 / 5 / 6 (16, 27.4).

**Guns vs butter: siege research (`research::siege_offense`).** Catapult-class siege engines
crack city walls (bombard, collateral) but have low raw combat, so the score's military terms
(attacker/defender strength) never pick their tech -- Construction was chosen about once a game
where Archery was chosen fifty times, and civs ground against walled cities with no answer. A
contextual goal (the user's guns-vs-butter hunch) can route a civ toward its siege engine: when
its campaign is marching on a walled or well-garrisoned rival city that needs siege
(`campaign::needed_force().siege > 0`) **and** it is decisively stronger than that rival -- the
same `TOTAL_WAR_RATIO` (power at 140% of the target's defence) diplomacy uses to commit to total
war -- the nearest researchable step toward the engine is beelined (score `SIEGE_SCORE`, above
the economy plan, below the survival `defense_goal`). A defensive, parity, or underdog civ, and a
barbarian target, keep their research on economy. The siege engine is found data-drivenly (any
trainable unit of our civ with combat, bombard/collateral, and a city-attack/collateral unit AI --
no unit name), mirroring production's `Role::Siege`.

The gate is the finding. Measured on seeds 1-16, 690 turns, against the AI with no siege goal
(captures per war = conquest conversion; techs at fixed turns = economy; final power/empire =
domination):

| vs no-siege baseline | every offensive civ (ungated) | strength-gated (kept) |
|---|---|---|
| captures per war | +19.3% | -4.8% (noise) |
| techs @ turn 300 | -7.2% | +1.0% |
| techs @ turn 450 | -8.4% | +4.8% |
| final power | -7.7% | +0.9% |
| avg max empire | -5.2% | -0.5% |
| civs eliminated | ±0% | -9.1% |

Routing *every* offensive civ to siege works as a weapon -- wars convert far more decisively
(+19% captures per war, and mid-game power at turn 300 rises ~11%) -- but it is a losing
guns-vs-butter trade: the research a civ spends beelining siege (-7% techs by turn 300) compounds
against it, and by the endgame its power and territory are down ~7%. Lowering the beeline's
priority did not help (`SIEGE_SCORE` at 0.6M instead of 1.5M: still techs@300 -6.5%, final power
-7.4%) -- the civ pursues siege until it has it either way, so it pays the same research; only
*when* changes. The conversion gain and the economy cost both come from the same civs, the
near-parity attackers who must crack walls, so tuning cannot separate them.

The strength gate separates them by excluding that population: a civ decisively stronger than its
target resolves its war fast enough for a cracked city to pay off before the research deficit
compounds. That gate is non-regressive (economy and power flat-to-up, 9% fewer civs eliminated),
but it fires rarely -- about one to three times a game (`FFH_AI_TRACE_TECH`): by the time a civ is
that dominant it has usually already teched past Construction, or its targets are too weak to need
siege. So the validated guns-vs-butter answer is **butter almost always wins** -- the civs that
most need siege are the ones that can least afford its research -- and the AI now reserves a siege
beeline for the rare dominant attacker who can. The ungated version is kept out; the gated one is
kept in as the holistically-correct (if seldom-triggered) policy, strictly better than a baseline
that never deliberately chose siege at all.

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
resources. Score / (travel + build time). Workers flee when enemies can reach them and skip
plots under danger. FfH's AI-only build bans (`CvGameUtils.canBuild`) are respected: no forts,
no farms for Fallow civs, no workshops before Guilds, no cottages under Aristocracy, no
windmills under Arete.

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
  having done so. The force covers the city when the defenders fall; committing to *march* wants
  a fifth of the attackers (at least one) to spare (so stacks do not advance understrength), but a
  committed force already positioned next to or beside the city *strikes at acceptable odds* -- it
  expects to clear the defenders without keeping that reserve -- rather than retreating to muster
  an overwhelming force against a target that only hardens while it waits (a lone unit still never
  qualifies: it cannot clear a city). Unseen cities: 2 + population/5 defenders for a civ,
  1 + age/25 for a barbarian town -- but the force needed to *approach and reveal* an unseen
  target is capped at a scouting-sized stack (not that growing assumption, which otherwise made
  a hardening barb town need an ever-larger force so the army mustered forever and never marched
  to look). Once the stack reaches the assembly ring and the real defenders show, the strike /
  step-back decision above applies. Measured (seeds 1-8, turn 300): barb towns taken 5 -> 8 with
  losses to barbarians flat (8 -> 9) and the economy unchanged.
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
- **Amphibious invasion** (`amphibious_target`, `UNITAI_ASSAULT_SEA`): a civ can ship an army
  across water to take an enemy city on a landmass where it has no city (the siege campaigns
  only target landmasses we already hold a city on). The target is the best overseas enemy (or
  barbarian) city -- revealed, coastal, on a landmass with no city of ours, within 24 plots of
  one of our coastal cities, whose weakest defender our best trainable attacker can beat
  (`can_take_city`) -- by value (population, wonders) for its cost (force needed, distance);
  deterministic ordering, recomputed each turn. *Production* (the coastal city nearest the
  target, the staging port): a fleet of assault transports big enough to carry the whole force in
  one wave (`ceil(need / cargo per ship)` of them -- one transport only trickles a few units per
  trip, which rarely masses enough to take a defended city), then attackers up to what one assault
  needs (`need` = defenders x2 + 1, capped). *Execution* reuses the overseas-settling
  machinery: the nearest army stacks (up to `need`) walk to the staging port and board the
  transport there (as a settler and its escort board); the transport, carrying an army rather
  than a settler, sails to a water plot beside the target city and lands the army on a land plot
  next to it; the landed stack then takes the city with `military::assault` (which holds rather
  than throwing the stack away if the revealed defenders are too strong). Every trip lands on the
  *same* plot -- the reachable land plot beside the city already holding the most of our army (then
  the soonest-reachable, then the strongest ground) -- so a trickle of small landings piles onto
  one plot and merges (`military::army` merges army units sharing a plot) into a stack that can
  strike, instead of scattering across the city's approaches and never massing. One transport trip
  still carries only the ship's cargo capacity, so with early, small transports the force builds up
  over several trips; bigger invasions wait for better ships.

## Diplomacy (`diplomacy.rs`)

Relations come from alignment, state religion, war history and open borders. One war at a
time; war is **greedy expansion**, not a dice roll: when free to start one (checked every 5
turns from turn 50, with 2+ cities), a civ prepares war on the met, land-reachable (within 16
plots) rival whose weakest known city its best trainable attacker can take (siege engines
lower the bar, `can_take_cities`), when our power is a sufficient edge over their defensive
power. The territory is the point -- domination and conquest are this rule run to its
conclusion -- so there is no grudge requirement and no roll. Flavour biases the bar, not the
drive: the leader's `iMaxWarRand` (low = war-eager) sets the required power edge, from near
parity (an eager warmonger accepts a slight underdog as acceptable risk) up to clear
superiority for a cautious leader, who also spares civs it likes; the greediest leaders strike
any profitable, reachable target. A broke empire wants a bigger edge; a bankrupt one does not
start wars. (This replaces the earlier port of `AI_doWar`'s three odds-gated kinds, which left
strong civs idle for want of a grudge or a lucky roll.) Reachable means by land (a shared
landmass within 16 plots) or, failing that, by sea: a civ whose takeable city is within
sea-reach of one of our coastal cities (`naval::amphibious_city`) is a valid target too, so an
isolated or across-the-water rival can now be invaded (see Naval / amphibious invasion). Among
the targets we can reach and take, a flavour nudge prefers one of the opposed alignment (good vs
evil, the core FFH conflict): a small bonus added to the target score, after the reachability
gate, so it only re-ranks viable targets and never sends a civ at a rival it cannot reach. In an
8-game A/B it lifted good-vs-evil wars from 35% to 59% of a civ's wars where opposed rivals were
reachable (inert where they are across the water), with captures and economy slightly up, not
traded -- a secondary bias that does not cost the primary. The
war plan starts as `WARPLAN_PREPARING_*` (production builds the army,
stacks gather on the rally city) and is declared when the army is ready (or after 40 turns).
Peace is sought when the war goes badly (war success and power), or drags on without gains;
it is offered as a real peace-treaty deal the other AI must accept. Every 5 turns an AI swaps
technologies one for one at similar cost (once `TECH_TRADE` allows it) and signs open borders
with civs it likes; `consider_offer` accepts tech swaps worth at least 80% of what it gives,
never with the civ it prepares a war against.

**Resources and gold (`trade.rs`).** The AI also trades resources (bonuses) and gold -- a port of
the base game's `CvPlayerAI` trade valuation (`AI_bonusTradeVal`, `AI_bonusTrade`, `AI_dealVal`,
`AI_goldTradeValuePercent`/`AI_goldPerTurnTradeVal`, `AI_maxGold(PerTurn)Trade`). The value of
*holding* a resource is ffh-core's already-ported `player_ai_bonus_val` (happiness/health,
unit/building enablement, the FfH mana and evil-resource terms); `trade.rs` is the trade layer on
top: a resource's trade value (scaled by the smaller of the two empires, the data
`AITradeModifier`, over a treaty's length), a deal's total gold-equivalent value (`deal_val`, techs
via the research-cost proxy since `AI_techTradeVal` is unported), gold valuation (worth more when
broke) and the gold caps (a research buffer plus an already-traded-to limit), and the give-refusal
(`bonus_trade`: never to our worst enemy, nor a resource they already have, nor a strategic /
happiness / health resource to a civ we do not like enough -- one step more generous toward a civ
of our alignment or favourite religion). `consider_offer` values any resource/gold/mixed deal
(accept when what we receive is worth at least what we give, no gold-for-gold, never a resource the
refusal blocks); `trade()` (AI<->AI) and `offers.rs` (AI->human) buy the resource we most want and
a neighbour can spare, paying with a surplus of ours or with gold, only at peace with civs we like.
No corporations exist in FfH 0.41, so that value is 0 and its branches are dead; `AI_getAttitude`
and `AI_isFinancialTrouble` reuse this AI's `attitude`/`production::financial_trouble` proxies
(docs/PORTING.md). It adds no saved state -- deal memory rides ported `Game` fields updated by
`implement_deal` -- and draws no RNG (deterministic, checksum-stable).

Measured (16-seed, 690-turn A/B vs the no-trade tip): trading is positive-sum, so every civ grows
-- final power +8.7%, research at turn 300 +3.4% (the hard guard), and 7% fewer civs eliminated
(weaker civs trade to stay viable). The cost is less *concentrated* domination -- biggest empire
-4.5%, top-power share -9.4%, and games run longer (6 vs 3 reaching turn 690) -- but the leader's
absolute power is about flat (it is the field catching up, not the leader weakening), and research
at turn 450 slips -3.1%. Kept as faithful, more human-like play that grows total power and mid-game
economy at the cost of some domination concentration.

**Peace with reparations (`diplomacy::end_war_val`, `trade::peace_tribute`).** A port of
`CvTeamAI::AI_endWarVal` and the `CvPlayerAI::AI_doDiplo` peace construction: ending a war carries a
value (large when weak/losing, cut to a fifth once we dominate at >120% power -- the FfH
divide-by-our-power variant, plus the land-ratio cut and the war-plan ×2/×1.5 multiplier; the
DAGGER/financial/warRand warmonger sub-factors are omitted as unported). The gap between the two
sides' `end_war_val` funds reparations: when clearly winning, an AI now *initiates* peace and demands
tribute (the costliest tech the loser can give, then gold) instead of a valueless white peace, and a
losing AI *sweetens* its peace offer with tribute of its own so the winner will accept -- converting
military success into economy. A civ pursuing conquest/domination against that enemy presses on
rather than cashing out (victory-pursuit override in both the offer and `consider_offer`), and so does
a civ *overwhelmingly* winning any war (`press_conquest`: our power >= 200% of the enemy's defensive
power) -- it takes cities instead of selling the war for a token tribute. The latter guard keys on the
war's power edge, not the civ's victory goal, because an adversarial check of the first A/B found
strong civs (2-5x power) tribute-farming weak victims for whole games instead of finishing them -- the
primary (conquest) traded for a secondary tech. Below 2x the enemy's cities are not reliably stormable,
so a tribute peace is a sensible outcome and the guard does not fire. The old
willingness (take a bare peace unless clearly winning, and always past a 60-turn exhaustion floor)
stays as the fallback, so even/stalemated wars still resolve and research is not starved.
**Deliberate deviation from the C++ quirk:** `AI_doDiplo` adds tribute gold at *face* value because
the proposing AI implements the deal unilaterally; here AI-AI deals are judged by the *recipient's*
`consider_offer`, whose `deal_val` scales gold by `gold_trade_value_percent` (200-300%), so face-value
gold would read as overpriced and always be refused. `peace_tribute` instead builds the gold that is
*worth* the gap under `deal_val` (the `AI_counterPropose` scaling), so the deal balances for the
judge. Cities-as-tribute are omitted (`AI_cityTradeVal` unported). No new saved state; no RNG.

Measured (16-seed, 690-turn A/B vs the pre-reparations tip, with the `press_conquest` guard):
captured cities +16%, civs eliminated +25%, the leader's world-share at turn 690 35.9% -> 42.5%
(x1.20 -> x1.40 over the runner-up) -- a markedly stronger march toward domination, the primary
goal. War declarations are *down* 10% and decision turns slightly faster, so pressing conquest
resolves wars rather than dragging them. Research at turn 300 is flat within noise (-1.7%, lower in
10 of 16 seeds / higher in 4, per-seed swings +-46), i.e. the economic engine is not sacrificed.
An adversarial trace audit of the first (un-guarded) cut caught strong civs tribute-farming weak
victims for whole games instead of conquering them (primary traded for a secondary tech); the
`press_conquest` guard fixed it (worst seed: peace events 31 -> 6).

**Gift tech to a weaker ally (`give_help`).** A port of C++ `AI_doDiplo`'s give-help (FfH "Better
Diplomatics" block B): a team leader gifts a free technology to a civ markedly weaker than itself
(team assets below half ours) that it likes (attitude over the target leader's `NoGiveHelp`
threshold, eased one step by shared alignment or favourite religion), at peace, on the deterministic
`contact_due` cadence (~150 turns). Deliberate deviations from the C++: the gift is the *cheapest*
tech we can give (deterministic -- ffh-ai draws no RNG -- and so it never hands over an expensive key
enabler) rather than a random one; and we never gift a civ that is our victory rival (the project's
"do not prop up a rival" rule). The gift does not raise attitude (`relation()` does not track gifts;
C++ `AI_getTradeAttitude` is unported), matching how resource/open-borders trading was integrated.
Like the base game it is rare (the gating plus the ~150-turn cadence fire it only a handful of times
across a 16-game set), so it is a faithful, low-impact "keep an ally alive" gesture rather than a
force on the outcome. No new saved state; no RNG. (Measured 16-seed/690-turn A/B vs the
peace-with-reparations tip: flat within noise -- research at turn 300 +1.2%, leader world-share at
turn 690 42.5% -> 42.8%, war/peace/elimination counts unchanged -- with research at turn 690 +11% and
one domination win, i.e. harmless-to-slightly-helpful, as expected from a feature that fires only a
handful of times.)

**Hire a war ally (`hire_war_ally`, `declare_war_trade_val`).** A port of C++ `AI_doDiplo`'s paid
war-bribe (`TRADE_WAR`): a civ already at war pays a third AI, at peace with it, tech + gold to
declare war on one of its enemies -- turning gold into a second front. The price is a faithful port
of `CvTeamAI::AI_declareWarTradeVal` (the target's size, the bribee's attitude to the target and to
us, the relative powers, a dogpile bonus when the target has no other wars, scaled up as the game
ages); `consider_offer` values an incoming bribe with the same function and runs the AI half of
`AI_declareWarTrade` that ffh-core leaves open (one war at a time, the target not too strong even
with our help, and the bribee's attitude to briber and target). Deterministic deviations from the
C++: the target is the enemy the bribee most dislikes (`AI_getAttitudeWeight`) and the payment is the
costliest tech(s) the bribee would value plus a gold top-up, rather than the C++ random picks; a
staggered period from the leader's `DeclareWarTradeRand` replaces the 1-in-N roll; and we never pay a
civ that is our victory rival (do not strengthen a would-be winner). `AI_isAllyLandTarget` is unported
(assumed false); nukes are 0 in FfH 0.41. No new saved state; no RNG.

Measured (16-seed/690-turn A/B vs the gift tip, and an adversarial audit of briber bankruptcy and
bribee prop-up): research at turn 300 flat (-0.9%, hard guard PASS -- it moves on only 2 seeds); war
declarations flat (285 -> 286, so bribes organise dogpiles onto existing wars rather than spawning new
ones); and more games resolve with a genuine victory before the turn cap (8/16 -> 11/16, domination
wins 1 -> 2). The audit refuted the two risks: the winning team is unchanged in 15/16 seeds and the
most-bribed civ never snowballs (bribe + tribute income is too small to move outcomes -- leader
world-share barely moves, 42.8% -> 43.7%), and bribers cannot bankrupt themselves (`max_gold_trade`
caps the gold under treasury with a deficit buffer, payment is tech-first, and an offer is made only
when the payment is worth over three quarters of the price) -- in practice the bribers are the
*winners*, converting gold into a decisive second front (seed 5: the dogpile cuts the target from 13
cities to 2 and the briber-side winner closes out by domination 64 turns early). No new saved state;
no RNG.

## Random events (`events.rs`)

The rules offer an AI player the trigger's events it can do, in trigger order (`player_trigger_data`).
Until `choose_event` was implemented the controller default took the first, and FfH lists the
decline / pay / lose-a-unit answer first in many triggers: War Games disbanded the unit, the Strange
Advisor added inflation for good, Clairone's first answer turned down a free Great Artist, the Spider
Mine's third released a Giant Spider, and every Adaptive leader (Varn, Cardith, Cassiel) became
Aggressive every 100 turns. About 78 events fire per 300-turn game, about 25 of them with a real
choice (seeds 1-8).

`events::value` follows C++ `AI_eventValue` term by term in one unit (gold) instead of the C++ mix
of gold, hammers x100 and per-turn yields x20: gold, tech progress, free units (great people 600)
and buildings at twice their hammers, a disbanded unit and its experience, golden ages, building
yield/commerce changes in the cities that have the building, happiness and health only where a city
is at or near its cap, food, population, revolt and hurry anger, plot yields, improvements, routes
and resources, experience and free promotions, unit support, inflation on the pre-inflation costs,
the Armageddon counter by alignment, religion conversions, attitude, follow-up and cleared events.
Permanent changes count 50 turns. No random draws: `rand(iAIValue)` is its mean, the final +0-20% is
dropped. Python callback effects are 0 as in C++, except a short table of frequent clear ones
(hostile spawns, the Iron Orb's free tech, a seceding city) and the Adaptive trait choice, valued
from the trait's data (`trait_value`: extra yield at the threshold on worked plots, civic upkeep,
commerce per city, capped happiness and health, great people rate, wonder production, free
promotions, no anarchy). An event that would declare a war we are not in is never chosen.

Measured (6 civs, standard map, 300 turns, seeds 801-944, 144 paired games against the first-option
default; t is the paired t over seeds): at turn 300 legacy score +1.2% (t 1.0), techs +1.4% (t 1.0),
research +2.0% (t 0.8), production +2.5% (t 1.4), research slider +2.9% (t 1.5; +3.8%, t 2.4 at turn
200), culture +8.2% (t 1.8); captures, eliminations and wars unchanged. A small lever (a few real
choices per civ in 300 turns), economy-positive throughout and no regression.

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

### Round 3b: goal-directed research + victory pursuit together

Re-measured on fresh seeds (601-648 at 300 turns, 601-624 at full 690-turn games with
`--until-supremacy`) against master, both verified to survive and to be synergistic, so both were
merged:
- **beelines-fixed** alone (601-648, 300 turns): techs 22.1 -> 30.3, research 83.7 -> 147.9,
  legacy score 876 -> 1115, cities 6.9 -> 7.6, civs eliminated 16 -> 8; CPU flat.
- **victory-pursuit** alone (601-624, full games): actual cultural victories 1/24 -> 10/24, games
  decided ~31 turns earlier, at a late cost (research -17% at t500, legacy -2% at t690); early
  game (turn 300) unchanged.
- **Together** (601-624, full games): cultural victories **13/24**, decided earliest (avg 509 vs
  master 562, victory-pursuit 531), and the research victory-pursuit spent on the culture slider
  is restored by beelines (research at t500 251 vs victory-pursuit's 204, master's 245; legacy at
  t690 3073 vs 2907). Leave-one-out (cultural victories per 24 full games): master 1, beelines
  alone 8 (its stronger economy reaches the culture threshold by itself), victory-pursuit alone
  10, together 13 -- so beelines adds +3 over victory-pursuit alone and the planner adds +5 over
  beelines alone; each contributes. The goal-directed economy reaches the (achievable)
  cultural victory faster, so the two reinforce rather than fight. The combined research now
  weighs the plan's goal_bonus against the victory goal-tech bonus, so the AI no longer always
  beelines a single expensive victory tech (e.g. a rare Tower run); that is judged by the win rate
  above, not by a unit test (`tower_victory_makes_the_last_tower_tech_the_goal`).

## Victory pursuit (`victory.rs`)

Without it the AI never aimed at a victory: 19 of 24 full games (seeds 1-24, 6 civs, standard
map, normal speed, played to the rules' end at turn 690) ended on the time limit, the other five
were won by civs that got there by accident (3 cultural, 2 domination).

**Estimates** (pure functions of the game state, nothing cached; O(cities + techs) per civ): the
turns each civ needs for each condition of `CvGame::testVictory` (`CIV4VictoryInfo.xml`).
- *Cultural*: three cities at legendary culture (50000 at normal speed). Per city, the culture
  missing at its culture rate with the slider all in and the culture process (production to
  culture at 50%); the third city's turns. Before Drama frees the culture slider, Drama's
  research is added.
- *Tower of Mastery*: the Towers of Alteration, Divination, Necromancy and the Elements (800
  hammers each) and the Tower of Mastery (4000, Strength of Will): the missing techs'
  research (prerequisites included, the lesser towers built meanwhile in the four best cities),
  then the Tower of Mastery in the best city. FfH makes a building's `PrereqBonuses` AND
  requirements (`CvCity::canConstruct`): each lesser tower needs all four of its manas, 16 in
  all. The estimate is out of reach while the raw mana nodes in our cities' radius cannot supply
  the manas we lack (the AI does not trade for mana).
- *Domination*: land at 78% - 2 per civ team ever alive (66% with 6 civs) and people 25 points
  above the runner-up's share; once we hold half the land needed, the cities still to take at
  30 turns each. *Conquest*: when our power is twice all the rivals' together, their cities at
  30 turns each. *Altar of the Luonnotar*: only once the Exalted altar (six Great Prophet
  stages) stands, the final altar's research and hammers.
- Religious, score (no target score) and time victories are not pursued.

**Plan** (`plan`, `plan_ours`): from turn 200 (scaled to the game's length; from 100 when the team
holds 1.5x the next team's share of the world) the condition with the fewest turns that fits
before the time limit; a cultural victory only for the front-runner (no other civ's estimate
under 4/5 of ours: only one civ wins it, the others' culture is research lost). A *rival* is
another civ whose own best estimate is within 60 turns and shorter than ours.

**Pursuit** (each hook tagged `victory-pursuit`):
- research: the goal's techs (the towers', Drama, the final altar's) count double plus 4000 and
  are charged at most 15 turns of research (a beeline);
- production (`victory::production`, after the garrison, the first army and the workers; before
  expansion when the goal is 30 turns away): the towers in the best cities (the Tower of Mastery
  in the best one; the rules let one city build each) and a caster when the towers' mana nodes
  need one; for a cultural victory, in the three cities nearest legendary, culture buildings and
  wonders by culture per turn of production, else `PROCESS_CULTURE`; the final altar where the
  Exalted altar stands;
- culture slider (`empire::sliders`): the lowest of 40/60/80/100% that brings the cities to
  legendary within 100 turns (and 10 turns before the time limit), else 100%, lowered in steps
  only when the treasury runs short. The finance logic reads research and culture together
  (`economy::spend_pct`): counting research alone, a civ going for culture took itself for
  bankrupt and disbanded its army;
- a Great Bard (great work, 4000 culture) goes to the one of those cities furthest from
  legendary; casters build the mana nodes the towers lack;
- defense (`victory::guard`): once the goal is within 90 turns (the others will come for it), the
  cities that carry it keep two more defenders;
- wars (`diplomacy::victory_war`): against a rival about to win (defensive power below 1.5x ours,
  reachable over land within 20 plots, a city we can take), and for a domination or conquest
  goal against the weakest civ we can reach, prepared without the leader's roll or grudge; the
  rival's victory cities (those nearest legendary, those with towers) are worth 3000 more to the
  siege campaign; these wars end only when lost (or after 80 turns).

Measured on full games (seeds 1-24, 6 civs, standard map, normal speed, to the rules' end;
`victory_check.py`); economy per alive civ, at turns 400 / 500 (one game was over by 500):

| | before | after |
|---|---|---|
| games decided before the time limit | 5 / 24 (21%) | 19 / 24 (79%) |
| by victory | cultural 3, domination 2 | cultural 18, domination 1 |
| decision turn (decided games) | 651 [608-679] | 603 [494-684] |
| games still on at turn 600 | 24 | 16 |
| legacy score | 1361 / 1911 | 1352 / 1877 |
| techs | 37.9 / 51.4 | 37.4 / 50.7 |
| cities / population | 7.2 / 8.0, 55.3 / 77.2 | 7.2 / 8.0, 55.3 / 75.9 |
| research (beakers/turn) | 158 / 249 | 145 / 197 (the culture slider: 70% -> 61% research) |
| civs eliminated; wars declared; civ cities taken by civs | 48; 175; 497 | 45; 175; 534 |

300-turn games (seeds 1-24) are unchanged but one (23 identical checksums: the planner rarely
acts before turn 300); CPU time per 300-turn game 13.9 -> 14.1 s (seeds 1, 2, 3, 5; about 47 ms
per turn, measured alternately on a shared machine).

How it got there (same 24 full games; decided / cultural + domination / mean decision turn):

| version | decided | by victory | turn |
|---|---|---|---|
| no planner | 5 | 3 + 2 | 651 |
| planner; culture slider the lowest that finishes before the time limit | 19 | 17 + 2 | 660 |
| + front-runner only, slider for a win within 100 turns, great works | 17 | 16 + 1 | 607 |
| + finance reads research and culture together, guard (kept) | 19 | 18 + 1 | 603 |
| kept, without the front-runner rule | 19 | 18 + 1 | 596 |
| kept, without the guard | 18 | 17 + 1 | 604 |
| kept, without the victory wars | 21 | 21 + 0 | 597 |

- The first version won at the deadline (turn ~680): the lowest slider that finishes in time.
- Pushing harder made the culture front-runner an easy target: a culture slider of 60%+ left
  research under 40%, which the finance logic took for bankruptcy (garrisons capped, field units
  disbanded, no new army), and the other civs' rival wars took its culture cities (seeds 14, 23,
  24). Reading research and culture together and guarding the victory cities fixed it.
- The front-runner rule is neutral on decisions (19 either way, 7 turns later with it) and keeps
  research: 21 instead of 26 civs on culture at turn 500, research +3%.
- The victory wars cost two decisions (seeds 9, 10: the culture front-runner was stopped, as
  intended) and turn one cultural game into a domination: 175 wars instead of 157, civ cities
  taken by civs 534 instead of 458. Kept: stopping a rival is the point.
- Before FfH's AND reading of `PrereqBonuses` was found, the Tower of Mastery looked reachable:
  the Balseraphs of seed 3 beelined every tower tech by turn 363 and then could build no tower
  for want of most of the 16 manas. The estimate now requires the manas, so no civ beelines
  the arcane techs for nothing.

## Cost control

- **Reachability**: `reach.rs` floods once from a unit (Dijkstra on movement points, bounded
  by a turn budget, unrevealed plots assumed passable) and scores every reached plot; only the
  chosen target gets a real path through the rules' `MISSION_MOVE_TO`.
- **City sites** (`sites.rs`): scored at most once per `update()` per player, only within 7
  plots of our cities (overseas: coastal plots within 14 of our coastal cities, only when
  the home landmasses are full), greedily picked with city spacing.
- Targets, peril and the road plan are computed once per turn per player.
- Measured with `ffh stats` (6 civs, standard map, 300 turns, 20 seeds): about 24 ms per
  turn on average (four games in parallel on four cores), two thirds of it inside the
  controller (including the moves it orders).

## Tools

- `ffh stats --seed N --at T1,T2,.. [--every N]` plays one game and prints one JSON line:
  per-civ snapshots after the given numbers of turns played (and always at the end), game-wide
  values at the same turns, whole-game events (captures by and from barbarians, wars, peace,
  eliminations), the outcome and the turn each civ died. Snapshots only read the game (the sync
  checksum is the same with or without them). Fields of a snapshot row (one per ever-alive
  player, barbarians included; all computed from game state at the end of that turn):
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
  `religions` (founded so far), `land_plots`, `estimate_end_turn`, and the legacy score's
  `max_*`/`init_*` (population, land, tech, wonders). Players can appear mid-game (the Mercurians join the team of the civ that built the
  Mercurian Gate; the Infernals and empires formed by events get a new team), so aggregate by
  `team`.
  - victory progress (alive civs): `towers` (Tower of Mastery classes owned), `tower_eta`,
    `culture_eta`, `domination_eta`, `conquest_eta` (the planner's estimates, null when out of
    reach), `legend3_pm` (the third-best city's culture, per mille of legendary), `altar_stage`,
    `mana_raw` (raw mana nodes in our cities' radius), `mana_types`, `culture_pct` (culture
    slider), `goal` and `goal_eta` (the planner's pick), `rival` (the player it would stop).
  `crates/ffh-cli/scripts/ai_metrics.py --seeds 1-12 [--every 10] [--out runs.json] [--compare
  old.json]` runs it over many seeds in parallel and summarizes (with the planner's goals and the
  victories before the time limit). With the agents' cache present it replays games already played
  from the shared run cache and takes a machine-wide CPU slot per game; `tools/agents/ab.sh` runs
  it on a base and on your worktree (docs/AGENTS.md).
- `crates/ffh-cli/scripts/victory_check.py --seeds 1-24 [--out games.json | --load games.json]
  [--compare base.json] [--min-share 0.5]` plays full games (to turn 690 or an earlier
  victory) and reports the share decided before the time limit, per victory type, the decision
  turn, and the economy at turns 300-600; `--min-share` makes it a pass/fail check. A short
  version is the ignored test `run::tests::full_games_end_in_victories` (`cargo test --release
  -p ffh-cli -- --ignored`).
- `ffh autoplay --report T` prints, per civ, costs, unit roles, improvements, civics and every
  city's yields, trade, happiness and production; `ffh inspect <player>` shows each city's
  plots (worked, yields, improvements).
- `FFH_AI_TRACE=all|<player>` traces AI decisions to stderr; `FFH_LOG=all|<player>` prints the
  game log.
- `--option GAMEOPTION_X` (repeatable) turns game options on for any of these commands (for
  example `GAMEOPTION_ADVANCED_START`).

## Known gaps (future work)

- Naval: amphibious invasions now ship an army across water to take an overseas enemy city
  (see Naval), but there is no naval combat, no escorts and no blockades (the transport sails
  undefended), and one trip lands only the ship's cargo capacity, so invasions are small until
  good transports arrive late in the tech tree. Overseas settling still sends one settler at a time.
  The trickle no longer scatters and no longer under-ships: each trip lands on the reachable plot
  already holding the most of our army (they pile onto one plot and strike), and the staging port
  builds a fleet big enough to carry the force in one wave. Traced water-isolated civs that used to
  land repeatedly without ever assaulting now take overseas cities; 4 of 5 such civs convert and
  held overseas cities rose ~20% at a fixed turn (8-game A/B), economy flat.
  The remaining hard case is a civ whose *main army is on a different landmass from the staging
  port* -- an isolated civ (e.g. Doviello, seed 5) whose only target sits beyond its home coast's
  sea-range, reachable only from a small forward colony that cannot build transports and holds
  little army. `committed` only musters army already on the staging landmass, so the home army
  (3000+ idle stack-turns) never joins. Two simple fixes were tried and both measured worse, so
  both were reverted: (a) letting *any* coastal city build the assault transports (so the fleet is
  built and the colony's few units cross) made civs start more overseas wars they still cannot
  convert -- captures -8/-9%, economy -7% at t690; (b) staging from the port on the landmass that
  holds the most of our army (ship the home army directly, however far) got the home army aboard
  (boards 1->11) but the home->target journey was so long the transports sailed all game and
  delivered almost nothing (333 sail-actions, 3 landings, 0 assaults) -- the target is simply too
  far from home for direct shipping, which is why the forward colony exists. The real fix is the
  original DLL's approach: classify the home landmass `AREAAI_ASSAULT_MASSING`, commit the home
  army, and ferry it in *two hops* (home -> forward colony, then a short colony -> target assault),
  massing at the colony before striking, sized so the landed force can take the city and gated so
  it does not bankrupt the economy. That is a multi-hop logistics pipeline (a new transport mode
  plus cross-landmass `committed`), a dedicated feature rather than a staging tweak.
- War conversion vs. decisive wars: civs with a real army but no reachable *takeable* target sit
  idle (traced `big army / took=0` civs, e.g. Sheaim seed 1: ~71 power of army, exactly one war
  prepared all game, 2283 unit-turns "0 targets"). The dominant land-war gate is `can_take_cities`
  (diplomacy.rs): it clears a war only when our best *trainable* attacker reaches the enemy's
  weakest city defender x1.2 (the fortification margin). Tracing the greedy war loop across seeds
  1-3 (all civs) found this gate rejects ~700 land-reachable (<=16 plots), already-power-parity
  targets -- ~8x the wars that pass -- and none of those rejections are catastrophic per-unit gaps
  (worst ~0.6x; 44% are cases where our unit already *beats* their defender, blocked only by the
  x1.2 margin). Hypothesis: the x1.2 "solo the fortress" bar is wrong because a war is declared
  only after massing `army_needed_for_war` (4-14) attackers, so a stack should take a fortified
  city at per-unit parity. Dropping the no-siege multiplier 120->100 (attacker >= defender) was
  measured (16-seed A/B, full 690-turn games): wars_declared +47%, but net territory flat (final
  cities 838->816), biggest empire flat (avg 22.8->22.1), economy 2-8% behind at *every* fixed
  checkpoint t200/t300/t450, and the +16% final "power" was standing military that never converted
  (NEW games ran *longer*, 6 vs 4 reaching t690 -- grinding, not decisive). Reverted: the per-unit
  bar turns out to select for *decisive* wars; the near-parity fights it filters out grind and
  mutually destroy power (seed 3: the relaxed gate flipped the winner and left the victor with half
  the base winner's power), an arms race rather than domination -- and over-militarization is a BTS
  pathology this AI otherwise avoids.
  A second attempt (v2) built the "decisive-force" gate that reasoning implied: SELECTION loosened
  to per-unit parity PLUS a numbers requirement (the force we will field overwhelms the softest
  revealed city's defender *count* with a reserve, via the campaign's own `rough_need`/`margin`),
  and a new DECLARE backstop that commits a prepared war only when the *real gathered army*
  decisively covers a visible target city -- reusing `campaign::covers`/`force_on`, the exact
  predicate the march logic uses, so a declared war is one the army would actually march on. This is
  the numbers-aware, truthfully-computed gate v1 lacked. It also measured net-negative (16-seed A/B):
  wars +15% but captures-per-war -19% (wars LESS decisive, not more), economy -8% at t300 worsening
  to -9.3% at t450 (a *growing*, sustained drag -- not one-time war-prep cost -- so an affordability
  pre-gate would not recover it), games longer (7 vs 4 reaching t690), eliminations +30%. The only
  gain was domination *concentration* (biggest empire +10%, top-power share +14%), but it came from
  a more war-torn world that suppressed everyone's research. Reverted.
  Conclusion from both attempts: the war-DECLARATION gate is not the lever. Wars are dynamic -- the
  enemy mobilizes and reinforces once war is declared -- so any declaration-time snapshot (per-unit,
  numbers, or real-army-covers-the-opening) cannot guarantee the *whole* war is decisive; the
  opening advantage erodes into a grind. The baseline's standing gross-power-ratio + per-unit bar is
  a near-optimal heuristic for a declaration gate precisely because it demands a *persistent* edge.
  If this pathology is pursued again, the lever is the campaign/prosecution layer (sustain local
  force superiority through a war, concentrate faster, pick targets that stay takeable) or the
  one-war-at-a-time constraint (diplomacy.rs) -- not the decision to declare.
- Sparse worlds (2 civs on a standard map): barbarians and their cities dominate the empty
  land; expansion is slow because settlers wait until no enemy is near their city. Letting
  them go out under a held threat founded more cities but lost many more to the barbarians
  (measured), so the gate stays.
- Expansion is slow everywhere in the first 100 turns (about two cities; second city at
  turn 70, third at 110): settlers cost 220 hammers on normal speed (120 + the new-city
  production value) and cities wait for size 3-4 and for the barbarians to leave. Faster
  openings were measured and cost research by turn 300 (see "Settlers and escorts").
- Economy: research at about two thirds of the commerce by turn 300 (145 beakers a turn, 29
  techs); maintenance and civic upkeep take a quarter of the commerce. The first 100 turns are
  slow whatever the research order (6-7 techs at turn 100 with two cities).
  The civs still stuck at low research expanded to 7-8 cities in Despotism before reaching
  Cartography: only the settler gate (`finance::city_affordable`, not wired in) prevents that,
  and gating expansion costs cities (no better score by turn 300).
  A diagnosis (instrumented Finance + per-city builds) found research is cost-limited, not
  slider-limited (`empire::sliders` already maxes research subject to solvency), and the dominant
  cost is unit support (35-48% of costs at t300), while research-multiplier buildings are starved:
  at war + army-short the production ladder returned a military unit every turn and never reached
  the building step, so libraries were built in ~1 of 6-8 cities and beakers = commerce x slider
  with no multiplier. Fix (`production.rs` decide(), the "economy-before-the-army-tail" gate): a
  high-value economy building (identified data-drivenly by its gold/research commerce + specialist/
  GP/trade value, `is_economy_building`) now preempts the surplus-army *interleave* -- but only for
  a civ whose finances are actually strained by upkeep (`Finance::support_heavy` or `structural`),
  so the research-suppressed laggards get relief while healthy conquerors keep their army. It is
  strictly downstream of `army_choice` (the operation's real needed force) and every garrison, so
  only the non-converting attacker tail is deferred; nothing is disbanded. Measured (16-seed A/B):
  research@t300 +4%, final power +12%, biggest empire +3% and top-power share +10% (domination up),
  net territory and eliminations flat. A broader variant (gate on any economy building, no finance
  condition) gave more research (+12%) but cut captures ~25% and shrank the top empire -- it
  dampened healthy conquerors, so the upkeep-strained condition was kept.
- Sieges: one operation at a time; armies are small (at war the operation spends half its turns
  gathering, typically 2-3 units against a need of 4-6), so a barbarian town of 5+ archers is
  out of reach and is left alone; casters do not join the strike (they only follow the army); a
  city emptied of defenders is entered even when a counterattack will retake it. Tried and
  dropped (no measurable effect on seeds 1-48): raiding (pillaging) while a war operation
  gathers, a "can we hold it" check before the strike, every unit following the victor into the
  city, stacks marching on a barbarian town turning back for cities in peril (fewer towns
  taken, no fewer cities lost), barbarian targets out to 20 plots, a larger strike margin.
  One prosecution defect WAS fixed: the Strike gate fired off the adjacent ring alone
  (`ok_adj = covers(adj, spare=false)`) while a committed stack one plot out (dist 2) was not yet
  in the fight (`strike()` pools only units a single plot from the city), so the adjacent force
  struck at acceptable odds and bounced, then re-mustered (16 "steps back" for one civ in seed 3).
  `build_op` now defers a MARGINAL strike one turn (Strike->StepIn) when a committed dist-2 stack
  can reach an adjacent plot that turn and the combined force would then take the city with a
  reserve -- strictly one-turn-bounded (a stack that cannot close now is not counted, so a
  hardening city is still struck at acceptable odds), and it only ever downgrades Strike to StepIn
  so it can never make a strike weaker. Measured (16-seed A/B): "steps back" ~23->5 and razed
  cities -29% (captures kept intact, not destroyed), final power +4.6%, eliminations -5.7%,
  research flat; but gross captures-per-war was flat (-1.9%), so this is a coherence/retention
  improvement (the army stops striking piecemeal and keeps more of what it takes), NOT a
  conversion win -- the outcome deltas other than razing are within 16-seed noise.
- Loose stacks exposed to a stronger enemy (the classic Civ4 "a stack parked in front of
  something that one-shots it"). Measured with a gated diagnostic (`FFH_AI_EXPOSURE`, which flags
  army stacks standing where a single visible enemy can reach and out-fight their best defender
  next turn): 230-450 *distinct* such stacks a game (4 games x 690t), ~90% with enough attackers
  to wipe the stack, a tail at 3-7x strength, concentrated in the mid/late war era (t250+), not
  early-game barbarian noise. It is entirely *loose* stacks -- stacks in an operation are never
  exposed, because `campaign.rs` keeps them concentrated -- so the gross cases (a lone unit next
  to a death stack, attacking into a stronger defender) are already handled by the odds-gated
  attack logic and the idle-retreat-to-city fallback; this is the residue. It looks bad but is
  not costing the AI: making an outclassed loose stack fall back to the safest plot it can reach
  (a city, or a plot where it is not outclassed with terrain credited), strictly after every
  attack/garrison/heal/city-defence/escort/operation/hunt step so offence is untouched, measured
  clearly NET-NEGATIVE on the primary goal (16-seed, 690t A/B): final power -8.1%, biggest empire
  -9.2%, top-power share -10.8%, techs@450 -7.3%, games longer (5 vs 3 reaching t690, i.e.
  grinding), for only +4% captures per war; and it barely moved the exposure itself (-18% over
  four games, worse in two of them -- the stack retreats toward a safe haven it cannot reach in
  one turn, so it ends the turn on another exposed plot and churns). The exposed stacks apparently
  do useful work -- pressure, map presence, attrition -- that retreating removes, so pulling them
  back makes the AI timid, the same over-caution the war-declaration and strike-hold experiments
  hit. Reverted; the `FFH_AI_EXPOSURE` counter is kept as observation tooling. A fix that helped
  would have to avoid *creating* the exposure -- danger-aware movement that ends each turn on a
  safe plot -- without making stacks passive, which is a pathfinding change, not a retreat rule.
- No hurrying beyond the civic-gated gold/population hurries, no wonder racing, no vassals or
  defensive pacts; the vote sources (Overcouncil / Undercouncil) use the defaults.
- Victories: nearly every decided game is a cultural victory. The Tower of Mastery needs 16
  manas and the AI neither trades resources nor conquers for mana, so the estimate is out of
  reach in every measured game; Great Prophets lightbulb instead of building the six altar
  stages; domination needs two thirds of the land and the siege operations take about one city
  per war; the religious victory (80% of the world's people) is not pursued.
- Casters use FfH's own end-of-turn spell choice; there is no planned use of summons or
  terraforming, and no dedicated logic for most civ-specific mechanics (Sheaim planar gates,
  Infernal and Mercurian projects, Khazad vaults, Armageddon rituals; see `civs.rs` for the
  ones handled).
