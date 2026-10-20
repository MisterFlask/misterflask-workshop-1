//! `ffh-play export-site OUT [--data DIR]` (docs/WEB.md): the files of the static site (GitHub
//! Pages) that come from this binary: the browser client as `ffh-play serve` serves it, and the
//! game data in one pack. `tools/web-site.sh` adds the engine (`crates/ffh-web`, WebAssembly) and
//! the page's scripts (`crates/ffh-web/site`).
//!
//! | File | What |
//! |---|---|
//! | `OUT/index.html` | the client's page, with [`BOOT_SCRIPT`] on its own line before the script tag that loads `app/main.js` |
//! | `OUT/app/**` | every client file ([`http::ASSETS`], [`http::ASSET_BYTES`]) |
//! | `OUT/art/**` | the art modules ([`http::ART_MODULES`]) and the sprite bundle ([`http::ART_BUNDLE_PATH`]) |
//! | `OUT/web/data.pak` | the game data ([`data_pack`]) |
//!
//! The data pack: `FFHPAK1\n`, then for every file under the data directory (sorted by its path
//! relative to it, `/` separators) `<path>\n<decimal byte length>\n<bytes>`. The page mounts each
//! file at `/data/<path>`.
//!
//! Files already in `OUT` are kept (the script writes the others there).

use std::path::{Path, PathBuf};

use crate::transport::http;

/// The page's script that runs the engine and routes `/api` to it (`crates/ffh-web/site/boot.js`).
pub const BOOT_SCRIPT: &str = r#"<script type="module" src="web/boot.js"></script>"#;

/// The data pack's header.
pub const PACK_MAGIC: &[u8] = b"FFHPAK1\n";

/// The pack's path in the site.
pub const PACK_PATH: &str = "web/data.pak";

/// The client's `index.html` for the site: [`BOOT_SCRIPT`] on its own line (with the same
/// indentation) before the line of the script tag whose `src` is `/app/main.js` or `app/main.js`.
/// A page that already has it is returned as it is.
pub fn site_index(html: &str) -> Result<String, String> {
    if html.contains(BOOT_SCRIPT) {
        return Ok(html.to_string());
    }
    let loads_main = |line: &str| line.contains("<script") && ["src=\"/app/main.js\"", "src=\"app/main.js\"", "src='/app/main.js'", "src='app/main.js'"].iter().any(|s| line.contains(s));
    let mut out = String::with_capacity(html.len() + BOOT_SCRIPT.len() + 8);
    let mut done = false;
    for line in html.split_inclusive('\n') {
        if !done && loads_main(line) {
            let indent: String = line.chars().take_while(|c| *c == ' ' || *c == '\t').collect();
            let eol = if line.ends_with("\r\n") { "\r\n" } else { "\n" };
            out.push_str(&indent);
            out.push_str(BOOT_SCRIPT);
            out.push_str(eol);
            done = true;
        }
        out.push_str(line);
    }
    if !done {
        return Err("index.html: no <script> tag loads app/main.js".into());
    }
    Ok(out)
}

/// Every file under `dir`: its path relative to `dir` (`/` separators), sorted.
pub fn data_files(dir: &Path) -> Result<Vec<String>, String> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
        let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        for e in entries {
            let path = e.map_err(|e| format!("{}: {e}", dir.display()))?.path();
            let meta = std::fs::metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
            if meta.is_dir() {
                walk(root, &path, out)?;
            } else if meta.is_file() {
                let rel = path.strip_prefix(root).map_err(|e| format!("{}: {e}", path.display()))?;
                let parts: Option<Vec<&str>> = rel.components().map(|c| c.as_os_str().to_str()).collect();
                let rel = parts.ok_or_else(|| format!("{}: the name is not UTF-8", path.display()))?.join("/");
                if rel.contains('\n') {
                    return Err(format!("{}: a newline in the name", path.display()));
                }
                out.push(rel);
            }
        }
        Ok(())
    }
    let mut files = Vec::new();
    walk(dir, dir, &mut files)?;
    files.sort();
    Ok(files)
}

/// The data pack of `dir` (see the module header).
pub fn data_pack(dir: &Path) -> Result<Vec<u8>, String> {
    let files = data_files(dir)?;
    if files.is_empty() {
        return Err(format!("{}: no data files", dir.display()));
    }
    let mut pack = PACK_MAGIC.to_vec();
    for rel in &files {
        let path = rel.split('/').fold(dir.to_path_buf(), |p, part| p.join(part));
        let bytes = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        pack.extend_from_slice(format!("{rel}\n{}\n", bytes.len()).as_bytes());
        pack.extend_from_slice(&bytes);
    }
    Ok(pack)
}

