//! T13 (design §8): performance budgets, `#[ignore]`d (timings depend on the host; run it with
//! `cargo test --release -p ffh-play --test bench -- --ignored --nocapture`, or `ffh-play bench`).
//! See `ffh_play::bench` for what is measured; `tests/screens.rs` runs the quick variant for
//! correctness (the same game three ways) in every test run.

use std::path::Path;

use ffh_play::bench::{render, run, BenchOptions};

#[test]
#[ignore]
fn t13_performance() {
    // `ffh bench` is the workspace's `ffh` binary, next to this crate's.
    let ffh = Path::new(env!("CARGO_BIN_EXE_ffh-play")).with_file_name("ffh");
    let o = BenchOptions { ffh: Some(ffh), verbose: true, ..BenchOptions::default() };
    let r = run(&o);
    eprint!("{}", render(&o, &r));
    let failures = r.failures();
    assert!(failures.is_empty(), "T13 budgets missed: {failures:#?}\n{}", render(&o, &r));
}
