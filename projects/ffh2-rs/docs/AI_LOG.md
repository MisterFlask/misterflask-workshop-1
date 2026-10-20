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

## Unique units (2026-10-20)

The kept version is in docs/AI.md, "Unique units". It is kept under the owner's rule (CLAUDE.md,
"AI changes"), so what its measurement shows is a cost to improve, not a gate. Back with it from
round 5 (below): the Amurite casters (now `civs::unique_casters`, also for the Svartalfar,
Doviello and Clan of Embers), the Illians' rituals, the Calabim vampire rules, the Grigori
Medics, the Infernal Manes beyond a garrison quota (point 3 of their entry), and the
building-gated units of the Khazad, Luchuirp and Lanun, now generic (`production::unit_building`).
Their entries stay as the record of what they cost then.

### An enabling building by strength squared per hammer (`48a0bf9`, reverted by `d9bee1e`)

1. With the Sculptor's Studio visible, the Luchuirp reached Construction (Wood Golems, 6) before
   Archery and built Studios from turn 150 (2.6 by turn 200 against 0.4). The Archery Range then
   looked only 14% better than the golems for their Dwarven Slingers (5 with a first strike, +25%
   in cities), under the 33% bar, so golems at 90 hammers held their cities (turn 300: 15 golems
   and 8.5 slingers, against 2.7 and 20) and research at turn 300 fell 211 -> 179. Weighed as
   `best_unit` weighs a choice, strength squared per hammer, the slinger is worth twice the golem.
2. Re-measured on five civilizations (Khazad, Kuriotates, Luchuirp, Malakim, Sidar; seeds
   8701-8720): squared strength per hammer overrates cheap units -- a Warrior (3, 25 hammers)
   outscores an Axeman (4, 60) -- so Training Yards were no longer built (turn 300: Khazad
   3.3 -> 0.9, Malakim 3.5 -> 1.5, Sidar 1.3 -> 0.3) and Warriors piled up (+3 to +6 a game in
   all five).
3. A building that opens a unit has to compare it with the city's best the way the role uses
   them, linearly: a third stronger, or a third stronger per hammer. The Archery Range then comes
   where golems hold a city (72% better per hammer) and the Training Yard where Warriors do (33%
   stronger). That is the kept rule; the Luchuirp's remaining cost is their army's, not their
   garrison's (docs/AI.md, "Unique units").

### Workers cast into Beastmen fight, without a gate (replaced the same day)

1. The Doviello's Worker -> Beastman (`SPELL_UPGRADE_DOVIELLO_WARRIOR`, cast by the rules' AI
   gate at war while they have as many workers as cities) keeps the worker's AI on the Beastman
   (`CvUnit::convert`). In 190 census games 558 Beastmen came from workers; they built nothing,
   sat in our cities, and production counted them as workers and trained none (Battlemasters, the
   same units cast on, spent 28% of their turns on the worker AI).
2. Giving them their own default AI (attack) let production train workers at war, and the cast
   took the new workers too: conversions 63 -> 79 and workers trained 80 -> 104 on the ten census
   seeds. With the other fixes (seeds 9201-9220) the Doviello survived less often: alive 0.7 ->
   0.5 (t -2.52), share@end -113 (t -0.62). The idle "workers" had been extra defenders in our
   cities.
3. Cast only on workers beyond production's own target (`production::worker_target`), so the
   cast never takes a worker production would replace. Against the same build without the fix and
   the gate (seeds 9201-9240): share@end -17 (t -0.20), alive flat, and from turn 300 1.3 more
   workers (t 2.9), improvements +4-10%, research +8-13%.

### Donal's Recruit in a city half as good as our best (replaced the same day)

1. Recruit is once, and a bigger city gives more units (population / 3 + 1).
2. Donal marches with the army: 5 of 9 Donals on the census seeds left their city and never
   recruited. And at two units a city, 3 of 9 still did not: a unit trained at the end of our
   turn first moves on the next, before the end-of-turn cast.
3. Recruit in the first city of ours giving two or more, at the start of our turn
   (`civs::bannor`): every Donal recruited (9 of 9).

## Spellcasting (2026-10-19, `magic.rs`)

The kept version is in docs/AI.md, "Spellcasting". Development measured each civilization forced
into slot 0, its spellcasting on against the same binary with `FFH_AI_MAGIC_OFF` for it (every
other civilization casting by `magic.rs`), seeds 1-20, full games (6 civs, standard map). These
variants failed there (the last one on a first fresh set, 8101-8120, spent by it) and were
changed before the final validation; the way each failed was found by replaying the seeds where
the two sides first split (`ffh autoplay --turns N` checksums bisect the turn, `FFH_AI_TRACE` and
`FFH_AI_TRACE_CAST` show the casts).

