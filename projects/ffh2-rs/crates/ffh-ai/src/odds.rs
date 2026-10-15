//! Strike odds: the chance that a set of attackers takes a city, from the rules' combat odds.
//!
//! The attackers fight one after another, each against the defender the rules would put up.
//! Every fight ends in a kill (with the rules' `getCombatOdds`) or a loss that leaves the
//! defender wounded by the rounds the attacker won first; the distribution of those wounds
//! (first strikes, combat limits and withdrawal included) is worked out exactly, and the
//! defenders' health is carried from fight to fight as a distribution over joint states (one
//! hit-point value per defender), not as an expected value. Expected values let many long
//! shots add up to a "capture": six Hunters at 2% each wear an Archer down on paper, but each of
//! them dies having dealt a hit or two, and the Archer holds.
//!
//! Deterministic: no random draws (a dynamic program over states, exact up to rounding); equal
//! states merge, and when more than [`MAX_STATES`] remain the defenders' health is rounded to
//! coarser steps until they fit (only beyond steps of 32 hit points are the least probable states
//! dropped, counted as "the city holds"); ties are broken by index.

use ffh_core::game::Game;
use ffh_core::ids::*;
use ffh_core::unit::CombatDetails;
use ffh_core::utils::{combat_odds, CombatOddsInput};

use crate::view::attack_power;

/// Defenders modelled (more than that: the city is not taken).
const MAX_DEF: usize = 16;
/// States carried from one fight to the next.
const MAX_STATES: usize = 96;
/// States below this probability are dropped.
const MIN_P: f64 = 1e-6;

/// Outcome of a strike, worked out over the distribution of fights.
#[derive(Clone, Debug, Default)]
pub struct Odds {
    /// Probability that every defender falls to the attackers (all of them, in strike order).
    pub p_take: f64,
    /// Probability that every defender has fallen after the first `i + 1` attackers.
    pub taken_after: Vec<f64>,
    /// Expected attackers lost (dead, not withdrawn) before the city falls.
    pub exp_lost: f64,
    /// Expected defender hit points left at the end (summed over the defenders).
    pub exp_hp_left: f64,
    /// Best odds (per mille) of one of the attackers against the defender it meets first.
    pub best: i32,
    pub n_def: usize,
    /// Attackers (units able to attack).
    pub n_att: usize,
    /// Probability mass dropped with the least probable states (counted as holding).
    pub dropped: f64,
}

/// Sort `next` by state and merge equal states into `states`.
fn merge(next: &mut [([u8; MAX_DEF], f64)], states: &mut Vec<([u8; MAX_DEF], f64)>) {
    next.sort_unstable_by(|a, b| a.0.cmp(&b.0));
    states.clear();
    for &(s, p) in next.iter() {
        match states.last_mut() {
            Some(last) if last.0 == s => last.1 += p,
            _ => states.push((s, p)),
        }
    }
}

impl Odds {
    /// Attackers needed (in strike order) for the city to fall with probability `p`, if the
    /// force suffices.
    pub fn needed(&self, p: f64) -> Option<usize> {
        if self.n_def == 0 {
            return Some(0);
        }
        self.taken_after.iter().position(|&t| t >= p).map(|i| i + 1)
    }
}

/// One fight's constants: attacker `a` against defender `d` (strengths with every modifier,
/// city defense reduced by the bombarded `wall_cut`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Pair {
    a_str: i32,
    a_fp: i32,
    a_hp: i32,
    limit: i32,
    a_fs: i32,
    a_cfs: i32,
    a_immune: bool,
    withdraw: i32,
    d_max: i32,
    d_fs: i32,
    d_cfs: i32,
    d_immune: bool,
    max_hp: i32,
}

/// Outcomes of one fight from a given defender health: (defender hit points after, probability
/// the attacker survives, probability it dies).
type Outcomes = Vec<(u8, f64, f64)>;

fn binom(n: i32, k: i32) -> f64 {
    if k < 0 || k > n {
        return 0.0;
    }
    let k = k.min(n - k);
    let mut r = 1.0f64;
    for i in 0..k {
        r = r * (n - i) as f64 / (i + 1) as f64;
    }
    r
}

