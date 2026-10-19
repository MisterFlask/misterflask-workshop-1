# Rules that treat AI players differently

Inventory (2026-10-19) of every branch in the rules (`crates/ffh-core`, ported from the FFH2 0.41
DLL and rules Python) whose outcome depends on whether a player is human or AI, plus the AI-only
branches of the Python the port did not take into the rules. Each group says what it is and
whether it is a hack (AI behaviour policy written as a rule, which belongs in `ffh-ai`) or a
legitimate rule. Moving a group into the AI is a decision per group (CLAUDE.md, "AI changes");
done so far: the world spells (A1). Line numbers are as of commit `3023b92`.

Counts: the rules read the human flag 229 times outside tests (`.human`, `*_is_human`), plus 7
`group_ai_is_controlled` sites, 22 reads of handicap AI fields (and 8 inert per-era reads), 1
`bAllowAI` read and 3 autorun sites; every read is in one of the groups below.

## A. AI behaviour policy in the rules (hacks)

**A1. World spells -- moved to the AI** (`d440143`, docs/PORTING.md): the AI-only conditions of 20
world spells' `req*` functions are `ffh-ai/src/world_spells.rs::ready`; no AI game changed.

**A2. Other spells' requirements** (`ffh/spells_a.rs`, `spells_b.rs`, from `CvSpellInterface.py`):
23 `req*` functions and one effect add conditions for AI casters only. `casting::choose_spell`
values spells from generic XML fields and never names these spells, so for the live ones the rule
is the only thing that stops a wasteful cast: moving them needs the same conditions in the AI
(as for the world spells), which changes no game.

| spell | AI-only condition (a human may cast without it) |
|---|---|
| Commander Join | caster base strength above 5 |
| Crew Buccaneers / Longshoremen / Skeleton Crew / Normal Crew | certain ship types only, Normal Crew never -- dead code: all four are `bAllowAI` 0 |
| Declare Nationality | not for pirate-sea or animal units, Hidden or Invisible units, barbarians |
| Destroy Undead | only undead of a team at war with us (a human: any undead in range) |
| Domination (requirement and effect) | resist chance cap 20% for the AI, 95% for a human; the effect also picks the target under that cap, so it needs a rules decision (one cap for all, or an AI-chosen target) |
| Earthquake | every owned plot in range belongs to a team at war with us |
| Escape | not at 50+ damage |
| Feed | 20+ damage |
| Hastur's Razor | one of its own damaged units in range (a human: any) |
| Healing Salve | 25+ damage |
| Hellfire | not on its own plot |
| Recruit Mercenary | never for the Khazad; only with a unit at war within 1 plot |
| Rob Grave | not for Good alignment |
| Sanctify | plot (x+1, y+1) must be its own (a Python quirk); never Infernal |
| Scorch | plains / Fields of Perdition only if not its own; snow only if its own and not Doviello or Illians |
| Sirona's Touch | 15+ damage |
| Spread the Council of Esus | its state religion is Esus |
| Spring | desert only if its own (no flames); never Infernal |
| Take Equipment (22 AI-castable `SPELL_TAKE_*_PROMOTION`) | caster strength - 2 above the holder's, and the holder's unit type lacks the item as a free promotion |
| Upgrade Doviello Warrior (worker -> Beastman) | at war, and workers at least the city count |

**A3. Other AI behaviour inside rules functions** (mostly C++ "BETTER AI" additions):

