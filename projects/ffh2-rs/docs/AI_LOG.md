# AI changes that did not move the win rate

Every AI change that was measured and did not hold -- no gain, a loss, or a gain on development
seeds that fresh seeds did not confirm -- gets an entry here, so the next attempt starts from what
the games showed instead of repeating the idea. **Read the entries for the area before changing
the AI, and add one before reverting a change** (CLAUDE.md, "AI changes"). Older failures from
before this log are recorded where they were measured in docs/AI.md (in particular "Looked
promising on development seeds, failed on fresh ones" and the "Tried, not kept" notes of each
section).

Each entry answers three questions, from the games themselves (counts with `ffh stats
--detail`, per-checkpoint paired comparisons, `FFH_AI_TRACE` replays), not from the idea:
1. **Why it made sense**: the mechanism and the evidence that it was missing.
2. **Why it did not move the win rate in the games**: what actually happened when it fired --
   how often, when, what the units or cities did, what it cost.
3. **What it implies**: the correct version of the improvement, the generic gap underneath, or a
   balance verdict when the content itself is weak in this game.

## How changes are measured (and why entries pile up)

- Development: `crates/ffh-cli/scripts/civ_ab.py` (or `ai_metrics.py` / `ab_compare.py` for
  generic changes) on development seeds, against the base, the other changes switched off.
- Validation: seeds never used to choose anything, the change on against the same binary with it
  off (`FFH_AI_CIV_OFF` for a civilization's logic). Keep only if neither the win rate nor the
  share at the end falls. Seeds 1-40 chose among variants and flatter them: in round 5, five of
  the nine civilization changes kept after development lost on fresh seeds 5001-5064. Seeds used
  for a validation are spent; the next validation takes new ones (6001+).
- A result from a handful of games is a hypothesis: the Infernal change below looked like
  +500 share over 6 games and was -29 over 60.
- Before reverting, look at the games: replay the seeds where the change fired with
  `FFH_AI_TRACE` (a throwaway trace line is fine on a scratch branch; it must not change the
  checksum) and compare the snapshots per checkpoint (`civ_ab.py --load`, the snapshot fields).

## Round 5 (2026-10-18): civilization mechanics

Numbers are full games (6 civs, standard map, to a decision or turn 690); "dev" is seeds 1-40
against base `2d95e9c`, "fresh" is seeds 5001-5064 at `d4ff7e3`, the civilization's logic on
against off; wins as "seeds won by one side only (with / without)"; share is score v2 in basis
points (fair 1667); t is the paired t over seeds.

### Casters through a Mage Guild (Amurites `5c6fdbe`, Sheaim `4f00cf1`)

The most productive safe city built a Mage Guild once Knowledge of the Ether was known and
trained casters (Amurites up to (cities + 2) / 2; Sheaim at war, one per two cities, Death
promotions first). Amurites: dev 5.0% -> 7.5% (2 / 1), share@end +108 (t 1.06); fresh 4.7% ->
1.6% (0 / 2), share@end -25. Sheaim: dev 2.5% -> 2.5% (1 / 1), share@end +95 (t 0.79); fresh
0.0% -> 1.6% (1 / 0), share@400 -144 (t -2.17), share@end -88.

1. Both civilizations are built around casters (Arcane and Summoner leaders, palace mana), and
   the base AI had none: adepts need a Mage Guild in the training city, and no AI builds one
   (no yields, `iAIWeight` 0, value 0).
2. The guilds and casters came (Amurites 1.4 guilds and ~1.4 casters a game from turn 300;
   Sheaim 2.0 guilds, 1.3-1.6 casters from turn 400) and cast a lot -- 277 to 1185 casts a game
   in six traced fresh games -- but almost only buffs, from the ported `AI_BuffCast` at the end of
   every turn: Amurite Wizards and Adepts cast Haste and Regeneration (over 95% of their casts; in
   three games no caster cast a single Fireball), Sheaim mages Dance of Blades (65-75%), with
   Fireball 10-60, Summon Spectre 23-28 and Raise Skeleton 2-9 times a game. The Amurite casters
   summoned nothing (summons 0.0-0.1 alive at any checkpoint). The cost was real: at turn 400
   the Amurites had military strength -29 (t -2.1) and research -25 (t -2.1), 6.5 fewer
   buildings by turn 500 (the guild and caster hammers came out of Forges, Alchemy Labs,
   Courthouses, Moneychangers); the Sheaim military -25 (t -2.1), 0.9 fewer cities (t -2.4), and
   by turn 500 they had taken 1.8 -> 1.0 cities (t -2.8).
3. Casters are not worth their cost to this AI until it casts combat spells in combat. The port
   omits FfH's casting before a stack attacks (`AI_DirectDamageCast` and `AI_SummonCast` before
   `groupAttack`, docs/PORTING.md), so damage spells and summons happen only at the end of the
   turn when an enemy happens to be in range, and the buff cast fills every other caster turn.
   The correct version is generic, not per civilization: casters march with the strike stack and
   cast damage and summons right before it attacks, promotions chosen for that (Fire for
   Fireball, Death for Raise Skeleton), and only then does a Mage Guild have a value (the casters
   it enables), for every civilization. Not a balance verdict: FfH's casters are strong in human
   hands.