/// The rules' odds of fight `k` at defender health `hp` (per mille) and the distribution of its
/// outcomes (CvUnit::resolveCombat: rounds won with the rules' per-round odds, first strikes,
/// the attacker's combat limit, withdrawal).
fn outcomes(k: &Pair, hp: i32, die: i32, damage: i32) -> (i32, Outcomes) {
    let max_hp = k.max_hp.max(1);
    let hp = hp.clamp(1, max_hp);
    let d_str = (k.d_max * hp / max_hp).max(1);
    let d_fp = (k.d_max + d_str + 1) / 2;
    let input = CombatOddsInput {
        attacker_strength: k.a_str,
        attacker_firepower: k.a_fp,
        defender_strength: d_str,
        defender_firepower: d_fp,
        attacker_hit_points: k.a_hp,
        defender_hit_points: hp,
        defender_max_hit_points: max_hp,
        attacker_combat_limit: k.limit,
        attacker_first_strikes: k.a_fs,
        attacker_chance_first_strikes: k.a_cfs,
        attacker_immune_to_first_strikes: k.a_immune,
        defender_first_strikes: k.d_fs,
        defender_chance_first_strikes: k.d_cfs,
        defender_immune_to_first_strikes: k.d_immune,
    };
    let survive = k.withdraw.clamp(0, 100) as f64 / 100.0;
    let win_hp = if k.limit < max_hp { (max_hp - k.limit).max(1) as u8 } else { 0 };
    let defender_odds = die * d_str / (k.a_str + d_str).max(1);
    if defender_odds == 0 {
        return (1000, vec![(win_hp, 1.0, 0.0)]);
    }
    let attacker_odds = die - defender_odds;
    let sf = (k.a_fp + d_fp + 1) / 2;
    let to_att = (damage * (d_fp + sf) / (k.a_fp + sf).max(1)).max(1);
    let to_def = (damage * (k.a_fp + sf) / (d_fp + sf).max(1)).max(1);
    let hit_limit = max_hp - k.limit;
    let n_a = ((hp - hit_limit).max(0) + to_def - 1) / to_def;
    let n_d = (k.a_hp.max(1) + to_att - 1) / to_att;
    if n_a == 0 {
        // Nothing left for this attacker to do (a siege engine against a defender at its limit).
        return (0, vec![(hp as u8, 1.0, 0.0)]);
    }
    if attacker_odds <= 0 || k.a_str <= 0 {
        return (0, vec![(hp as u8, survive, 1.0 - survive)]);
    }
    let odds = combat_odds(&input, die, damage).clamp(0, 1000);
    let p = attacker_odds as f64 / die as f64;
    let q = 1.0 - p;
    // loss[h]: probability the attacker loses having landed h hits.
    let mut loss = vec![0.0f64; n_a as usize];
    let (a_lo, a_hi) = if k.d_immune { (0, 0) } else { (k.a_fs, k.a_fs + k.a_cfs) };
    let (d_lo, d_hi) = if k.a_immune { (0, 0) } else { (k.d_fs, k.d_fs + k.d_cfs) };
    let w = 1.0 / ((a_hi - a_lo + 1) * (d_hi - d_lo + 1)) as f64;
    // From h hits landed and m taken (weight pw): the race of the normal rounds, lost after
    // landing h2 hits.
    let race = |loss: &mut Vec<f64>, h: i32, m: i32, pw: f64| {
        let need = n_d - m;
        for h2 in h..n_a {
            let x = h2 - h;
            loss[h2 as usize] += pw * binom(x + need - 1, x) * p.powi(x) * q.powi(need);
        }
    };
    for i in a_lo..=a_hi {
        for j in d_lo..=d_hi {
            if i > j {
                // Our first strikes: rounds only we can win.
                let f = i - j;
                for x in 0..=f.min(n_a - 1) {
                    race(&mut loss, x, 0, w * binom(f, x) * p.powi(x) * q.powi(f - x));
                }
            } else if j > i {
                // Theirs: the attacker may die before landing a hit.
                let f = j - i;
                for y in 0..=f.min(n_d - 1) {
                    race(&mut loss, 0, y, w * binom(f, y) * q.powi(y) * p.powi(f - y));
                }
                let mut dead = 0.0;
                for y in n_d..=f {
                    dead += binom(y - 1, n_d - 1) * q.powi(n_d) * p.powi(y - n_d);
                }
                loss[0] += w * dead;
            } else {
                race(&mut loss, 0, 0, w);
            }
        }
    }
    // The rules' odds decide kill or loss; the rounds decide how much a loss wounds.
    let lost_dp: f64 = loss.iter().sum();
    #[cfg(test)]
    tests::DP_WIN.with(|c| c.set(1.0 - lost_dp));
    let lost = 1.0 - odds as f64 / 1000.0;
    let mut out: Outcomes = Vec::with_capacity(n_a as usize + 1);
    if odds > 0 {
        out.push((win_hp, odds as f64 / 1000.0, 0.0));
    }
    if lost > 0.0 {
        if lost_dp > 1e-12 {
            for (h, &l) in loss.iter().enumerate() {
                if l > 0.0 {
                    let pl = l * lost / lost_dp;
                    let left = (hp - h as i32 * to_def).max(1) as u8;
                    out.push((left, pl * survive, pl * (1.0 - survive)));
                }
            }
        } else {
            let left = (hp - (n_a - 1) * to_def).max(1) as u8;
            out.push((left, lost * survive, lost * (1.0 - survive)));
        }
    }
    (odds, out)
}