| # | where | what the AI does differently | `ffh-ai` relies on it |
|---|---|---|---|
| a | `group/movement.rs:376-410` (`CvSelectionGroup::groupAttack`) | an AI group attacking an enemy city whose defences fall in 1-7 bombard turns bombards with every unit and ends the head's moves instead of attacking (a human attacks); the pre-attack AI casting there is omitted | yes: `campaign.rs` holds siege units back while walls stand |
| b | `movement.rs:907-923` (`AI_getBestGroupAttacker`) | for an AI group, war wizards need 95% odds and heroes 85% to be chosen as the attacker, units above level 4 count half below 95% | implicitly (assaults) |
| c | `movement.rs:984-996` (`AI_getBestGroupSacrifice`) | AI war wizards, heroes and units above level 4 are never sacrificed below 68% odds | as b |
| d | `player/civics_religion.rs:832` (`canDoReligion`) | an AI cannot "do" a religion its leader weights below 0 (a human: only at -100 or less); feeds conversion and forced-religion deals | yes: `empire.rs` filters religions through `player_can_convert` |
| e | `civics_religion.rs:858-872` (`canConvert`) | an AI cannot convert to a religion weighted below 0 (the favourite-religion part is inert) | yes |
| f | `unit/promotions.rs:408` (`canUpgrade`) | AI missionary-AI units below level 2 cannot upgrade (stops priest spam) | yes: upgrades and priest training |
| g | `promotions.rs:417` | AI permanent-defense units cannot upgrade to units without a defensive bonus -- inert, nothing sets the flag | no: delete |
| h | `unit/ffh.rs:1258-1277` (`castCreateUnit`) | an AI caster's summon is flagged for suicide mode, a Fireball summon bombards at once, temporary summons join the caster's group | yes: `casting.rs` reads `ai_suicide_summon` |
| i | `unit/ffh.rs:1943` (`doImmortalRebirth`) | a reborn AI immortal gets a heal mission | no |
| j | `unit/missions.rs:435` (`CvUnit::found`) | an AI settler's counter-AI escorts leave its group | mostly inert |
| k | `ffh/custom.rs:1317` (`giftUnit`) | a Manes given to an AI Infernal joins a city as population, (100 - 5 x pop)% of the time; a human keeps the unit | no |
| l | `player/diplomacy_trade.rs:404` (`canTradeItem`) | an AI keeps one copy of a resource it trades; a human may trade its last | little: `trade.rs` keeps a copy anyway |
| m | `path.rs` (several), `group/misc.rs:1209` (path callbacks, `AI_getPlotDanger`) | AI groups that cannot fight avoid danger, AI land groups path only within their area, attack paths weigh defences, border danger counts for the AI only | heavily: decision support inside the shared path finder; low priority |

**A4. AI shortcuts written as rules**
- **Lairs** (`unit/lifecycle.rs:1124`, `CvUnit::setXY`): an AI unit at war with the barbarians that
  enters any lair destroys it; a human unit destroys only non-permanent ones and explores the
  permanent ones (Barrow, Goblin Fort, Ruins) with the Explore Lair spells, which the AI cannot cast
  (`bAllowAI` 0). `ffh-ai` targets lairs this way (`military.rs`). Needs a design decision.
- **Information** (`unit/movement.rs:415-458`, `plot/units.rs:475`, the path finder's revealed
  checks; `canMoveInto`, `movementCost`): the AI moves with full information (enemy checks on
  unseen plots, the real owner rather than the revealed one, real terrain costs on unrevealed
  plots). An information advantage `ffh-ai` relies on throughout; expensive to remove.
- **Godslayer** (`ffh/events.rs:1759, 1771`, Ascension): a human gets the sword through an event
  that hides it on a remote plot, an AI in its capital.
- **Barbarian city production** (`ffh/custom.rs:1459`, `CustomFunctions.BarbCityProduction`): AI
  production code in the rules (called from `ffh-ai` `production.rs`); not a rule, belongs in the AI.

## B. Difficulty handicap (legitimate: Civ4's difficulty system)

The handicap tables give AI players their own percentages: train, construct and create costs
(`player/units.rs:598/1263/1296`), growth, unit cost and supply, inflation, war weariness
(`player/economy.rs`), civic upkeep, upgrade price, work rate, free XP, the animal and barbarian
combat bonuses (`unit/combat.rs:1033-1069`), advanced-start points and starting units
(`player/start.rs`), free techs (`game/setup_init.rs:491`) and the starting-plot pick order. At the
standard handicap (Noble, which AI-only games use) the AI already gets construct 85%, unit supply
50%, upgrade cost 25%, inflation 50%, war weariness 70% and +25% against barbarians; in all-AI
games these apply to every player alike. Hard-coded, not scaled by difficulty: an AI city's
initial production of 10 (`city/turn.rs:170`) and free units for an AI that has met a human
(`player/economy.rs:332`). The AI-assist options `AI_NO_MINIMUM_LEVEL` and `AI_NO_BUILDING_PREREQS`
are off by default. The per-era AI modifier is 0 at every difficulty (inert).

## C. Decisions a human makes in a popup and the AI by code (no rules difference)

These go through `Controller`: production and its re-choice triggers, research and free techs,
conquered and flipped cities (the AI never gifts a conquered city and keeps a flipped one), the
Kuriotates city-or-settlement choice (the AI takes a full city), captured units, gifts, elections,
take-control offers, the citizen governor, trade denials, end of turn, war-plan bookkeeping, the
AI's queued follow-up attack, automation and wake-ups, `AI_isDeclareWar` (AI units enter closed
borders or declare war when moving only with a war plan ready; inert because `ffh-ai` declares war
explicitly). Three events give a human the AI's answer because the port has no popup hook for them:
Heroic Gesture 2, Great Mediator 2 (`ffh/random_events_b.rs:1136/1169`) and Wedding Feud 3
(`random_events_a.rs:2844`).

