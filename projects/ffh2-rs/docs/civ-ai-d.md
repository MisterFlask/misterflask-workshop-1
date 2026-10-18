# Civilization-specific AI, group D: Malakim, Sheaim, Sidar, Svartalfar, Infernals, Mercurians

For the orchestrator to merge into docs/AI.md ("Civilization-specific"). Every number is from
`crates/ffh-cli/scripts/civ_ab.py`: full games (to a decision or turn 690, `--until-supremacy`),
seeds 1-40, the civ forced into slot 0, against the base AI at 2d95e9c, with the other group-D
civilizations' logic turned off on the new side (`FFH_AI_CIV_OFF`) so each result is that
civilization's own. Share = score v2 in basis points of the world (dead = 0); t = paired t over
seeds (|t| < 2 is noise); wins are compared by discordant seeds (won by one side only).

Shared pieces: `civs/world_spell.rs` (hold a world spell: `spell_value` returns 0 and the `turn`
hook casts it through `unit_cast` when the moment comes; the AI-only `req*` gates still apply), and
a `promotion_value` hook (`units::best_promotion`, marked `// civ-mechanics`). Games without the
changed civilizations are byte-identical to the base (Bannor forced, 300 turns, seeds 1-5: same
checksums).

- **Sheaim** (kept): adepts need a Mage Guild in the training city (`PrereqBuildingClass`), whose
  generic value is about 0, so no Sheaim ever had a caster although both leaders are Summoners
  and the palace gives Death, Chaos and Fire mana (census: Mage Guild in 1 of 40 games, no
  adept). Once Knowledge of the Ether is known, the most productive safe city builds a Mage Guild
  (one per four cities); at war or preparing one, guild cities train adepts up to one per two
  cities (at most 6); Sheaim casters take Death first (Raise Skeleton, Spectre), then Channeling 2
  and Fire. The generic caster role and end-of-turn summons do the rest. Win 2.5% -> 2.5% (1 seed
  each way), share at the end 912 -> 1007 (t 0.79), legacy score at the end 795 -> 972 (t 1.37),
  alive at the end 50% -> 60%; at turn 300 flat (1310 -> 1304, t -0.19: the guild comes around
  turn 200-250). Mage Guilds 0.0 -> 2.8 per game; casters in 19 of 40 games, with skeletons,
  spectres, mages and Eaters of Dreams in the snapshots from turn 250.
  Tried, not kept: the Planar Gate valued by the creatures its city can call (100 per eligible
  class, 2+ classes, one gate, while a unit is affordable), on top of the above: built in a few
  games (0.1 per game), share at the end -1 (t -0.15) -- too rarely eligible to matter.
  Not tried: the Elegy of the Sheaim and Worldbreak timing (the AI-only Worldbreak gate, counter
  50, is reached in about 3% of games, near the end).
- **Sidar** (kept): Into the Mist gives every unit Hidden (invisible outside our cities, +1 first
  strike) until it fights or casts; units trained later do not get it. The generic spell choice
  cast it the first turn the AI-only gate (40 units) opened, often at peace, where the army lost
  it to barbarians before any war. The AI holds it and casts it when at war with a civilization or
  preparing one (seed 4: turn 298 at peace -> 327 at war), or at 60 units with no war. Win 5.0% ->
  7.5% (1 seed won only by the new side, 0 by the base), share at the end 1333 -> 1407 (t 0.94),
  legacy at the end 1395 -> 1520 (t 1.13), share at turn 300 1531 -> 1529 (t -0.41: the spell
  mostly falls after turn 300), cast in 22 -> 22 games. Small and within noise; every end measure
  is up and the win rate does not drop.
- **Malakim** (nothing kept). Tried: Religious Fervor (a priest in every city) held for a war, or
  until 10 cities have the religion: win 30.0% -> 22.5% (0 seeds won only by the new side, 3 by
  the base), share at the end 1642 -> 1541 (t -0.66), cast in 22 -> 22 games, later. The Citadel of
  Light (burns the strongest enemy plot within 2 every turn) valued at 250 in frontier cities at
  war: every game byte-identical to the base -- the Malakim AI never researches Sorcery, so it is
  never buildable.