/// C++ `AI_getBestGroupSacrifice`-like value: attack power and withdrawal chance for the cost.
pub fn sacrifice_value(g: &Game, u: UnitId) -> i64 {
    let cost = g.d.units[g.units[u].unit_type].production_cost.max(1) as i64;
    attack_power(g, u) as i64 * (100 + g.unit_withdrawal_probability(u) as i64) * 100 / (cost * (10 + g.units[u].experience.max(0) as i64))
}

/// `apply_combat_modifier` of the rules, inverted and reapplied: a strength computed with
/// modifier `from` recomputed with modifier `to`.
fn remodify(strength: i32, from: i32, to: i32) -> i32 {
    let base = if from > 0 { strength as i64 * 100 / (from as i64 + 100) } else { strength as i64 * (100 - from as i64) / 100 };
    let s = if to > 0 { base * (to as i64 + 100) / 100 } else { base * 100 / (100 - to as i64) };
    (s as i32).max(1)
}

/// Defender `d`'s strength on `tp` against attacker `a`, `wall_cut` points of city defense
/// bombarded away.
fn defender_max(g: &Game, tp: PlotIdx, a: UnitId, d: UnitId, wall_cut: i32) -> i32 {
    if wall_cut > 0 {
        let mut det = CombatDetails::default();
        let m = g.unit_max_combat_str(d, Some(tp), Some(a), Some(&mut det));
        remodify(m, det.modifier_total, det.modifier_total - wall_cut)
    } else {
        g.unit_max_combat_str(d, Some(tp), Some(a), None)
    }
}

/// The fight constants of attacker `a` against defender `d`, and the defender's rank (C++
/// `isBetterDefenderThan`: strength and first strikes; times its health it picks the defender
/// the rules put up).
fn pair(g: &Game, tp: PlotIdx, a: UnitId, d: UnitId, wall_cut: i32) -> (Pair, i64) {
    let d_max = defender_max(g, tp, a, d, wall_cut);
    let k = Pair {
        a_str: g.unit_curr_combat_str(a, None, Some(d), None),
        a_fp: g.unit_curr_firepower(a, None, Some(d)),
        a_hp: g.unit_curr_hit_points(a),
        limit: g.units[a].combat_limit,
        a_fs: g.unit_first_strikes(a),
        a_cfs: g.unit_chance_first_strikes(a),
        a_immune: g.unit_immune_to_first_strikes(a),
        withdraw: g.unit_withdrawal_probability(a),
        d_max,
        d_fs: g.unit_first_strikes(d),
        d_cfs: g.unit_chance_first_strikes(d),
        d_immune: g.unit_immune_to_first_strikes(d),
        max_hp: g.unit_max_hit_points(d),
    };
    let fs_damage = (g.d.defines.combat_damage * 2 / 5) as i64;
    let mut r = d_max as i64;
    if !k.a_immune {
        r = r * ((k.d_fs * 2 + k.d_cfs) as i64 * fs_damage + 100) / 100;
    }
    if k.d_immune {
        r = r * ((k.a_fs * 2 + k.a_cfs) as i64 * fs_damage + 100) / 100;
    }
    (k, r)
}

