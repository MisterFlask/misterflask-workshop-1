# AI design (ffh-ai)

The C++ AI (CvPlayerAI/CvCityAI/CvUnitAI/CvTeamAI, ~60k lines, plus Python
`AI_*` hooks) is not ported. It is the main cause of FFH2's late-game turn times
(per-unit re-evaluation of every plot, repeated A* searches, Python spell checks).
`ffh-ai` is a new design with two hard requirements: **legal play only** (it acts
through the same rules APIs a human would) and **bounded cost per turn**.

## Shape

`Ai` implements `ffh_core::Controller`. `play_turn(g, p)` runs a fixed pipeline:

1. **Analyze** (read-only, once per turn): visible threats per plot, own military
   strength per area, city needs (defense, happiness, health, growth), candidate
   city sites (cached, refreshed every few turns), war targets.
2. **Empire**: research choice, civics, state religion, commerce sliders.
3. **Cities**: production queues from a scored candidate list (settlers/workers while
   expanding, minimum garrison, military when threatened or at war, buildings by
   marginal yield value, wonders when safe), hurry when cheap and useful.
4. **Units**, by role (stored in `Unit::ai_type`):
   settlers -> best reachable site; workers -> highest-value improvement/route in
   their cities' radius; defenders -> garrison quotas; attack stacks -> gather, then
   strike the best target by odds-weighted value; explorers; spellcasters -> cast the
   best available spell (buff/summon/damage) by a per-spell value function; heroes
   and great people -> settle, build, or golden age; naval units.
5. **Diplomacy**: declare war when clearly stronger than a reachable neighbour with
   poor attitude; seek peace when losing or war-weary; open borders with friends.

## Cost control

- Pathfinding is bounded (max turns) and cached per group per turn; target scans use
  one Dijkstra flood from each stack rather than A* per candidate.
- Per-turn analysis is O(plots + units); no per-unit full-map scans.
- Independent analysis (e.g. site scoring, threat maps) can run on worker threads in
  the CLI; the simulation itself stays single-threaded and deterministic.

## Determinism

AI randomness uses the game RNG (as the C++ AI does) or values derived from it, so
a seed fully determines a game including AI choices.