## D. Data flags

- **Spell `bAllowAI`** (`unit/ffh.rs:188`): 57 of the 267 spells cannot be cast by an AI: the 9
  Explore Lair spells, 6 Take Equipment (building, unit, Infernal Grimoire), 4 Crew, 5 Hire, 6 Found
  Temple, and Add to Flesh Golem, Arena Battle, Blaze, Burning Blood, Call Form, Commander Split,
  Consume Soul, Create Battering Ram, Enter Portal, Herald's Call, Hide, Impersonate Leader, Kidnap,
  Mask, Open Chest, Promote Settlement, Read the Grimoire, Reveal, Sacrifice Altar, Sacrifice Pyre,
  Sing, Slave Trade Buy, Slave Trade Sell, Sprint, Taunt, Tsunami, Wane. These are capabilities the
  old AI could not value: opening one needs AI logic for it (the lairs go with A4).
- Trigger `bOtherPlayerAI` (Saundra, Switch Civs never pick a human as the other player),
  civilization `bAIPlayable` (equal to `bPlayable` for every civilization), promotion `bAIControl`
  (Enraged: the unit acts on its own, for a human too): legitimate.

## E. Other asymmetries

- **Human-only rules that could be made symmetric**: stale production decays only in human cities
  (`city/turn.rs:1072, 1091`, a BtS anti-exploit); free techs through `chooseTech` reach only
  humans (Iron Orb option 3 gives an AI nothing; `player/economy.rs:821`, `random_events_a.rs:1461`,
  `team/tech.rs:570`); only a human entering Letum Frigus gets its reward (`spells_b.rs:2470`); only
  human Elohim see the unique improvements at the start (`ffh/events.rs:159`); the barbarians
  declare war only on a human Barbarian-trait civilization that is far ahead (`events.rs:305`).
- **Legitimate**: the challenge options (One City Challenge, No Healing for Humans, Always War,
  changing difficulty: about 20 sites), the BtS trade rules between human and AI teams (a human
  cannot become an AI's vassal, only AI teams sell war or peace with third parties or a civic
  change; `ffh-ai` hires war allies this way), the event semantics that need a human's attitude
  (`player/events.rs:1368`), Earth Day pushing a civic onto AI civilizations, Switch Civs for a
  leading human, holy-city and headquarters preference within mixed teams, minor ones (a traded
  city keeps its anger timers only for a human receiver; human starting units are scattered).
- Plot flags that only scenarios set (`move_disabled_human` / `_ai`): inert.

## Not ported as rules (AI policy in the Python)

- `CvGameUtils.canBuild` (no forts, no windmills under Arete, workshops need Guilds unless Fallow,
  no cottages under Aristocracy, no farms for Fallow): not rules (`py_can_build` answers -1);
  `ffh-ai` applies the bans itself (`workers.rs`, `finance.rs`).
- `cannotResearch` (religion techs the leader dislikes; Stirrups for the Clan and the Luchuirp),
  `cannotTrain` (scout, demagog, settler, worker and adept caps, no Warriors once better units are
  trainable, no Airship or Lightbringer, early caps on some units, no Privateers once two teams
  have Optics and Iron Working), `cannotConstruct` (no Mercurian Gate for evil AIs, Prophecy of
  Ragnarok only for evil AIs), `cannotCreate` (never Purge the Unfaithful, no Samhain under 4
  cities, no projects early or in weak cities): not applied anywhere.
- `CustomFunctions.warScript` / `dogpile`: replaced by `ffh-ai`'s diplomacy; `onUnitBuilt`'s AI
  unit-AI quotas, `AI_chooseProduction`, `AI_chooseTech`, `AI_MageTurn` and the C++ AI calls (77
  `// AI:` markers, including the pre-attack casting in `groupAttack`): replaced or omitted.

## Suggested order for moving the hacks

1. The 19 live spell gates of A2 (the same conditions in the AI; no game should change) and the 4
   dead crew branches (delete); Domination needs its rules decision first.
2. The religion bans (A3 d, e) into `empire.rs`; the missionary upgrade ban (A3 f) into the AI's
   upgrades, and A3 g deleted.
3. The summon handling (A3 h) and the small ones (i, j, k, l) into the AI.
4. The group-attack AI branches (A3 a-c): `ffh-ai` would choose attackers and bombard itself --
   the most work, and to be measured.
5. Lairs and the Explore Lair spells (A4): a design decision.

Leave B (difficulty), the challenge options and C; the information advantage (A4) and the path
finder's AI options (A3 m) are large AI changes for little gain.