/// Everything about a strike that does not depend on the defenders' health: per attacker the
/// fight constants against each defender (attackers with the same constants share a kind and
/// their fight outcomes), the defenders' ranks, collateral damage.
struct Prep {
    n_def: usize,
    atts: Vec<UnitId>,
    kinds: Vec<Vec<Pair>>,
    kind_of: Vec<usize>,
    ranks: Vec<Vec<i64>>,
    /// Per attacker: (damage, cap) on each defender and the most units hit; None without
    /// collateral damage.
    collat: Vec<Option<(Vec<(i32, i32)>, i32)>>,
    /// Groups (two or more) of interchangeable defenders (same type, promotions, fortification
    /// and strength): their health is kept sorted, so states that differ only by which of them
    /// is hurt merge.
    groups: Vec<Vec<usize>>,
    init: [u8; MAX_DEF],
    max_hp: i32,
    die: i32,
    damage: i32,
    memo: std::collections::BTreeMap<(usize, usize, u8), usize>,
    memo_out: Vec<(i32, Outcomes)>,
}

impl Prep {
    fn new(g: &Game, tp: PlotIdx, defs: &[UnitId], atts: &[UnitId], wall_cut: i32) -> Prep {
        let n_def = defs.len().min(MAX_DEF);
        let defs = &defs[..n_def];
        let atts: Vec<UnitId> = atts.iter().copied().filter(|&u| attack_power(g, u) > 0 && g.unit_can_attack(u)).collect();
        let strength: Vec<i32> = defs.iter().map(|&d| g.unit_max_combat_str(d, Some(tp), None, None)).collect();
        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut grouped = vec![false; n_def];
        for j in 0..n_def {
            if grouped[j] {
                continue;
            }
            let uj = &g.units[defs[j]];
            let same: Vec<usize> = (j..n_def)
                .filter(|&o| {
                    let uo = &g.units[defs[o]];
                    !grouped[o] && uo.unit_type == uj.unit_type && uo.fortify_turns.min(5) == uj.fortify_turns.min(5) && strength[o] == strength[j] && uo.has_promotion == uj.has_promotion
                })
                .collect();
            for &o in &same {
                grouped[o] = true;
            }
            if same.len() >= 2 {
                groups.push(same);
            }
        }
        let mut init = [0u8; MAX_DEF];
        for (j, &d) in defs.iter().enumerate() {
            init[j] = g.unit_curr_hit_points(d).clamp(1, 255) as u8;
        }
        let mut kinds: Vec<Vec<Pair>> = Vec::new();
        let mut kind_of = Vec::with_capacity(atts.len());
        let mut ranks = Vec::with_capacity(atts.len());
        let mut collat = Vec::with_capacity(atts.len());
        for &a in &atts {
            let (ps, rs): (Vec<Pair>, Vec<i64>) = defs.iter().map(|&d| pair(g, tp, a, d, wall_cut)).unzip();
            let kind = match kinds.iter().position(|k| *k == ps) {
                Some(i) => i,
                None => {
                    kinds.push(ps);
                    kinds.len() - 1
                }
            };
            kind_of.push(kind);
            ranks.push(rs);
            let cs = g.unit_base_combat_str(a) * g.unit_collateral_damage(a) / 100;
            if cs > 0 && n_def >= 2 {
                let limit = g.unit_collateral_damage_limit(a);
                let uc = g.unit_get_unit_combat_type(a);
                let per: Vec<(i32, i32)> = defs
                    .iter()
                    .map(|&d| {
                        if uc.is_some_and(|c| g.d.units[g.units[d].unit_type].unit_combat_collateral_immune[c] != 0) {
                            return (0, 0);
                        }
                        let ts = g.unit_base_combat_str(d);
                        let sf = (cs + ts + 1) / 2;
                        if ts + sf <= 0 {
                            return (0, 0);
                        }
                        let mut dmg = g.d.defines.collateral_combat_damage * (cs + sf) / (ts + sf);
                        dmg *= 100 + g.units[a].extra_collateral_damage;
                        dmg *= (100 - g.units[d].collateral_damage_protection).max(0);
                        dmg /= 100;
                        dmg /= 100;
                        (dmg.max(0), limit.min(limit * (cs + sf) / (ts + sf)))
                    })
                    .collect();
                collat.push(Some((per, g.unit_collateral_damage_max_units(a))));
            } else {
                collat.push(None);
            }
        }
        let mut p = Prep {
            n_def,
            atts,
            kinds,
            kind_of,
            ranks,
            collat,
            groups,
            init,
            max_hp: g.d.defines.max_hit_points.max(1),
            die: g.d.defines.combat_die_sides,
            damage: g.d.defines.combat_damage,
            memo: std::collections::BTreeMap::new(),
            memo_out: Vec::new(),
        };
        p.canon(&mut init);
        p.init = init;
        p
    }