/// The files of a data pack: `(path, bytes)` in pack order.
pub fn read_pack(pack: &[u8]) -> Result<Vec<(String, &[u8])>, String> {
    let mut rest = pack.strip_prefix(PACK_MAGIC).ok_or("not a data pack (no FFHPAK1 header)")?;
    let mut files = Vec::new();
    let line = |rest: &mut &[u8]| -> Result<String, String> {
        let nl = rest.iter().position(|&b| b == b'\n').ok_or("a truncated entry")?;
        let s = std::str::from_utf8(&rest[..nl]).map_err(|_| "an entry head is not UTF-8")?.to_string();
        *rest = &rest[nl + 1..];
        Ok(s)
    };
    while !rest.is_empty() {
        let path = line(&mut rest)?;
        let len: usize = line(&mut rest)?.parse().map_err(|_| format!("{path}: a bad length"))?;
        if rest.len() < len {
            return Err(format!("{path}: truncated"));
        }
        files.push((path, &rest[..len]));
        rest = &rest[len..];
    }
    Ok(files)
}

/// What [`export_site`] wrote.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Exported {
    /// Files written (the pack included).
    pub files: usize,
    /// The data files in the pack, and its size.
    pub data_files: usize,
    pub pack_bytes: usize,
}

/// Writes the site's files that come from this binary into `out` (created), with the game data
/// of `data_dir` (see the module header).
pub fn export_site(out: &Path, data_dir: &Path) -> Result<Exported, String> {
    // The pack first: a missing data directory fails before anything is written.
    let pack = data_pack(data_dir)?;
    let mut files = 0;
    let mut write = |rel: &str, bytes: &[u8]| -> Result<(), String> {
        let path: PathBuf = rel.split('/').fold(out.to_path_buf(), |p, part| p.join(part));
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
        files += 1;
        Ok(())
    };
    let index = http::ASSETS.iter().find(|(p, _, _)| *p == "index.html").map(|(_, _, body)| *body).ok_or("no index.html among the client files")?;
    write("index.html", site_index(index)?.as_bytes())?;
    for (path, _, body) in http::ASSETS.iter().filter(|(p, _, _)| *p != "index.html") {
        write(path, body.as_bytes())?;
    }
    for (path, _, bytes) in http::ASSET_BYTES {
        write(path, bytes)?;
    }
    for (path, body) in http::ART_MODULES {
        write(path, body.as_bytes())?;
    }
    write(http::ART_BUNDLE_PATH, http::ART_BUNDLE.as_bytes())?;
    write(PACK_PATH, &pack)?;
    let data_files = read_pack(&pack)?.len();
    Ok(Exported { files, data_files, pack_bytes: pack.len() })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_boot_script_goes_before_main() {
        for src in ["/app/main.js", "app/main.js"] {
            let html = format!("<!doctype html>\n<head>\n  <link rel=\"stylesheet\" href=\"app/style.css\">\n  <script type=\"module\" src=\"{src}\"></script>\n</head>\n");
            let out = site_index(&html).unwrap();
            assert_eq!(out, html.replace("  <script type=\"module\" src", &format!("  {BOOT_SCRIPT}\n  <script type=\"module\" src")), "{src}");
            assert_eq!(site_index(&out).unwrap(), out, "inserted once");
        }
        assert!(site_index("<script src=\"other.js\"></script>\n").is_err());
        // The embedded page has the tag.
        let index = http::ASSETS.iter().find(|(p, _, _)| *p == "index.html").unwrap().2;
        assert!(site_index(index).unwrap().contains(BOOT_SCRIPT));
    }

    #[test]
    fn packs_round_trip() {
        let dir = std::env::temp_dir().join(format!("ffh-play-pack-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("xml/Units")).unwrap();
        std::fs::create_dir_all(dir.join("text")).unwrap();
        std::fs::write(dir.join("xml/Units/b.xml"), "<b/>\n").unwrap();
        std::fs::write(dir.join("xml/A.xml"), "").unwrap();
        std::fs::write(dir.join("text/en.tsv"), "K\tv\n").unwrap();
        let pack = data_pack(&dir).unwrap();
        assert_eq!(pack, b"FFHPAK1\ntext/en.tsv\n4\nK\tv\nxml/A.xml\n0\nxml/Units/b.xml\n5\n<b/>\n");
        let files = read_pack(&pack).unwrap();
        assert_eq!(files, vec![("text/en.tsv".to_string(), &b"K\tv\n"[..]), ("xml/A.xml".to_string(), &b""[..]), ("xml/Units/b.xml".to_string(), &b"<b/>\n"[..])]);
        assert!(read_pack(&pack[..pack.len() - 1]).is_err());
        assert!(read_pack(b"FFHPAK2\n").is_err());
        std::fs::remove_dir_all(&dir).unwrap();
        assert!(data_pack(&dir).is_err());
    }
}
