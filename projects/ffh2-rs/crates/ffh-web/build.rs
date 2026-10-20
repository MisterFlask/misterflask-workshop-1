// The rules recurse deeply (paths, AI): the engine gets the stack `ffh-play serve`'s game thread
// has (16 MiB) instead of wasm-ld's default 1 MiB.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_ARCH").as_deref() == Ok("wasm32") {
        println!("cargo:rustc-link-arg=-zstack-size=16777216");
    }
}
