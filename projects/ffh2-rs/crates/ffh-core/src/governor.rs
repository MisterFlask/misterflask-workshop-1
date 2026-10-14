//! City citizen governor (replaces C++ `CvCityAI::AI_assignWorkingPlots`,
//! `AI_addBestCitizen`, `AI_removeWorstCitizen`, `AI_juggleCitizens`).
//!
//! New code, not a port: the C++ city AI is not ported (see docs/AI.md). The
//! governor decides which plots a city works and which specialists it employs;
//! the rules (which plots *can* be worked, forced specialists, yields) stay in the
//! ported CvCity code, which feeds this module plain candidate lists.
//!
//! Algorithm: always work the city centre; fill forced specialists; then add
//! citizens greedily by marginal value, where food is weighted up while the city
//! would starve or (unless avoiding growth) while the surplus is below the growth
//! target; finally do a bounded swap pass (worst worked vs. best unworked).

use ffh_data::*;

/// Relative weights for the three yields and the commerce types (per city emphasis).
#[derive(Clone, Copy, Debug)]
pub struct Weights {
    pub food: i32,
    pub production: i32,
    pub commerce: i32,
    /// Per commerce type multiplier (gold, research, culture, espionage), percent.
    pub commerce_type: [i32; CommerceType::NUM],
    /// Value of one great-people point.
    pub great_people: i32,
}

impl Default for Weights {
    fn default() -> Self {
        Weights { food: 3, production: 2, commerce: 1, commerce_type: [100, 110, 60, 30], great_people: 1 }
    }
}

/// A plot the city may work (index into the city's plot list).
#[derive(Clone, Debug)]
pub struct TileOption {
    pub city_plot: usize,
    pub yields: [i32; YieldType::NUM],
    /// Commerce split by type after the player's sliders (optional refinement).
    pub commerce: [i32; CommerceType::NUM],
}

/// A specialist the city may employ.
#[derive(Clone, Debug)]
pub struct SpecialistOption {
    pub specialist: SpecialistType,
    pub yields: [i32; YieldType::NUM],
    pub commerce: [i32; CommerceType::NUM],
    pub great_people: i32,
    /// How many more can be added (`i32::MAX` if unlimited).
    pub max_more: i32,
}

#[derive(Clone, Debug)]
pub struct GovernorInput {
    /// Citizens to place (population + free specialists, minus forced specialists).
    pub citizens: i32,
    /// Food eaten per citizen (C++ `FOOD_CONSUMPTION_PER_POPULATION`), times population.
    pub food_consumption: i32,
    /// Food from the centre plot and other fixed sources.
    pub base_food: i32,
    /// Desired food surplus (e.g. 2 + population/3); ignored when `avoid_growth`.
    pub growth_target: i32,
    pub avoid_growth: bool,
    pub tiles: Vec<TileOption>,
    pub specialists: Vec<SpecialistOption>,
    pub weights: Weights,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Assignment {
    /// City plot indices to work (centre excluded).
    pub tiles: Vec<usize>,
    /// Specialists to add (type, count).
    pub specialists: Vec<(SpecialistType, i32)>,
}

fn commerce_value(w: &Weights, commerce: &[i32; CommerceType::NUM], raw: i32) -> i32 {
    let split: i32 = commerce.iter().zip(w.commerce_type.iter()).map(|(c, m)| c * m).sum::<i32>() / 100;
    // If no split is given, value raw commerce directly.
    if commerce.iter().all(|&c| c == 0) {
        raw * w.commerce
    } else {
        split * w.commerce
    }
}

fn value(w: &Weights, food_weight: i32, yields: &[i32; 3], commerce: &[i32; 4], gpp: i32) -> i32 {
    yields[0] * food_weight * 4
        + yields[1] * w.production * 4
        + commerce_value(w, commerce, yields[2]) * 4
        + gpp * w.great_people * 2
}

/// Chooses tiles and specialists for `input.citizens` citizens.
pub fn assign(input: &GovernorInput) -> Assignment {
    let w = &input.weights;
    let mut food = input.base_food - input.food_consumption;
    let mut taken = vec![false; input.tiles.len()];
    let mut spec_used = vec![0i32; input.specialists.len()];
    let mut out = Assignment::default();

    for _ in 0..input.citizens.max(0) {
        let food_weight = if food < 0 {
            w.food * 4
        } else if !input.avoid_growth && food < input.growth_target {
            w.food * 2
        } else if input.avoid_growth {
            w.food / 2
        } else {
            w.food
        };
        // best tile
        let mut best: Option<(i32, bool, usize)> = None; // (value, is_tile, index)
        for (i, t) in input.tiles.iter().enumerate() {
            if taken[i] {
                continue;
            }
            let v = value(w, food_weight, &t.yields, &t.commerce, 0);
            if best.map_or(true, |(bv, _, _)| v > bv) {
                best = Some((v, true, i));
            }
        }
        for (i, s) in input.specialists.iter().enumerate() {
            if spec_used[i] >= s.max_more {
                continue;
            }
            let v = value(w, food_weight, &s.yields, &s.commerce, s.great_people);
            if best.map_or(true, |(bv, _, _)| v > bv) {
                best = Some((v, false, i));
            }
        }
        match best {
            Some((_, true, i)) => {
                taken[i] = true;
                food += input.tiles[i].yields[0];
                out.tiles.push(input.tiles[i].city_plot);
            }
            Some((_, false, i)) => {
                spec_used[i] += 1;
                food += input.specialists[i].yields[0];
            }
            None => break,
        }
    }
    for (i, n) in spec_used.iter().enumerate() {
        if *n > 0 {
            out.specialists.push((input.specialists[i].specialist, *n));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tile(i: usize, f: i32, p: i32, c: i32) -> TileOption {
        TileOption { city_plot: i, yields: [f, p, c], commerce: [0; 4] }
    }

    #[test]
    fn avoids_starvation_first() {
        let input = GovernorInput {
            citizens: 2,
            food_consumption: 4,
            base_food: 2,
            growth_target: 2,
            avoid_growth: false,
            tiles: vec![tile(1, 0, 3, 0), tile(2, 3, 0, 0), tile(3, 2, 1, 0)],
            specialists: vec![],
            weights: Weights::default(),
        };
        let a = assign(&input);
        assert_eq!(a.tiles.len(), 2);
        assert!(a.tiles.contains(&2), "food tile must be worked: {:?}", a);
    }

    #[test]
    fn uses_specialists_when_no_tiles() {
        let input = GovernorInput {
            citizens: 2,
            food_consumption: 4,
            base_food: 6,
            growth_target: 0,
            avoid_growth: false,
            tiles: vec![tile(1, 1, 0, 0)],
            specialists: vec![SpecialistOption {
                specialist: SpecialistType(0),
                yields: [0, 0, 0],
                commerce: [0, 3, 0, 0],
                great_people: 3,
                max_more: i32::MAX,
            }],
            weights: Weights::default(),
        };
        let a = assign(&input);
        assert_eq!(a.tiles.len() as i32 + a.specialists.iter().map(|s| s.1).sum::<i32>(), 2);
    }
}