    fn canon(&self, s: &mut [u8; MAX_DEF]) {
        for grp in &self.groups {
            let mut v = [0u8; MAX_DEF];
            for (k, &j) in grp.iter().enumerate() {
                v[k] = s[j];
            }
            v[..grp.len()].sort_unstable_by(|a, b| b.cmp(a));
            for (k, &j) in grp.iter().enumerate() {
                s[j] = v[k];
            }
        }
    }

    /// The defender the rules put up against attacker `i` in state `s` (best rank times
    /// health), and how many still stand.
    fn defender(&self, i: usize, s: &[u8; MAX_DEF]) -> (usize, usize) {
        let mut j = 0;
        let mut jr = -1i64;
        let mut alive = 0;
        for o in 0..self.n_def {
            if s[o] == 0 {
                continue;
            }
            alive += 1;
            let r = self.ranks[i][o] * s[o] as i64;
            if r > jr {
                jr = r;
                j = o;
            }
        }
        (j, alive)
    }

    /// Fight outcomes of attacker `i` against defender `j` at health `hp` (memoized; index
    /// into `memo_out`).
    fn fight(&mut self, i: usize, j: usize, hp: u8) -> usize {
        let key = (self.kind_of[i], j, hp);
        if let Some(&x) = self.memo.get(&key) {
            return x;
        }
        let out = outcomes(&self.kinds[self.kind_of[i]][j], hp as i32, self.die, self.damage);
        self.memo_out.push(out);
        self.memo.insert(key, self.memo_out.len() - 1);
        self.memo_out.len() - 1
    }

    /// Odds (per mille) of attacker `i` against the defender it meets first.
    fn first_odds(&mut self, i: usize) -> i32 {
        let init = self.init;
        let (j, _) = self.defender(i, &init);
        let x = self.fight(i, j, init[j]);
        self.memo_out[x].0
    }

    /// The order in which the strike sends the attackers (see `campaign::strike`): collateral
    /// attackers first while several defenders stand, then the best odds when they are good
    /// (68%+), else the unit cheapest to lose for its strength.
    fn strike_order(&mut self, g: &Game) -> Vec<usize> {
        let n = self.atts.len();
        let odds: Vec<i32> = (0..n).map(|i| self.first_odds(i)).collect();
        let best = odds.iter().copied().max().unwrap_or(0);
        let several = self.n_def >= 2;
        let mut keyed: Vec<(bool, i64, UnitId, usize)> = (0..n)
            .map(|i| {
                let u = self.atts[i];
                let c = several && g.unit_collateral_damage(u) > 0;
                let v = if c || best >= 680 { odds[i] as i64 } else { sacrifice_value(g, u) };
                (c, v, u, i)
            })
            .collect();
        keyed.sort_by_key(|&(c, v, u, _)| (std::cmp::Reverse(c), std::cmp::Reverse(v), u));
        keyed.into_iter().map(|(_, _, _, i)| i).collect()
    }