- **Summons at the end of the turn attacking where they take most hit points.** 1: a temporary
  summon dies at the start of our next turn anyway, so a hit point it takes from an enemy looked
  free whatever the odds; FfH's AI attacks with it only at 5% or better (`AI_summonAttackMove`),
  taking the best odds. 2: the Balseraphs' Loki casts Create Puppet every turn (FfH's weight
  1000) on his way to Disrupt a city; in seed 9 a barbarian Spectre shadowed him and his puppet
  attacked it every turn at hopeless odds, the Spectre then attacked the stack, Loki defended and
  lost 75 hit points, and hurt he was no longer sent to Disrupt (`civs/balseraphs.rs` gives up
  over 20 damage): no city taken by turn 110 instead of 2. Balseraphs win 45% -> 30% (3 / 6),
  share@end -456 (t -1.14), captures down from turn 200 (2.0 -> 1.6, t -2.3). With FfH's 5% floor
  but the target still chosen by hit points they still lost (45% -> 20%, 0 / 5; share@300 -208,
  t -2.13): fewer puppets alive at every checkpoint (turn 200 1.2 -> 0.9, t -2.3), and seed 8 split
  from the game without the change at turn 165 on a puppet's target alone. 3: a summon also
  defends what it stands with, and Loki's puppets are worth more as FfH plays them (the round-5
  entry on Create Puppet below says the same); summons attack as FfH's do
  (`casting::summons_attack`), and seed 8 is then the same game until turn 172.
- **Training casters for their spells** (two variants). 1: with spells cast where they help, a
  caster's worth could be read from them: one caster for every five field units when an adept
  could cast something worth taking to a fight with our mana, and the Mage Guild in our most
  productive safe city while no city could train one. The first variant also counted the second
  sphere promotions' spells (Fireball) at 3/4 for Arcane leaders (1/2 otherwise) once mages were
  within reach, and one more caster at war. 2: First variant: the Amurites' palace mana (Fire,
  Body, Metamagic) gives adepts Haste and Floating Eye (Blaze is not for the AI); their guild and
  one or two adepts came around turn 250 (casters at turn 300 0.1 -> 1.1) and cast nothing -- no
  Amurite cast in 100 turns of seed 9, where a one-city empire at war built the guild and an adept
  (the war's extra caster) and died at turn 338. Share@300 -16 (t -1.85), land at turn 350 -10 (t
  -2.1), research -22 (t -2.0), share@end -188 (t -1.56). Second variant, an adept's own spells
  only and no war bonus: only the Enchantment civilizations trained casters (Grigori, Illians,
  Luchuirp, Sidar: Enchanted Blade); 1.4 casters at turn 300 cost the Luchuirp a fifth of their
  research (t -1.7) and the Grigori a tenth by turn 350, the Illians and the Sidar had more army
  later, and against the same spellcasting without caster production (a scratch switch) the four
  lost wins: 17.5% -> 10.0% (3 / 9), share@300 -10 (t -1.5), share@end -4 (t -0.04). 3: a caster
  is hammers and upkeep paid around turn 250 for blessings and curses that pay only in the fights
  that follow, and this AI's late, small wars do not repay them (the third such result, after the
  Amurites and the Sheaim in round 5). Not kept: `magic.rs` trains no casters. Before trying
  again: magic earlier (research toward Knowledge of the Ether), or casters for a lasting summon
  (Raise Skeleton: an army unit a turn).
- **Skipping units that another unit's spell has just killed.** 1: a unit in delayed death
  should not act. 2: FfH's end-of-turn loop (`CvGame::updateMoves`) skips a unit only after its
  own cast killed it, so the second wolf a Form Wolf Pack consumes still casts Form Wolf Pack in
  its turn, forming a further pack from wolves already spent. Skipping it cost the Doviello packs
  (seed 13 split at turn 166 on that alone; share@end -250, t -1.83). 3: that is a C++ quirk of the
  AI loop, which `casting::end_turn` keeps for both paths (marked there); whether the AI should
  keep the extra packs is a separate decision, not part of the spellcasting change.
- **Casting nothing when nothing is worth casting.** 1: a cast that does nothing (Haste on a quiet
  turn, Dance of Blades where no unit can take it) is not worth a unit's turn, so the end of the
  turn cast only what had a value. 2: a cast is never quite a no-op -- it rolls for a miscast when
  the unit or spell has a chance of one, ends the promotions casting removes -- so every skipped
  cast split the game from FfH's: the Balseraphs' games split on a puppet's Dance of Blades on
  nobody (seed 10 at turn 85, seed 8114 at turn 193), and on fresh seeds 8101-8120 the first six
  civilizations went 1 / 7 in wins, share@end -85 (t -1.76), the Balseraphs -332 (t -2.29), most
  of it play that had nothing to do with spells after such splits. 3: when nothing is worth
  casting, the unit casts what FfH would: free, and the same game where the values see no
  difference (seed 8114 then plays the same through turn 300), so what a measurement shows is the
  change's own.

The kept version passed on fresh seeds 8201-8220: win 17.7% -> 18.5% (11 / 8), share@end +36
(t 1.39) (docs/AI.md, "Spellcasting").

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
   hands. (That generic version is "Spellcasting" above.)

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

### World spells at moments of their own (round 5, `0e4945b` on `scratch/ws-timing`)

