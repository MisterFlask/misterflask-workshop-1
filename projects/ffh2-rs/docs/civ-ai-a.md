# Civilization-specific AI, group A (Amurites, Balseraphs, Bannor, Calabim, Clan of Embers)

Sections in the style of docs/AI.md "Civilization-specific", for the orchestrator to integrate.
Every number is `crates/ffh-cli/scripts/civ_ab.py --civ X --seeds 1-40`, the civ forced into slot
0 of 6-civ standard games, against **the same binary with `FFH_AI_CIV_OFF=X`** (`--off-base`:
identical opponents, so only that civ's own logic differs). "Full games" play to a decision or
turn 690 (`--until-supremacy`); share is score v2 in basis points (fair share 1667; dead = 0);
t is the paired t over seeds; wins are "won only by new / only by base" with the sign test.

- **Clan of Embers**:
  - *Warrens* (Masonry, 180 hammers): every unit finished in the city that is not a world,
    national, siege or naval unit gets a free copy (settlers and workers too). The effect is
    Python only, so the generic estimate was 0 and it was never built (0/40 games). Valued as
    the hammers the copies add: half the city's production (a hammer a turn is 150 in
    `building_value`); research picks Masonry up through the same value. Built in 3.4 cities
    by turn 300, 7.6 by the end.
  - *Rantine* (hero) casts Convert City in a barbarian city whose barbarian units are no
    stronger than him: the city becomes the Clan's, without war. The Clan is at peace with the
    barbarians (`TRAIT_BARBARIAN`), so no army ever went there. While no civ war is on and he is
    healthy, Rantine walks to the nearest reachable (15 turns) such city and casts. +1.2 cities
    at turn 300 (t 3.2) on its own.
  - *For the Horde* (world spell, barbarian player with 60+ units): half the barbarian orcs
    become the Clan's. Cast only at war with a civ or while preparing one, instead of the first
    turn the gate opens: cast in 27 of 40 full games, first seen around turn 320 instead of 218.
    Held, the barbarians (the Clan's allies) keep harassing its rivals, and the converted units
    arrive with a war to fight instead of costing support at peace.
  - Full games: win rate 33.3% -> 43.6% (6 / 2, p 0.29; 39 pairs, one crash), share@300
    2086 -> 2233 (t 1.95), share@end 2391 -> 2978 (t 2.94), legacy@end +22% (t 2.2).
    300-turn leave-one-out (share@300 against the logic off): Rantine alone +93 (t 2.0), For
    the Horde alone +141 (t 2.4), Warrens alone -40 (t -1.8: more power, +52, but less
    research and score by turn 300). Full games without the Warrens: share@end +362 (t 1.9)
    instead of +588, so the Warrens pay late and are kept.
- **Balseraphs**:
  - *Loki* (hero, `bRivalTerritory`) casts Disrupt in a city of another team: 0-2 of the owner's
    culture there is lost (`CvSpellInterface.py` `spellDisrupt`), and a city left with none
    becomes the Balseraphs', without war (the owner's attitude -4). The generic AI used him as a
    2-strength army unit. At peace he goes to the nearest reachable (12 turns) revealed city of a
    civ we are not at war with whose owner culture is below 20 and whose culture rate is 0 (one
    Disrupt a turn drains one point on average, so a city producing culture never falls) and
    casts Disrupt there every turn: 1-10 cities a game in traced 300-turn games (about 3). Full
    games: win rate 17.5% -> 20.0% (5 / 4), share@300 1751 -> 1919 (t 2.16), share@end
    1597 -> 1864 (t 1.52); cities +1.1 at turn 300 (t 2.4). 300 turns: share@300 +189 (t 2.4).
  - Tried, not kept: Create Puppet and Create Puppet Loki valued as plain summons (by the
    puppet's strength, only with enemies in reach) instead of their `iAIWeight` 1000, which makes
    every adept summon a puppet every turn instead of its own spells. 300 turns: share@200 -124
    (t -1.66), share@300 -77 (t -0.8); the suicide puppets are worth more than they look.
  - Revelry (golden age at 25 population) is cast as soon as the gate opens, which is fine
    (the bonus compounds); not changed.
- **Calabim** (kept, effect within noise):
  - *Vampires*: the rules put every Vampire (and every Moroi upgraded into one) on
    `UNITAI_FEASTING` (`ev_unit_created`, from C++ `AI_feastingmove`), which no part of this AI
    accepts: they were left out of stacks, siege forces and the attacker count. `Calabim::turn`
    sets them back to `UNITAI_ATTACK`.
  - *Feast* (city -1 citizen, the unit + population - 2 XP; FfH's estimate -50, never cast):
    cast in a city above its happy cap, size 5+, by units below 30 XP (FfH's own rule).
    *Gift Vampirism* (Python only, value 0): the Vampire promotion's value per level 6+ unit
    (level 4+ Moroi) on the plot.
  - *River of Blood* (world spell, 5+ cities): held until a civ war or until our cities have the
    happy and healthy room to keep the two citizens each gains.
  - Full games: win rate 7.7% -> 7.7% (2 / 2), share@300 1404 -> 1419 (t 0.68), share@end
    996 -> 1104 (t 0.59), River of Blood in 30 / 30 games, first seen around turn 210 instead
    of 201. Vampires stay rare (mean 0.0 -> 0.1 at the end: they need Feudalism and a Governor's
    Manor), so these rules are seldom reached; the gain is not established.
- **Bannor** (nothing kept): Crusade is adopted by the generic AI in any war (its `iAIWeight` of
  1000 a city), and in traced games a Crusade in a losing war came before a collapse (6 cities
  to 1). Held back to wars we are clearly winning on power (ours at least 1.2x the enemies'
  sum, through the new `CivPlay::civic_value` hook, and kept once adopted), it was taken in 7
  instead of 15 full games and changed nothing measurable: win rate 10.0% -> 12.5% (1 / 0),
  share@300 +7.5 (t 1.28), share@end +10 (t 0.16). The hook stays (no civ overrides it). Notes:
  the anarchy on adopting Crusade is 2 turns in these games, not the ~6 the diagnosis estimated,
  and Crusade comes late (Fanaticism, turn ~280-390), after the turn-300 checkpoint.
- **Amurites** (kept, effect within noise):
  - *Casters*: Adepts and Mages need a Mage Guild in the training city (`PrereqBuildingClass`),
    which nothing built (`iAIWeight` 0, no yields), and the generic caster step comes after
    buildings with a target of (cities + 2) / 4: Amurite games had no Adept, Mage or Wizard at
    turn 300 (and no Mage Guild in 39 of 40). `Amurites::production`: the most productive city,
    unthreatened, with a unit affordable, builds the Mage Guild once Knowledge of the Ether is
    known (turn ~230 in these games) and trains casters up to (cities + 2) / 2. Full games:
    win rate 5.0% -> 7.5% (2 / 1), share@300 +1 (t 0.06: the casters come after turn 230),
    share@end 1071 -> 1179 (+108, t 1.06), alive 0.6 -> 0.7; at the end Wizards 0 -> 1.5, Adepts
    0 -> 0.8, Archmages 0 -> 0.2, Mage Guilds 0 -> 2.7.
  - Not attempted: building typed mana nodes (the diagnosis' A6: a larger change, and the
    casters it would feed only exist from turn ~230), Arcane Lacuna (its AI-only gate, 7 typed
    mana nodes on the map, was never met: the world spell was cast in 0 of 40 games on either
    side), Cave of Ancestors and Teach Spellcasting (small).

## Notes for the orchestrator

- **Generic fix in this branch**: `units::great_person` read `Player::capital_city` raw and
  panicked on a dangling id (a player whose capital was destroyed with no other city to move it
  to; reached with the Clan's conversions, Clan seed 31). It now uses `player_get_capital_city`,
  as the other AI readers already did. Games that did not crash are unchanged.
- **Casters need a Mage Guild for every civ.** Adepts (and Mages) have
  `PrereqBuildingClass` Mage Guild, a building with `iAIWeight` 0 and no yields that the generic
  production never values: the base Amurites had no Mage Guild in 39 of 40 games and no caster
  at all. This is the same cross-cutting gap as the Siege Workshop and Stable, and it probably
  removes Adepts from every civ's play; only the Amurites value it here (in their production
  hook).
- **AI legality under Crusade**: AI-initiated AI-to-AI deals (`diplomacy::manage`, peace and
  trades through `implement_deal`) never check `player_can_contact`, so a Crusading Bannor
  (`bNoDiplomacyWithEnemies`) still makes peace with its enemies; only `offers.rs` checks it
  (for human recipients). Generic, not fixed here.
- **AI-only gates** in the ported Python (rules code, left as they are): Arcane Lacuna needs 7
  typed mana nodes on the map, never met (0 / 40 games); For the Horde needs a barbarian player
  with 60 units; River of Blood and Rally need 5 cities; Revelry 25 population; Feed 20 damage.
  The Clan and Calabim changes only cast later than these gates allow, never earlier.
- **Risks**: Loki's Disrupt takes cities from civs at peace with the Balseraphs (each flip is
  -4 attitude with the victim); the Clan's held For the Horde leaves more barbarians on the map
  for longer (they are the Clan's allies and the other civs' enemies).
- No rules bug found in the mechanics touched (Warrens, Convert City, For the Horde, Disrupt,
  Feast, Gift Vampirism, River of Blood, Crusade, Mage Guild prerequisites behave as in the
  FfH2 Python/XML).