    /// The strike with the attackers in `order` (indices into `atts`).
    fn run(&mut self, order: &[usize]) -> Odds {
        let n_def = self.n_def;
        let n_att = order.len();
        let mut states: Vec<([u8; MAX_DEF], f64)> = vec![(self.init, 1.0)];
        let mut next: Vec<([u8; MAX_DEF], f64)> = Vec::new();
        let mut taken = 0.0f64;
        let mut taken_after = Vec::with_capacity(n_att);
        let mut exp_lost = 0.0f64;
        let mut dropped = 0.0f64;
        let mut best = 0;
        for &i in order {
            best = best.max(self.first_odds(i));
            if states.is_empty() {
                taken_after.push(taken);
                continue;
            }
            next.clear();
            for si in 0..states.len() {
                let (st, pr) = states[si];
                let (j, alive) = self.defender(i, &st);
                let mut s2 = st;
                // Collateral damage on the others, the healthiest first (CvUnit::collateralCombat
                // picks at random weighted by health).
                if alive >= 2 {
                    if let Some((per, max_units)) = &self.collat[i] {
                        let mut others: Vec<usize> = (0..n_def).filter(|&o| o != j && s2[o] > 0).collect();
                        others.sort_by_key(|&o| (std::cmp::Reverse(s2[o]), o));
                        for &o in others.iter().take((*max_units).max(0) as usize) {
                            let (dmg, cap) = per[o];
                            let damage = self.max_hp - s2[o] as i32;
                            let nd = damage.max((damage + dmg).min(cap));
                            s2[o] = (self.max_hp - nd).max(1) as u8;
                        }
                    }
                }
                let x = self.fight(i, j, st[j]);
                for &(hp, p_live, p_dead) in &self.memo_out[x].1 {
                    let p = pr * (p_live + p_dead);
                    if p <= 0.0 {
                        continue;
                    }
                    exp_lost += pr * p_dead;
                    let mut s3 = s2;
                    s3[j] = hp;
                    if s3[..n_def].iter().all(|&h| h == 0) {
                        taken += p;
                    } else {
                        self.canon(&mut s3);
                        next.push((s3, p));
                    }
                }
            }
            // Merge equal states; too many: round the defenders' health to coarser steps (2, 4,
            // .. 32 hit points, to the nearest) and merge again; still too many: keep the most
            // probable (the rest counts as holding). (Dropping the least probable states alone
            // lost a quarter of the mass on average against a few defenders, and the estimate
            // read 7% where the strikes took the city 60% of the time.)
            merge(&mut next, &mut states);
            let mut q = 2;
            while states.len() > MAX_STATES && q <= 32 {
                next.clear();
                for &(mut s, p) in &states {
                    for h in s[..n_def].iter_mut() {
                        if *h > 0 {
                            *h = ((*h as i32 + q / 2) / q * q).clamp(1, self.max_hp) as u8;
                        }
                    }
                    self.canon(&mut s);
                    next.push((s, p));
                }
                merge(&mut next, &mut states);
                q *= 2;
            }
            states.retain(|s| s.1 >= MIN_P);
            if states.len() > MAX_STATES {
                states.sort_unstable_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal).then(a.0.cmp(&b.0)));
                dropped += states[MAX_STATES..].iter().map(|s| s.1).sum::<f64>();
                states.truncate(MAX_STATES);
            }
            taken_after.push(taken);
        }
        let exp_hp_left = states.iter().map(|(s, p)| p * s[..n_def].iter().map(|&h| h as f64).sum::<f64>()).sum();
        Odds { p_take: taken, taken_after, exp_lost, exp_hp_left, best, n_def, n_att, dropped }
    }
}