### Warcry held for the strike (Hippus, `ebb1427`)

Warcry (every unit +1 strength, +1 move and Blitz, about 20 turns) was held until the war
operation stepped next to its target or struck, a city was in peril, or 20 turns of war. Dev
15.0% -> 22.5% (4 / 1), share@300 +91 (t 2.10); fresh 14.1% -> 10.9% (1 / 3), share@300 -1,
share@end -6.

1. The generic cast came the turn war was declared, and the diagnosis assumed the war operation
   spends about half its turns gathering, so the buff would be gone before the strike.
2. The assumption was wrong for this AI: it declares war after 30-40 turns of preparation and
   strikes within about 3 turns (seed 5003: declared at turn 217, strikes at 220 and 226), well
   inside Warcry's ~20 turns, so the cast at the declaration already covers the strike. In two of
   four traced seeds both versions cast on the same turn; where they differed (seed 5001) the
   generic cast fell on a war fought with 3 units that ended in peace after 11 turns, and the held
   cast, at turn 306, changed nothing either. Over 64 games captures, losses and military strength
   are the same at every checkpoint.
3. The cast turn is not where the Hippus' wars are decided. The one waste the traces show is the
   default casting at any war, even one with no army; with the timing now in the AI
   (`world_spells.rs`), "at war with an army ready to strike" is the natural default to measure.
   Warcry's Blitz goes unused by `campaign::strike` (it drops units once they have attacked), but
   letting Blitz units attack again was flat in development, so the buff is small for this AI's
   stacks.

### Ritual projects (Illians, `76c1ffd`)

The most productive city built The White Hand (Philosophy, 400 hammers: three Priests of Winter)
and Stir from Slumber (Drifa) as soon as it could. Dev 12.5% -> 17.5% (2 / 0), share@end +269
(t 1.90); fresh 15.6% -> 14.1% (2 / 3), share@end -40 (t -0.56).

1. The generic AI reaches projects only when no building has value left, so The White Hand came
   very late or never (0.0 a game), and Stir from Slumber is excluded.
2. It was built more often (0.1 -> 0.6 a game) but late, because Philosophy is late: in traced
   fresh games the project finished at turns 325-372, and the three priests then spent the game
   summoning Ice Elementals (8-38 summons a game) and nothing else. Stir from Slumber was never
   built (it needs Divine Essence and an eliminated player). No snapshot measure moved (military,
   cities, captures, research flat at every checkpoint): the gain on seeds 1-40 was noise.