- **Svartalfar** (nothing kept). Tried: never casting Veil of Night (Hidden Nationality for every
  unit: cannot capture cities, hostile to everyone; the diagnosis expected it to cost the army its
  conquests): win 20.0% -> 10.0% (0 seeds won only by the new side, 4 by the base), share at the end
  1513 -> 1358 (t -0.99), legacy at the end 1598 -> 1342 (t -1.40). The generic cast at the gate
  (25 units, at peace) is worth more than keeping the army able to capture.
- **Infernals** (kept; they join when the first team learns Infernal Pact). Fallow: no food, cities
  never grow, so population comes only from Manes (Add to City), which FfH's spell value (-250)
  never cast. Manes in one of our cities with another defender there now add themselves; lone
  Manes walk to the nearest city. Over the 6 of 120 full games (seeds 1-40 with Malakim,
  Svartalfar and Sidar forced) where the Infernals appear on both sides: population at the end
  29.5 -> 64.5 (t 2.72), cities 4.0 -> 5.7 (t 1.42), share 1056 -> 1557 (t 2.21), Manes left at the
  end 12 -> 0; the Infernal team won 1 -> 0 of those games. Six games only.
- **Mercurians** (kept; they join the builder's team when the Mercurian Gate is built). Their
  starting settler had no site (their city stands in the builder's culture) and waited all game
  paying support; with no site on its landmass 20 turns after it appeared it is disbanded. Over
  the 76 of 120 games where they appear on both sides: settlers left at the end 68 -> 9, share
  351 -> 385 (t 1.21), cities 2.29 -> 2.47, their team won 28 -> 31. The forced civs of those games
  are unchanged to turn 300 and within noise at the end (Malakim +34, t 0.49; Svartalfar -14,
  t -0.39; Sidar +70, t 1.16).

## World spells: the moment, if the AI-only gate were open

- **Religious Fervor** (Malakim; gate: 5 cities with the state religion): measured: casting at the
  gate beats holding it. With no gate I would cast it once the religion is in most cities and the
  civ is at war or about to be; worth little, the priests are a few hundred hammers of support.
- **Into the Mist** (Sidar; gate: 40 units): at the start of a war, so the army marches unseen.
  The gate rarely matters: by 40 units most Sidar are already at war or near it.
- **Veil of Night** (Svartalfar; gate: 25 units and at peace with every civ): measured: casting at
  the gate is good. The peace condition is a real constraint (it forbids casting mid-war); with it
  open I would still cast early, as soon as the army is built.
- **Worldbreak** (Sheaim; gate: Armageddon counter 50, reached in ~3% of games and only after
  turn 550): with the gate open, cast the turn a campaign strikes a city (from the `turn` hook,
  before units move) once the counter is 20+; this could matter a lot (half-health defenders in
  every enemy city) but the AI gate makes it moot.
- **Divine Retribution** (Mercurians; gate: at war with 2+ civ teams): cast only when the enemy's
  demons and undead outweigh our team's and neutrals' (it hits everyone); small. Not implemented.
- **Hyborem's Whisper** (Infernals; needs Malevolent Designs, never reached in these games).

## For the orchestrator

- Two of the diagnosis's highest-ranked "do not cast / hold" proposals (Veil of Night, Religious
  Fervor) measured worse: a world spell's value from the first turn it is allowed may beat a
  better-timed cast, because a cast at the gate keeps its effect for the rest of the game.
- Cross-cutting (not fixed generically): no AI builds a Mage Guild (done for the Sheaim only, with
  a visible gain); Malakim never reach Sorcery (Citadel of Light, Sand Lions' adepts).
- Rules: none found. Wane and Kidnap (Sidar, Svartalfar) lack `bAllowAI`, so the AI can never cast
  them; opening them is a rules decision.
