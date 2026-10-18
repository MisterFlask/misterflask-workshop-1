# Civilization-specific AI, group B: Doviello, Elohim, Grigori, Hippus, Illians

Sections in the style of docs/AI.md "Civilization-specific", for the orchestrator to merge.
Every number is from `crates/ffh-cli/scripts/civ_ab.py`: the civ forced into slot 0, seeds 1-40,
6 civs on a standard map, base = 2d95e9c. Unless stated otherwise the games are full games (to a
decision or turn 690, `--until-supremacy`), and the other group-B civs' logic is off on the new
side (`FFH_AI_CIV_OFF`), so the difference is that civ's own play. "share" is score v2 in basis
points (dead = 0); "wins" gives the discordant seeds (won only by new / only by base); t is the
paired t over seeds. "share@T carried" carries a finished game's last snapshot forward, so games
that end at different turns compare at the same turn (share@end compares a civ that won at turn
400 with the same civ at turn 600 of a game another civ won).

Held world spells share `civs/held.rs`: the civ's `spell_value` returns 0 for the spell, and its
`turn` hook casts it through `unit_can_cast` / `unit_cast` when its condition holds, so the
ported Python's AI-only gates still apply.

<!-- HIPPUS -->

<!-- DOVIELLO -->

- **Illians**: The White Hand (Philosophy, 400 hammers: three Priests of Winter, strength 5 + 2
  cold, Medic, Channeling II, Ice) and Stir from Slumber (Drifa, a dragon; allowed once a
  player has been eliminated) are built by the most productive city as soon as it can (not
  while threatened or broke). The generic AI reaches projects only once no building has value
  left (`best_project`), so The White Hand came very late or never; Stir from Slumber is on the
  generic exclusion list. The White Hand per game at the end 0.0 -> 0.7 (Stir from Slumber was
  never built: by the time a player is eliminated the Illians rarely have Divine Essence).
  Wins 12.5% -> 17.5% (2/0), share@300 -0.5 (t -0.05; the projects come after turn 300 in most
  games), share@end 1460 -> 1728 (+269, t 1.90), rank 3.9 -> 3.5 (t -2.45), legacy@end +259
  (t 1.63). Stasis (world spell: 20 turns without production or research for every other
  player, barbarians included) stays with the generic end-of-turn cast, the turn its AI gate
  (5 cities) opens (turn 130-280 in the base games).
  Tried, not kept: holding Stasis for the moment it hurts most, i.e. our war operation on the
  march (the target trains no defender), a city in peril, a rival within 40 turns of victory,
  else 40 turns after the fifth city: share@200 -48 (t -1.4), share@300 -13 (t -0.3), share@end
  +198 (t 1.29), wins 2/1 (300-turn development run: share@300 -48). Holding it only while a war
  is under way or being prepared, else casting at the gate: share@300 -36 (t -0.9), share@400
  -97 (t -1.1), share@end +100 (t 0.7), wins 1/0. The setback is worth most early: a 20-turn
  freeze costs the rivals 20 turns of growth whenever it falls, and early growth compounds;
  holding it for a rival's victory only shifts that rival by the same 20 turns later. In the
  base games the gate opens 50-130 turns before the Illians' first war, so a war trigger means
  a long wait.
- **Elohim** (nothing kept): Sanctuary (30 turns in which no foreign unit can enter our land,
  foreign units thrown out; AI gate: at war with two civs) was held until enemy civ units stood
  in our land within 3 plots of a city or a city was in peril (fallback: 15 turns of two wars).
  The gate opens in few games: 4 of 40 games changed; share@300 +4.5, share@end -78 (t -1.1),
  wins 0/0. Corlindale (a combat-0 caster hero no role trains) and Hallowing of the Elohim
  (-5 Armageddon counter, 600 hammers) were judged not worth their hammers and not tried.
- **Grigori** (nothing kept): Grigori Medics (Medicine, Infirmary; Medic I-III) were trained at
  war, one per 4 attackers (at most 3), with an Infirmary in the most productive city if none
  existed: 0.9 medics a game at the end, Infirmaries 0.1 -> 0.6; share@300 -1 (t -1.3),
  share@end +31 (t 0.3), wins 0/1. Ardor (great-person threshold back to its base; AI gate: 5
  great people) is cast by the generic choice at the gate. The first great people of a Grigori
  game are mostly Adventurers (the Palace's points), each one raising the threshold; with N
  great people a game and one reset, the cost is smallest with the reset at N/2, so the gate is
  close to right for the 8-12 a Grigori game makes. Adventurers become ordinary army units and
  are upgraded by the generic code (Warrior, Axeman, Scout); keeping them as high-level casters
  was not tried (no AI trains or plays adepts well yet: cross-cutting finding 2).