3. By turn 325 three strength-5 priests and their elementals are worth less than 400 hammers of
   the empire's best city. For this AI The White Hand is weak for its price (a balance note: it
   is priced for an earlier game than the one in which the AI reaches Philosophy); it would only
   matter with earlier Philosophy or with the priests casting in strikes (see casters).

### Vampires, Feast, Gift Vampirism, River of Blood held (Calabim, `efb8d23`)

Vampires were set back from `UNITAI_FEASTING` to `UNITAI_ATTACK`; Feast was cast in a city above
its happy cap by units below 30 XP; Gift Vampirism valued by the Vampire promotion; River of Blood
held until a civ war or until our cities had room for its two citizens each. Dev 7.7% -> 7.7%
(2 / 2), share@end +109 (t 0.59); fresh 10.9% -> 4.7% (2 / 6), share@end -77 (t -0.61).

1. The rules put every Vampire on `UNITAI_FEASTING` (from C++ `AI_feastingmove`), which no part
   of this AI accepts, so vampires sat out of every stack; Feast (FfH's value -50) and Gift
   Vampirism (value 0) were never cast.
2. The mechanics barely fire: vampires come late and only through Moroi upgrades (traced: from
   turns 280-370, 61 gold each), Feast was cast 1-14 times a game and Gift Vampirism 2-4, and the
   vampires, once in stacks, spent their casts on Haste and Regeneration like the casters above.
   River of Blood's hold released early (turns 147-175 in traced games). No snapshot measure moved
   (research slightly up, everything else flat); the fresh 2 / 6 is noise around zero.
3. The Calabim's vampire play is gated behind Feudalism, Governor's Manors and Moroi upgrades,
   which this AI reaches late, and then needs the generic caster/strike play. The stance fix is a
   correctness fix with nothing to show while vampires are rare; it is worth re-measuring with the
   caster work, not alone.

### Manes added to cities (Infernals)

Manes in one of our cities with another defender there cast Add to City (+1 population); lone
Manes walked to the nearest city. Dev: 6 games where the Infernals appear, population 29.5 ->
64.5, share 1056 -> 1557 (t 2.21). Fresh: the 60 games of seeds 5001-5064 (all forced
civilizations) where they appear, replayed with the Infernal logic off: population +5.0 (t 2.23),
units -6.2 (t -2.77), alive 0.8 -> 0.7 (t -1.66), share -29 (t -0.68), legacy score -66; the
Infernal team won no game on either side.

1. Fallow: Infernal cities produce no food and never grow; Manes are their only growth, and FfH's
   spell value (-250) never casts Add to City.
2. The Manes did become population, but they were most of the Infernals' army: the rule kept one
   defender per city and converted the rest, and the Infernals, arriving at turns 300-350 into a
   world at war, had a third fewer units and survived less often.
3. Growth must not cost the Infernals their defense: convert only the Manes beyond a garrison
   quota (the generic defense requirement) or at peace. Six games could not show this; the
   protocol above now asks for a fresh check over at least tens of games where the mechanic
   fires.

### Tried in development, not kept (round 5)

Each failed already on seeds 1-40 (most also on a second development set); the numbers and more
detail are in docs/AI.md, "Civilization-specific".

- **Holding a world spell with a lasting effect** -- Stasis (Illians), Religious Fervor
  (Malakim), Wild Hunt (Doviello), never casting Veil of Night (Svartalfar), and on fresh seeds
  Warcry and River of Blood above. 1: each was cast the first turn its gate allowed, often at
  peace, and a "better moment" looked obvious. 2: the held casts came later and the games show
  the cost of the wait, not a better effect: Stasis held lost share up to turn 400 (share@200
  -48), Fervor held 0 / 3, Veil never cast 0 / 4 (the army's ability to capture was worth less
  than the veil), Wild Hunt held with its wolves made army units 0 / 3. 3: a world
  spell cast at its gate keeps its effect for the rest of the game, and early effects compound;
  with the timing now in the AI (`world_spells.rs`), a better time has to be measured as
  "earlier or as early", and a hold only pays for a short effect tied to a battle (none
  confirmed yet).
- **Siege Workshop and Stable for the Khazad** (Trebuchets, Dwarven Cannons, Hornguard).
  1: these units need `PrereqBuilding` buildings the generic production never builds. 2: the
  engines were built (2.5 and 1.4 a game) but changed no war (win 28.2% -> 23.1%, 1 / 3); no
  Hornguard ever, because research never values Warhorses (`research::unit_available` skips
  `PrereqBuilding` units). 3: generic gap -- `PrereqBuilding` units are invisible to production
  and research; a per-civ building rule cannot fix research, and the siege engines' value depends
  on how strikes use collateral damage.
- **Sculptor's Studio for the Luchuirp** (Wood Golem, Gargoyle). 1: same `PrereqBuilding` gap.
  2: with both golems, Wood Golems replaced the Dwarven Slingers as defenders (no promotions,
  slow healing) and the Studios cost markets (share@300 -91, t -1.28); Gargoyle only, share@end
  -86. 3: golems are worse city defenders than promoted slingers for this AI; the Studio is worth
  it only for units the defense logic values correctly (healing and promotions counted).
- **Siege Workshop for the Lanun's War Tortoise**: 0.3 Tortoises a game, no effect (same gap;
  too few Tortoises to show anything, not traced).
