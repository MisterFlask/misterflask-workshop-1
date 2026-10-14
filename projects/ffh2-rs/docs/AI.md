# AI design (ffh-ai)

The C++ AI (CvPlayerAI/CvCityAI/CvUnitAI/CvTeamAI, ~60k lines, plus the Python AI hooks
in CvGameUtils.py such as `AI_chooseTech`/`AI_chooseProduction`) is not ported. It is the
main cause of FFH2's late-game turn times (per-unit re-evaluation of every plot, repeated A*
searches, Python callbacks). `ffh-ai` is a new design with three hard requirements:

- **Legal play only.** It acts through the same rules APIs a human would: missions pushed on
  selection groups (`group_push_mission`), city orders (`city_push_order`), research
  (`player_push_research`), civics/religion/sliders, deals (`implement_deal`), spells
  (`unit_cast`). The rules decide what is allowed.
- **Bounded cost per turn.** No per-unit full-map scans; at most one bounded flood per group.
- **Deterministic.** Decisions are pure functions of the game state. The AI never draws from
  the game RNG, so it cannot perturb the rules' random stream; its caches are recomputed when
  the game turn changes, so save/load would not change its choices.

## Where the rules call the AI

`Ai` implements `ffh_core::Controller`; the rules call it where the C++ calls the AI:

| C++ | Controller | Module |
|---|---|---|
| `CvPlayerAI::AI_unitUpdate` (from `CvGame::updateMoves`) | `play_turn` | `lib.rs` |
| `CvCityAI::AI_chooseProduction` (from `CvCity::doProduction`) | `choose_production` | `production.rs` |
| `CvPlayerAI::AI_chooseResearch` / `AI_chooseFreeTech` | `choose_research` | `research.rs` |
| `CvPlayerAI::AI_conquerCity` | `raze_conquered_city` | `diplomacy.rs` |
| `CvPlayerAI::AI_considerOffer` | `consider_offer` | `diplomacy.rs` |
| `CvPlayerAI::AI_chooseEvent`, elections, votes, captured/gifted units | defaults in `controller.rs` | -- |
| `CvCityAI::AI_assignWorkingPlots` (citizens) | `Game::city_assign_working_plots` | `ffh-core/src/governor.rs` |

`CvPlayerAI::AI_foundValue` (and its bonus valuation) is ported faithfully into the rules
(`ffh-core/src/found_value.rs`) because starting plots and barbarian cities depend on it; the AI
reuses it to score city sites.

## A turn (`play_turn`)

1. **View** (`view.rs`): teams at war with us, and a *danger map* -- for every visible enemy
   unit, its attack power added to the plots it could reach next turn. O(enemy units x
   reach^2).
2. **Empire** (`empire.rs`): research when the queue is empty (`research.rs`: value of
   unlocked units/buildings/builds/civics/religions for this civ, FfH leader hints --
   favorite early techs, favorite religion, flavors -- plus one level of lookahead, divided
   by remaining cost); civics every 10 turns when the gain beats the anarchy; state religion
   (most widespread, leader favorite, holy city); the highest research rate that keeps gold
   from running out.
3. **Diplomacy** (`diplomacy.rs`): peace when a war drags on without advantage (offered through
   a real peace-treaty deal the other AI must accept); rarely, war on a weaker, disliked
   neighbour (relations from alignment, religion, history; aggression from the leader's
   `iMaxWarRand`; a deterministic roll).
4. **Units** (`units.rs`): every group that needs orders (the rules' `force_update`, or ready to
   move) gets exactly one decision by its head unit's role (`Unit::ai_type`, set when the city
   trains it): settlers -> best reachable site (`sites.rs`), workers -> best improvement in our
   cities' radius, garrisons -> fill each city's quota, army -> good-odds attacks, then march on
   the nearest enemy city and assault when the odds are acceptable, otherwise hunt nearby
   barbarians/animals; explorers -> nearest unrevealed area and goody huts/lairs; great people
   -> discover, join as specialist or golden age in the capital; missionaries -> our cities
   without the religion; casters -> with the army or in the most threatened city; barbarians
   and animals -> attack anything they can beat, else roam. Promotions and upgrades are
   applied first. Any group still idle is skipped so the turn always ends.
5. **End-of-turn spells** (`casting.rs`): FfH's own AI casts spells at the end of every turn
   (`CvGame::updateMoves`, "BETTER AI (Spellcasting end of turn)"): `CvUnit::chooseSpell`, then
   buffs and summons. This is ported term by term, with this AI's promotion/unit/building
   values in place of the C++ AI's.

City production (`production.rs`) is a priority list: missing or overwhelmed garrison,
workers, settlers while sites remain, extra defenders, heroes, army (more at war), buildings
by marginal yield value per production turn (FfH leader favorite wonders get a bonus),
casters, missionaries, then a process. Barbarian cities use FfH's own
`CustomFunctions.BarbCityProduction`.

## Cost control

- **Reachability**: `reach.rs` floods once from a unit (Dijkstra on movement points, bounded
  by a turn budget, unrevealed plots assumed passable) and scores every reached plot; only
  the chosen target gets a real A* path through the rules' `MISSION_MOVE_TO`. The C++ AI runs
  a full path search per candidate target.
- **City sites** (`sites.rs`): scored once per turn per player, only within 7 plots of our
  cities (3 of our settlers before the first city), greedily picked with city spacing.
- Per-turn analysis is O(plots near enemies + our units).

## Known gaps (future work)

- No naval invasions or transport use; naval units hold in port.
- Wonder racing, vassals, defensive pacts, tech trading and the vote sources (Overcouncil /
  Undercouncil) use the defaults (no initiative).
- Stacks are coordinated by shared targets rather than formed into groups.
- Civ-specific mechanics with dedicated AI in FFH2 (e.g. Sheaim planar gates, Infernal and
  Mercurian projects, Kuriotates settlements) are only used to the extent the generic logic
  reaches them.
