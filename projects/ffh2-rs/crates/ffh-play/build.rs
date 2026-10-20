// Embeds the art toolkit (`web/art`, docs/ART.md 15-16) into the binary for the browser client:
// `$OUT_DIR/art.bundle.txt` (the sprite data in one text) and `$OUT_DIR/art_assets.rs` (the
// modules, each by `include_str!`), which `src/transport/http.rs` includes. The file lists come
// from `src/transport/art_files.rs`, which the server also uses for `--assets DIR`.
// Also embeds the isometric renderer's sprites (`assets/app/iso/*.png`, docs/ISO.md, written by
// `tools/m7/m7.py export`): `$OUT_DIR/iso_assets.rs`, every PNG there by `include_bytes!`.

use std::path::{Path, PathBuf};

include!("src/transport/art_files.rs");

fn main() {
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let art = manifest.join("web").join("art");
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/transport/art_files.rs");
    // A directory: cargo rescans every file under it.
    println!("cargo:rerun-if-changed=web/art");

    let bundle = art_bundle(&art).unwrap_or_else(|e| panic!("web/art: the sprite bundle: {e}"));
    write_if_changed(&out.join("art.bundle.txt"), &bundle);

    let modules = art_modules(&art).unwrap_or_else(|e| panic!("web/art: the modules: {e}"));
    let mut rs = String::from("/// The art modules (`web/art`), served at `/art/<path>`: `(path, content)`.\n");
    rs.push_str("pub const ART_MODULES: &[(&str, &str)] = &[\n");
    for m in &modules {
        let file = art.join(m);
        rs.push_str(&format!("    ({:?}, include_str!({:?})),\n", format!("art/{m}"), file.to_string_lossy()));
    }
    rs.push_str("];\n");
    write_if_changed(&out.join("art_assets.rs"), &rs);

    let iso = manifest.join("assets").join("app").join("iso");
    println!("cargo:rerun-if-changed=assets/app/iso");
    let mut pngs: Vec<String> = std::fs::read_dir(&iso)
        .unwrap_or_else(|e| panic!("{}: {e}", iso.display()))
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.ends_with(".png"))
        .collect();
    pngs.sort();
    let mut rs = String::from("/// The isometric renderer's sprites (`assets/app/iso/*.png`, docs/ISO.md): `(path, content type, bytes)`.\n");
    rs.push_str("pub const ASSET_BYTES: &[(&str, &str, &[u8])] = &[\n");
    for n in &pngs {
        rs.push_str(&format!("    ({:?}, PNG, include_bytes!({:?})),\n", format!("app/iso/{n}"), iso.join(n).to_string_lossy()));
    }
    rs.push_str("];\n");
    write_if_changed(&out.join("iso_assets.rs"), &rs);
}

fn write_if_changed(path: &Path, text: &str) {
    if std::fs::read_to_string(path).ok().as_deref() != Some(text) {
        std::fs::write(path, text).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    }
}