After the world spells' AI-only gates moved into the AI (`world_spells.rs`), five spells got a
moment of their own instead of FfH's condition; each was measured with its civilization forced,
against the same binary with FfH's condition (`FFH_AI_WS_DEFAULT=SPELL_X`), development seeds 1-40
and fresh seeds 6001-6064. Stasis as soon as the Illians have a city won big (fresh win 4.7% ->
23.4%, 13 / 1) and is held for a balance decision: it freezes every rival's opening, a human's
included (docs/AI.md). Not kept:

- **Raging Seas** (Lanun: 75 damage to every non-Lanun unit on a plot next to water, a quarter
  of the other civilizations' coastal improvements destroyed) at a strike on a coastal city or a
  coastal city of ours in peril in a civ war. Dev 37.5% -> 25.0% (0 / 5), share@end +42; fresh
  28.1% -> 25.0% (2 / 4), share@end +44 (t 0.55). 1: FfH's condition (at war with a fleet worth
  10+) held in 6-7 of 40-64 games, around turn 580, and 75 damage to every coastal defender looked
  decisive. 2: cast in 25 / 44 games instead, around turn 370-390, mostly on "a coastal city in
  peril" during the Lanun's own winning offensives (traced: turns 322-359, cities falling to them
  around the cast). The Lanun ended slightly richer (commerce +47 and research +36 at turn 600,
  captures and losses unchanged), but the damage is world-wide and reshuffles who else rises
  (share at turn 450, own minus default: Clan -565 in seed 30, Ljosalfar -911 in seed 6023). In
  the three traced lost games another civilization won instead: a rival's supremacy at turn 526,
  ten turns before the Lanun's cultural victory without the cast (seed 14, where the Lanun were
  weaker after it: share at turn 500 2026 against 2219); a time victory for the team that got the
  Mercurians (seed 30, where the Lanun were stronger and held 36% of the world); a Lanun without
  the cultural goal at the end and stronger in share (seed 6023). 3: the
  win-rate loss (2 / 9 over both sets, p about 0.07) is not explained by one mechanism; what is
  clear is that a defensive trigger spends the spell on fights already being won, and that a spell
  that hits everyone but us mostly changes which rival rises. If tried again: only the strike
  trigger, gated on a strike the odds say fails without it.
- **Sanctuary** (Elohim: no foreign unit may enter our land for 30 turns, those inside thrown
  out) at a city in peril in a civ war. Dev 10.3% -> 10.3% (2 / 2), share@end +198 (t 1.51);
  fresh 7.8% -> 6.2% (0 / 1), share@end +8 (t 0.10). 1: FfH's condition (two wars at once) opened
  in few games and late (cast in 27 of 64, around turn 420). 2: cast in 47 of 64 around turn 323;
  cities lost, captures and every checkpoint measure unchanged (lost cities at turn 600 2.9 ->
  2.8): "a city in peril" (the visible threat exceeds the garrison) is a weak signal, and the
  30-turn lockout delays an attacker the Elohim would have held anyway. The development gain did
  not repeat. 3: a lockout only matters against an invasion that would take cities; the trigger
  has to be that (several cities threatened, or a losing war), and even then the measured value
  of delaying a war is small for this AI.
- **March of the Trees** (Ljosalfar: a Treant for 5 turns on every forest of ours, the forests
  turned to New Forest) at a city in peril with 6+ of our forests within 2. Dev 32.5% -> 30.0% (2 /
  3), share@end -158 (t -1.33); fresh 37.5% -> 28.1% (2 / 8), share@end -60. 1: FfH's condition
  (two wars) cast it in 19 of 64 games, late (turn ~420), and Treants on every forest plot looked
  like a decisive defense. 2: it fired at the start of the Ljosalfar's own offensives (traced:
  turns 235, 331, 386, each a few turns after their declaration, while their stacks were taking
  cities), so the Treants defended against little; and the price is permanent: every Ancient
  Forest becomes a New Forest, losing its +1 food for good (Ancient Forests never regrow) and half
  its health. After the cast the elves fell behind: military -34 at turn 500 (t -2.7), units -3.7
  (t -2.0), population -12 and production -14 at turn 600 (population t -2.3), one more city lost
  (t 1.9). 3: the spell trades the elves' forest economy for five turns of Treants; it is a last
  stand, and FfH's two-wars condition approximates "only in desperation". If cast at all: when
  cities would fall (several in peril, or the capital), never on an offensive.
- **Worldbreak** (Sheaim: every unit in a foreign city takes the Armageddon counter as damage,
  cities may burn) at a strike with a counter of 20+. Dev 5.0% -> 5.0% (0 / 0), share@end +23;
  fresh 3.1% -> 1.6% (0 / 1), share@end -58 (t -1.45); 49 of 64 fresh games unchanged. 1: FfH's
  condition (counter 50) was never met (0 casts). 2: the counter reaches 20 late (casts around
  turn 457-475, in 8 / 15 games), so the damage was 20-40 on defenders the Sheaim, the weakest
  civilization here (2-5% wins), mostly could not exploit; no checkpoint measure moved
  significantly. 3: Worldbreak's value grows with the counter, which this AI rarely raises; the
  Sheaim's problem is elsewhere (their economy and army), and the spell is not the lever.

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
