//! CvFractal parity: replays the 8 configurations of `tools/parity/parity_main.cpp` (compiled
//! from the original CvFractal.cpp + CvRandom.cpp) and compares every value of
//! `tests/fixtures/cvfractal.txt`: `getHeightFromPercent(0, 5, .., 100)`, `getHeight(x, y)` for
//! the whole map, and the RNG seed afterwards.

use ffh_core::mapgen::fractal::{Fractal, FRAC_WRAP_X, FRAC_WRAP_Y};
use ffh_core::rng::Rng;
use ffh_core::utils::range;

struct Block {
    header: String,
    xs: i32,
    ys: i32,
    grain: i32,
    flags: i32,
    xe: i32,
    ye: i32,
    seed: u32,
    hints: bool,
    rifts: bool,
    pct: Vec<i32>,
    rows: Vec<Vec<i32>>,
    rng_end: u32,
}

fn kv(header: &str, key: &str) -> i64 {
    let prefix = format!("{key}=");
    header
        .split_whitespace()
        .find_map(|t| t.strip_prefix(prefix.as_str()))
        .unwrap_or_else(|| panic!("{key} missing in {header:?}"))
        .parse()
        .unwrap()
}

fn nums(s: &str) -> Vec<i32> {
    s.split_whitespace().map(|t| t.parse().unwrap()).collect()
}

fn parse(text: &str) -> Vec<Block> {
    let mut lines = text.lines().peekable();
    let mut blocks = Vec::new();
    while let Some(header) = lines.next() {
        assert!(header.starts_with("cfg "), "unexpected line {header:?}");
        let ys = kv(header, "ys") as i32;
        let pct_line = lines.next().unwrap();
        let pct = nums(pct_line.strip_prefix("pct:").expect("pct line"));
        let rows = (0..ys).map(|_| nums(lines.next().unwrap())).collect();
        let end_line = lines.next().unwrap();
        let rng_end = end_line.strip_prefix("rng_end=").expect("rng_end line").trim().parse().unwrap();
        blocks.push(Block {
            header: header.to_string(),
            xs: kv(header, "xs") as i32,
            ys,
            grain: kv(header, "grain") as i32,
            flags: kv(header, "flags") as i32,
            xe: kv(header, "xe") as i32,
            ye: kv(header, "ye") as i32,
            seed: kv(header, "seed") as u32,
            hints: kv(header, "hints") != 0,
            rifts: kv(header, "rifts") != 0,
            pct,
            rows,
            rng_end,
        });
    }
    blocks
}

#[test]
fn cvfractal_matches_cpp_reference() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/cvfractal.txt");
    let text = std::fs::read_to_string(path).expect("fixture");
    let blocks = parse(&text);
    assert_eq!(blocks.len(), 8);

    for b in &blocks {
        let mut r = Rng::new(b.seed);
        let mut f = Fractal::new();
        let mut rifts = Fractal::new();
        if b.rifts {
            rifts.frac_init(b.xs, b.ys, 1, &mut r, b.flags, None, b.xe, b.ye);
        }
        if b.hints {
            let min_exp = b.xe.min(b.ye);
            let smooth = range(min_exp - b.grain, 0, min_exp);
            let w = (1 << (b.xe - smooth)) + if b.flags & FRAC_WRAP_X != 0 { 0 } else { 1 };
            let h = (1 << (b.ye - smooth)) + if b.flags & FRAC_WRAP_Y != 0 { 0 } else { 1 };
            let hints: Vec<u8> = (0..w * h).map(|i| ((i * 37 + 11) % 256) as u8).collect();
            f.frac_init_hinted(b.xs, b.ys, b.grain, &mut r, &hints, b.flags, None, b.xe, b.ye);
        } else {
            f.frac_init(b.xs, b.ys, b.grain, &mut r, b.flags, if b.rifts { Some(&rifts) } else { None }, b.xe, b.ye);
        }

        let pct: Vec<i32> = (0..=100).step_by(5).map(|p| f.height_from_percent(p)).collect();
        assert_eq!(pct, b.pct, "{}: getHeightFromPercent", b.header);

        assert_eq!(b.rows.len(), b.ys as usize);
        for y in 0..b.ys {
            let row: Vec<i32> = (0..b.xs).map(|x| f.height(x, y)).collect();
            assert_eq!(row, b.rows[y as usize], "{}: getHeight row y={}", b.header, y);
        }

        assert_eq!(r.seed(), b.rng_end, "{}: rng seed after", b.header);
    }
}