- **Lanun coves only by boats without orders**: seeds 1-40 share@300 +34 (t 1.39), seeds 41-80
  -58 (t -1.75): the two development sets disagree, so the effect is zero and the rule is noise.
- **More Ljosalfar workers** (elves build ~twice as slowly on forest). 2: improvements +3.2 at
  turn 200 but fewer cities (legacy@200 -37.5, t -2.46: the workers' hammers came from settlers);
  only once no site is left: share@300 -46. 3: for this AI, cities beat improvements early; if
  the shortfall is worth fixing, the lever to try is which plots get improved first, not the
  worker count (untested).
- **Grigori Medics at war**: 0.9 a game, no effect (too few to matter; not traced). Not tried:
  keeping the Adventurers (the Grigori's first great people) as casters; they become ordinary
  army units upgraded by the generic code.
- **Malakim Citadel of Light** valued at 250 in frontier cities at war: every game
  byte-identical to the base -- the Malakim AI never researches Sorcery, so it is never
  buildable (the research plan's gap, not the building's).
- **Elohim Sanctuary held for an invasion**: the AI-only gate (two wars) opened in 4 of 40
  games; nothing to measure. With the gate now in the AI, Sanctuary needs a timing of its own
  (task: world-spell timing).
- **Bannor Crusade only in won wars**: taken in 7 instead of 15 games, no effect -- Crusade comes
  late (Fanaticism, turn 280-390) and costs 2 turns of anarchy, so its timing is not a lever.
- **Doviello**: a gold reserve for the upgrade spells (fewer Sons of Asena: worse -- spending the
  treasury on the only upgrade path is right); a Siege Workshop and Stables for the War Machine
  and Bison Riders never fired (research never got there in time). Not measured on fresh seeds:
  no Worker -> Beastman conversion at war (share@end +108, t 0.96, legacy@end t 2.08, win flat),
  the best untested candidate here.
- **Kuriotates variants**: no settlements at all (share@300 -183, t -1.9: the land matters);
  placement without the garrisons (share@300 +81 and +72 on two development sets, but share@end
  -4 and +16); an extra defender in every full city (share@end -318, t -2.0: unit support). The
  kept version is placement, garrisons and rebuild together.
- **Balseraphs: Create Puppet valued as a plain summon** instead of its `iAIWeight` 1000:
  share@200 -124 (t -1.66) -- the suicide puppets that FfH's weight produces are worth more than
  their summon value says.
- **Sheaim Planar Gate valued by its creatures**: built 0.1 a game, no effect (rarely eligible).
- **Hippus Blitz units attacking again in a strike**: flat (see Warcry).
