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
1. **Empire** (`empire.rs`): research when the queue is empty; civics every 10 turns when
   the gain beats the anarchy; state religion; the highest research slider that keeps the
   treasury from running dry; a bankrupt empire (slider at the bottom, still losing gold)
   scraps surplus garrison units, never below what production wants in that city; an empire
   paying unit support with research at a fifth or less disbands its weakest field units
   (never heroes) down to a mobile force of 1 + cities/4 while at peace.
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
   - **settlers** go to the best reachable site (`sites.rs`), re-planned every turn (and
     never left asleep); a site under danger needs an escort, which the settler picks up (a
     field unit or a spare garrison unit) or waits for in its city; the first city is founded
     in place unless a clearly better site is reachable this turn; when no safe path exists
     (it crosses a neighbour's land) an escorted settler takes the direct one; with no site
     left on its landmass it goes overseas (see Naval);
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
   that explore and escort);
6. FfH tier-2 units need a building in the training city (Archery Range for archers,
   Training Yard for axemen, ...): when one would open a defender or attacker a third
   stronger, about half the cities build it;
7. extra defenders up to the quota (capital, threat, nearby barbarians, war, leader);
8. a small mobile force (1 + cities/2) for barbarians and animals; more workers as worked
   plots need improving; heroes (not at the cost of the first expansion);
9. the army target (larger for military-minded leaders, doubled at war or while preparing
   one; at war the attackers are chosen for strength against cities -- city attack bonus,
   first strikes, collateral -- not mobility), a third of it siege engines at war, and a
   couple of siege engines whenever a stack is camped at a city it cannot take; in peacetime
   army and buildings alternate;
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
been pushed down to pay the bills; a culture-starved new city values culture.

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

## Field armies (`military.rs`)

Each army group is a stack; per decision, in order:

1. follow through a group attack the rules queued (`ai_group_attack`);
2. become the garrison of an undefended city of ours it stands in;
3. merge the army units on its plot (up to 8; only units with full moves);
4. attack an adjacent target worth it (never a civ city outside a war plan);
5. heal when hurt and safe;
6. defend a city of ours in peril within 5 turns;
7. escort a settler that asks for one (single units);
8. war: march on the target city (preparing: gather on the rally city -- the city nearest the
   target -- without crossing the border; too small: gather there first), stage on the best
   defensive plot next to it, bombard with siege engines, assault when the stack estimate
   allows (at a city we mean to take, a stack that with our other stacks around the city
   has three attackers per defender attacks even at poor odds: each lost fight still takes
   about a third of a defender's health and defenders do not heal between the fights of a
   turn), fall back to wait for siege engines when hopeless;
9. hunt the best barbarian/animal/lair target within 6 turns it can take (entering an empty
   lair destroys it);
10. peacetime campaign: the nearest known barbarian city on our landmass (within 20 plots),
    the same way as a war target;
11. join a bigger stack within 3 turns, else hold in the nearest city.

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
- Expansion is slow everywhere in the first 100 turns (about two cities): settlers cost 220
  hammers on normal speed and cities wait for size 3-4 and for the barbarians to leave.
- Economy: research at about half the commerce by turn 300; cottages need Education,
  libraries Writing; maintenance and civic upkeep take a third of the commerce. Gating
  expansion on the research it leaves was tried (fewer cities, no better score by turn 300).
- Wars rarely take civ cities: tier-1/2 attackers against fortified defenders in cities need
  siege engines and casters; stacks act one at a time (a grinding assault counts the other
  stacks around the city but does not wait for them).
- No hurrying beyond the civic-gated gold/population hurries, no wonder racing, no vassals or
  defensive pacts; the vote sources (Overcouncil / Undercouncil) use the defaults.
- Casters use FfH's own end-of-turn spell choice; there is no planned use of summons or
  terraforming, and no dedicated logic for civ-specific mechanics (Sheaim planar gates,
  Infernal and Mercurian projects, Kuriotates settlements, Armageddon rituals).
