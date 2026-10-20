//! `ffh-play export-site OUT` (docs/WEB.md): the static site's client files, art and data pack,
//! written by the binary into a temporary directory.

mod common;

use std::process::Command;

use common::TempDir;
use ffh_play::site::{data_files, read_pack, BOOT_SCRIPT, PACK_MAGIC};
use ffh_play::transport::http::{ART_BUNDLE, ART_BUNDLE_PATH, ART_MODULES, ASSETS, ASSET_BYTES};

#[test]
fn export_site_writes_the_client_and_the_data_pack() {
    let tmp = TempDir::new("export-site");
    let out = tmp.0.join("site");
    let data = ffh_data::default_data_dir();
    let run = Command::new(env!("CARGO_BIN_EXE_ffh-play")).arg("export-site").arg(&out).arg("--data").arg(&data).output().expect("run ffh-play");
    assert!(run.status.success(), "{}", String::from_utf8_lossy(&run.stderr));
    assert!(String::from_utf8_lossy(&run.stdout).contains("files in"), "{}", String::from_utf8_lossy(&run.stdout));

    // The page loads the boot script on its own line, before the client.
    let index = std::fs::read_to_string(out.join("index.html")).unwrap();
    let lines: Vec<&str> = index.lines().collect();
    let boot = lines.iter().position(|l| l.trim() == BOOT_SCRIPT).expect("the boot script line");
    let main = lines.iter().position(|l| l.contains("app/main.js\"")).expect("the client's script");
    assert_eq!(boot + 1, main, "{index}");
    let embedded = ASSETS.iter().find(|(p, _, _)| *p == "index.html").unwrap().2;
    assert_eq!(index.replacen(&format!("{}\n", lines[boot]), "", 1), embedded, "only the boot line is added");
    // Every client file and the art, as served.
    for (path, _, body) in ASSETS.iter().filter(|(p, _, _)| *p != "index.html") {
        assert_eq!(std::fs::read_to_string(out.join(path)).unwrap(), *body, "{path}");
    }
    for (path, _, bytes) in ASSET_BYTES {
        assert_eq!(std::fs::read(out.join(path)).unwrap(), *bytes, "{path}");
    }
    for (path, body) in ART_MODULES {
        assert_eq!(std::fs::read_to_string(out.join(path)).unwrap(), *body, "{path}");
    }
    assert_eq!(std::fs::read_to_string(out.join(ART_BUNDLE_PATH)).unwrap(), ART_BUNDLE);

    // The pack: the header, then every data file, sorted, with its bytes.
    let pack = std::fs::read(out.join("web/data.pak")).unwrap();
    assert!(pack.starts_with(PACK_MAGIC));
    let files = read_pack(&pack).unwrap();
    let names: Vec<&str> = files.iter().map(|(p, _)| p.as_str()).collect();
    let expected = data_files(&data).unwrap();
    assert_eq!(names, expected);
    assert!(names.contains(&"text/en.tsv") && names.contains(&"xml/GlobalDefines.xml"), "{names:?}");
    for (path, bytes) in &files {
        assert_eq!(*bytes, std::fs::read(data.join(path)).unwrap().as_slice(), "{path}");
    }

    // A bad data directory fails before anything is written.
    let other = tmp.0.join("other");
    let run = Command::new(env!("CARGO_BIN_EXE_ffh-play")).arg("export-site").arg(&other).arg("--data").arg(tmp.0.join("nothing")).output().unwrap();
    assert_eq!(run.status.code(), Some(1));
    assert!(!other.exists());
}