/// Play the strike of `atts` against the defenders `defs` of plot `tp` over the distribution of
/// fight outcomes, the attackers in the order the strike sends them. `wall_cut`: city defense
/// points bombarded away. Only units able to attack count.
pub fn strike_odds(g: &Game, tp: PlotIdx, defs: &[UnitId], atts: &[UnitId], wall_cut: i32) -> Odds {
    let n_def = defs.len();
    if n_def == 0 {
        let n_att = atts.iter().filter(|&&u| attack_power(g, u) > 0 && g.unit_can_attack(u)).count();
        return Odds { p_take: 1.0, taken_after: vec![1.0; n_att], exp_lost: 0.0, exp_hp_left: 0.0, best: 1000, n_def, n_att, dropped: 0.0 };
    }
    let mut prep = Prep::new(g, tp, defs, atts, wall_cut);
    if prep.atts.is_empty() || n_def > MAX_DEF {
        let hp: i32 = defs.iter().map(|&d| g.unit_curr_hit_points(d)).sum();
        let n_att = prep.atts.len();
        return Odds { p_take: 0.0, taken_after: vec![0.0; n_att], exp_lost: 0.0, exp_hp_left: hp as f64, best: 0, n_def, n_att, dropped: 0.0 };
    }
    let order = prep.strike_order(g);
    prep.run(&order)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pair(a_str: i32, d_max: i32) -> Pair {
        Pair { a_str, a_fp: a_str, a_hp: 100, limit: 100, a_fs: 0, a_cfs: 0, a_immune: false, withdraw: 0, d_max, d_fs: 0, d_cfs: 0, d_immune: false, max_hp: 100 }
    }

    fn total(out: &Outcomes) -> f64 {
        out.iter().map(|o| o.1 + o.2).sum()
    }

    #[test]
    fn outcomes_sum_to_one() {
        for (a, d, hp) in [(400, 400, 100), (200, 600, 100), (600, 200, 40), (300, 500, 70)] {
            let (_, out) = outcomes(&pair(a, d), hp, 1000, 20);
            assert!((total(&out) - 1.0).abs() < 1e-6, "{a} {d} {hp}: {}", total(&out));
        }
        let mut k = pair(300, 500);
        k.a_fs = 1;
        k.d_cfs = 2;
        k.withdraw = 30;
        k.limit = 75;
        let (_, out) = outcomes(&k, 100, 1000, 20);
        assert!((total(&out) - 1.0).abs() < 1e-6, "{}", total(&out));
        // A siege engine's win leaves the defender at a quarter of its health.
        assert!(out.iter().any(|o| o.0 == 25));
        assert!(out.iter().all(|o| o.0 >= 25));
        // Withdrawal: 30% of the losses survive.
        let dead: f64 = out.iter().map(|o| o.2).sum();
        let live_loss: f64 = out.iter().filter(|o| o.0 != 25).map(|o| o.1).sum();
        assert!((live_loss / (live_loss + dead) - 0.3).abs() < 1e-6);
    }

    #[test]
    fn long_shots_wound_little() {
        // A weak attacker loses almost surely and takes only a few hit points off.
        let (odds, out) = outcomes(&pair(150, 600), 100, 1000, 20);
        assert!(odds < 20, "{odds}");
        let mean: f64 = out.iter().map(|o| (o.1 + o.2) * (100 - o.0 as i32) as f64).sum();
        assert!(mean < 25.0, "{mean}");
    }

    thread_local! {
        /// The kill probability of the last `outcomes` call by its own rounds.
        pub(super) static DP_WIN: std::cell::Cell<f64> = const { std::cell::Cell::new(-1.0) };
    }

    #[test]
    fn rounds_match_the_rules_odds() {
        // The round-by-round model agrees with the rules' getCombatOdds (first strikes, chance
        // first strikes, immunity, combat limit, wounded units).
        let mut n = 0;
        for a in [150, 250, 300, 420, 600] {
            for d in [200, 300, 450, 700] {
                for hp in [100, 63, 30] {
                    for (afs, acfs, dfs, dcfs, aimm) in [(0, 0, 0, 0, false), (1, 0, 0, 0, false), (0, 0, 1, 2, false), (2, 1, 1, 0, false), (0, 0, 2, 0, true)] {
                        for (limit, a_hp) in [(100, 100), (75, 100), (100, 55)] {
                            let mut k = pair(a, d);
                            k.a_hp = a_hp;
                            k.a_str = a * a_hp / 100;
                            k.a_fp = (a + k.a_str + 1) / 2;
                            k.a_fs = afs;
                            k.a_cfs = acfs;
                            k.d_fs = dfs;
                            k.d_cfs = dcfs;
                            k.a_immune = aimm;
                            k.limit = limit;
                            DP_WIN.with(|c| c.set(-1.0));
                            let (odds, _) = outcomes(&k, hp, 1000, 20);
                            let dp = DP_WIN.with(|c| c.get());
                            if dp < 0.0 {
                                continue;
                            }
                            n += 1;
                            assert!((dp * 1000.0 - odds as f64).abs() <= 6.0, "{k:?} hp {hp}: rules {odds}, rounds {dp}");
                        }
                    }
                }
            }
        }
        assert!(n > 500);
    }

    #[test]
    fn matches_mean_rounds() {
        // Even fight: the loser lands on average about as many hits as it takes minus one.
        let (odds, out) = outcomes(&pair(400, 400), 100, 1000, 20);
        assert!((450..=550).contains(&odds), "{odds}");
        let loss: f64 = out.iter().filter(|o| o.0 > 0).map(|o| o.1 + o.2).sum();
        assert!((loss - (1.0 - odds as f64 / 1000.0)).abs() < 1e-9);
    }

    /// A strike of `n_att` identical attackers on `n_def` identical defenders (no game needed).
    fn synthetic(n_def: usize, n_att: usize, a_str: i32, d_max: i32) -> Prep {
        let k = pair(a_str, d_max);
        let mut init = [0u8; MAX_DEF];
        for h in init.iter_mut().take(n_def) {
            *h = 100;
        }
        Prep {
            n_def,
            atts: (0..n_att).map(|i| UnitId(i as u32)).collect(),
            kinds: vec![vec![k; n_def]],
            kind_of: vec![0; n_att],
            ranks: vec![vec![d_max as i64; n_def]; n_att],
            collat: vec![None; n_att],
            groups: if n_def >= 2 { vec![(0..n_def).collect()] } else { Vec::new() },
            init,
            max_hp: 100,
            die: 1000,
            damage: 20,
            memo: std::collections::BTreeMap::new(),
            memo_out: Vec::new(),
        }
    }

    /// The same strike sampled: each fight drawn from its outcome distribution.
    fn monte_carlo(n_def: usize, n_att: usize, a_str: i32, d_max: i32, trials: u32) -> f64 {
        let k = pair(a_str, d_max);
        let mut seed: u64 = 12345;
        let mut taken = 0;
        for _ in 0..trials {
            let mut hp = vec![100i32; n_def];
            for _ in 0..n_att {
                // The healthiest defender (all equal in rank).
                let Some(j) = (0..n_def).filter(|&j| hp[j] > 0).max_by_key(|&j| (hp[j], std::cmp::Reverse(j))) else { break };
                let (_, out) = outcomes(&k, hp[j], 1000, 20);
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let mut x = (seed >> 11) as f64 / (1u64 << 53) as f64;
                for o in &out {
                    x -= o.1 + o.2;
                    if x <= 0.0 {
                        hp[j] = o.0 as i32;
                        break;
                    }
                }
            }
            if hp.iter().all(|&h| h == 0) {
                taken += 1;
            }
        }
        taken as f64 / trials as f64
    }

    #[test]
    fn strike_matches_sampling() {
        for (n_def, n_att, a, d) in [(1, 3, 300, 450), (3, 7, 400, 450), (5, 12, 380, 420), (8, 16, 500, 450)] {
            let mut prep = synthetic(n_def, n_att, a, d);
            let order: Vec<usize> = (0..n_att).collect();
            let o = prep.run(&order);
            let mc = monte_carlo(n_def, n_att, a, d, 20000);
            assert!((o.p_take - mc).abs() < 0.03, "{n_def} vs {n_att}: dp {} sampled {mc} (dropped {})", o.p_take, o.dropped);
            assert!(o.dropped < 0.01, "{}", o.dropped);
            assert!(o.taken_after.windows(2).all(|w| w[0] <= w[1] + 1e-12));
        }
    }
}
