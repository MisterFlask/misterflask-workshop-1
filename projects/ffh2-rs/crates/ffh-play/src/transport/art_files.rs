// The art toolkit's files the browser client loads (docs/ART.md 15-16, `crates/ffh-play/web/art`):
// its ES modules, served one by one under `/art/` (the modules import each other and the set
// passes by relative URL), and the sprite data in one text, `/art/art.bundle.txt` (the format of
// `web/art/tools/bundle.mjs`, which `loadArt({ bundle })` reads).
//
// Shared by `build.rs` (through `include!`, which embeds the files into the binary) and by the
// HTTP server (`--assets DIR` serves them from disk): std only, no paths into the crate.

/// The art modules a browser may import, relative to the art directory with `/` separators,
/// sorted: the toolkit's own (`*.js` in the directory) and each set's (`*.js` in the directories
/// that `sets.txt` lists, except their `preview.js`). Node tools (`tools/`, `*.mjs`) and the
/// preview pages are not part of the client.
pub fn art_modules(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let js = |d: &std::path::Path, prefix: &str| -> std::io::Result<Vec<String>> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(d)? {
            let e = e?;
            let name = e.file_name().to_string_lossy().into_owned();
            if e.file_type()?.is_file() && name.ends_with(".js") && name != "preview.js" {
                out.push(format!("{prefix}{name}"));
            }
        }
        Ok(out)
    };
    let mut out = js(dir, "")?;
    for set in art_sets(dir)? {
        out.extend(js(&dir.join(&set), &format!("{set}/"))?);
    }
    out.sort();
    Ok(out)
}

/// The sets of `sets.txt`, in load order (`;` starts a comment).
pub fn art_sets(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let text = std::fs::read_to_string(dir.join("sets.txt"))?;
    Ok(text.lines().map(art_strip_comment).filter(|l| !l.is_empty()).map(str::to_string).collect())
}

fn art_strip_comment(line: &str) -> &str {
    let l = line.strip_suffix('\r').unwrap_or(line);
    l.split(';').next().unwrap_or("").trim()
}

/// The files of the sprite bundle, in its order: `palette.txt`, `sets.txt`, `types.json`, then
/// each set's `manifest.txt` and the sources it lists (`source <file>`).
pub fn art_bundle_files(dir: &std::path::Path) -> std::io::Result<Vec<String>> {
    let mut files = vec!["palette.txt".to_string(), "sets.txt".to_string(), "types.json".to_string()];
    for set in art_sets(dir)? {
        let man = format!("{set}/manifest.txt");
        let text = std::fs::read_to_string(dir.join(&man))?;
        files.push(man);
        for line in text.lines().map(art_strip_comment) {
            let mut f = line.split_whitespace();
            if let (Some("source"), Some(src), None) = (f.next(), f.next(), f.next()) {
                files.push(format!("{set}/{src}"));
            }
        }
    }
    Ok(files)
}

/// The sprite bundle: each file of [`art_bundle_files`] after a line `@@file <path>`, its last
/// newline dropped, the whole ending with a newline (as `tools/bundle.mjs` writes it).
pub fn art_bundle(dir: &std::path::Path) -> std::io::Result<String> {
    let mut out = String::new();
    for f in art_bundle_files(dir)? {
        let text = std::fs::read_to_string(dir.join(&f))?;
        if text.lines().any(|l| l.starts_with("@@file ")) {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{f} has a line starting with '@@file '")));
        }
        out.push_str("@@file ");
        out.push_str(&f);
        out.push('\n');
        out.push_str(text.strip_suffix('\n').unwrap_or(&text));
        out.push('\n');
    }
    Ok(out)
}
