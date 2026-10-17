//! The browser client's files (design §7 P5): every file of `assets/` is embedded and served
//! with its type, the page and the modules only reference embedded files (a module that is
//! added but not listed in `http::ASSETS`, or an import of a missing one, fails here, without a
//! browser), and the staged situations the browser smoke test uses (`ffh-play stage`) hold
//! what they promise. The client's own checks need node and a browser and are not part of
//! `cargo test`: `tools/web-check.sh`, `tools/web-api-smoke.mjs`, `tools/web-smoke.mjs`.

mod common;

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;

use common::TempDir;
use ffh_play::json::Json;
use ffh_play::transport::art_files::{art_bundle, art_modules};
use ffh_play::transport::http::{route, HttpCtx, HttpRequest, ART_BUNDLE, ART_BUNDLE_PATH, ART_MODULES, ASSETS, ASSET_BYTES};
use ffh_play::transport::shared::Shared;
use ffh_play::{Session, SessionOptions};

fn assets_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets")
}

fn art_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("web").join("art")
}

/// The files a client may load: `index.html` and everything under `app/` (recursively; the
/// isometric renderer keeps its PNG sprites in `app/iso/`).
fn files_on_disk() -> Vec<String> {
    let mut out = vec!["index.html".to_string()];
    let app = assets_dir().join("app");
    let mut stack = vec![app.clone()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(format!("app/{}", p.strip_prefix(&app).unwrap().to_string_lossy().replace('\\', "/")));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn every_client_file_is_embedded_as_it_is_on_disk() {
    let disk = files_on_disk();
    let mut embedded: Vec<String> =
        ASSETS.iter().map(|(p, _, _)| p.to_string()).chain(ASSET_BYTES.iter().map(|(p, _, _)| p.to_string())).collect();
    embedded.sort();
    let mut want = disk.clone();
    want.sort();
    assert_eq!(embedded, want, "http::ASSETS + ASSET_BYTES must list exactly index.html and assets/app/**");
    for (path, ctype, body) in ASSETS {
        let on_disk = std::fs::read_to_string(assets_dir().join(path)).unwrap();
        assert_eq!(*body, on_disk, "{path}");
        let want = match path.rsplit_once('.').map(|(_, e)| e) {
            Some("html") => "text/html",
            Some("js") => "text/javascript",
            Some("css") => "text/css",
            other => panic!("unexpected client file type {other:?} ({path})"),
        };
        assert!(ctype.starts_with(want), "{path}: {ctype}");
    }
}

/// The art (`web/art`, docs/ART.md 15-16) is embedded as it is on disk: every module of the
/// toolkit and its sets (`build.rs`), and the sprite bundle, which is what
/// `web/art/tools/bundle.mjs` writes (palette, set list, types.json, manifests, sources).
#[test]
fn the_art_is_embedded_as_it_is_on_disk() {
    let want: Vec<String> = art_modules(&art_dir()).unwrap().into_iter().map(|m| format!("art/{m}")).collect();
    let embedded: Vec<String> = ART_MODULES.iter().map(|(p, _)| p.to_string()).collect();
    assert_eq!(embedded, want, "build.rs embeds the art's modules");
    for need in ["art/art.js", "art/compose.js", "art/library.js", "art/mapping.js", "art/spritefmt.js", "art/ui/ui.js", "art/terrain/ground.js", "art/units/stack.js", "art/cities/citylight.js"] {
        assert!(want.iter().any(|m| m == need), "{need} is not among the art's modules: {want:?}");
    }
    for (path, body) in ART_MODULES {
        let on_disk = std::fs::read_to_string(art_dir().join(path.strip_prefix("art/").unwrap())).unwrap();
        assert_eq!(*body, on_disk, "{path}");
    }
    assert_eq!(ART_BUNDLE, art_bundle(&art_dir()).unwrap());
    let files: Vec<&str> = ART_BUNDLE.lines().filter_map(|l| l.strip_prefix("@@file ")).collect();
    assert_eq!(&files[..3], ["palette.txt", "sets.txt", "types.json"]);
    for set in ["terrain", "units", "cities", "ui"] {
        assert!(files.contains(&format!("{set}/manifest.txt").as_str()), "{files:?}");
        // every sprite source the set's manifest names travels in the bundle
        let manifest = std::fs::read_to_string(art_dir().join(set).join("manifest.txt")).unwrap();
        let sources: Vec<&str> = manifest.lines().filter_map(|l| l.trim().strip_prefix("source ")).map(str::trim).collect();
        assert!(!sources.is_empty(), "{set}/manifest.txt names no sources");
        for src in sources {
            assert!(files.contains(&format!("{set}/{src}").as_str()), "{set}/{src} is not in the bundle: {files:?}");
        }
    }
}

/// The module specifiers a module imports: `import ... from "x"`, `export ... from "x"`, the
/// last line of a multi-line import (`} from "x"`), `import "x"`, and the module of a worker
/// (`new Worker(new URL("x", import.meta.url), ..)`).
fn imports(src: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in src.lines() {
        let l = line.trim_start();
        if let Some(i) = l.find("new Worker(new URL(") {
            let s = &l[i + "new Worker(new URL(".len()..];
            if let Some(q) = s.chars().next().filter(|c| *c == '"' || *c == '\'') {
                if let Some(end) = s[1..].find(q) {
                    out.push(s[1..1 + end].to_string());
                }
            }
            continue;
        }
        // A module loaded on demand (`await import("./r3d/map3d.js")`, the 3D renderer).
        if let Some(i) = l.find("await import(") {
            let s = &l[i + "await import(".len()..];
            if let Some(q) = s.chars().next().filter(|c| *c == '"' || *c == '\'') {
                if let Some(end) = s[1..].find(q) {
                    out.push(s[1..1 + end].to_string());
                }
            }
            continue;
        }
        let spec = if l.starts_with("import ") || l.starts_with("export ") || l.starts_with('}') {
            match l.rfind(" from ") {
                Some(i) => &l[i + 6..],
                None if l.starts_with("import ") => &l[7..],
                None => continue,
            }
        } else {
            continue;
        };
        let s = spec.trim();
        let Some(q) = s.chars().next().filter(|c| *c == '"' || *c == '\'') else { continue };
        if let Some(end) = s[1..].find(q) {
            out.push(s[1..1 + end].to_string());
        }
    }
    out
}

/// A module specifier resolved against the importing module's path (`app/main.js` +
/// `../art/art.js` = `art/art.js`), or None when it leaves the server's root or is not relative.
fn resolve(from: &str, spec: &str) -> Option<String> {
    if !(spec.starts_with("./") || spec.starts_with("../")) {
        return None;
    }
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for p in spec.split('/') {
        match p {
            "." => {}
            ".." => {
                parts.pop()?;
            }
            p => parts.push(p),
        }
    }
    Some(parts.join("/"))
}

/// The set passes art.js imports by name (`PASSES`: `['./terrain/ground.js', 'installTerrain']`).
fn passes(art_js: &str) -> Vec<String> {
    let start = art_js.find("export const PASSES = [").expect("art.js PASSES");
    let end = start + art_js[start..].find("];").unwrap();
    art_js[start..end].lines().filter_map(|l| l.trim().strip_prefix("['").and_then(|r| r.split('\'').next())).map(str::to_string).collect()
}

#[test]
fn the_page_and_its_modules_reference_only_embedded_files() {
    let get = |p: &str| ASSETS.iter().find(|(q, _, _)| *q == p).map(|(_, _, b)| *b).or_else(|| ART_MODULES.iter().find(|(q, _)| *q == p).map(|(_, b)| *b));
    let index = get("index.html").unwrap();
    // The page loads the stylesheet and the main module, and nothing from another site.
    let mut refs = Vec::new();
    for attr in ["src=\"", "href=\""] {
        let mut s = index;
        while let Some(i) = s.find(attr) {
            s = &s[i + attr.len()..];
            let end = s.find('"').unwrap();
            refs.push(s[..end].to_string());
            s = &s[end..];
        }
    }
    assert!(refs.contains(&"/app/main.js".to_string()) && refs.contains(&"/app/style.css".to_string()), "{refs:?}");
    for r in &refs {
        if r.starts_with("data:") {
            continue;
        }
        let path = r.strip_prefix('/').unwrap_or_else(|| panic!("page reference {r} is not a server path"));
        assert!(get(path).is_some(), "the page references {r}, which is not embedded");
    }
    // Every module reached from main.js (and the art's set passes, which art.js imports by
    // name) is embedded; every embedded module is reached.
    let art_js = get("art/art.js").unwrap();
    let pass_modules: Vec<String> = passes(art_js).iter().map(|p| resolve("art/art.js", p).unwrap()).collect();
    assert_eq!(pass_modules, ["art/terrain/ground.js", "art/units/stack.js", "art/cities/citylight.js"]);
    let mut seen = vec!["app/main.js".to_string()];
    let mut queue = vec!["app/main.js".to_string()];
    while let Some(m) = queue.pop() {
        let src = get(&m).unwrap_or_else(|| panic!("{m} is imported but not embedded"));
        let mut next: Vec<String> = Vec::new();
        for spec in imports(src) {
            let path = resolve(&m, &spec).unwrap_or_else(|| panic!("{m} imports {spec:?}: only relative modules of this server (no CDN, no bare names)"));
            assert!(path.starts_with("app/") || path.starts_with("art/"), "{m} imports {spec}, outside app/ and art/");
            next.push(path);
        }
        if m == "art/art.js" {
            next.extend(pass_modules.iter().cloned());
        }
        for path in next {
            assert!(get(&path).is_some(), "{m} imports {path}, which is not embedded");
            if !seen.contains(&path) {
                seen.push(path.clone());
                queue.push(path);
            }
        }
    }
    for p in ASSETS.iter().map(|(p, _, _)| *p).chain(ART_MODULES.iter().map(|(p, _)| *p)) {
        if p.ends_with(".js") {
            assert!(seen.iter().any(|s| s == p), "{p} is embedded but no module imports it");
        }
    }
    assert_eq!(
        imports("import { a } from \"./x.js\";\nimport * as b from './y.js';\nimport {\n  c,\n} from \"./z.js\";\nimport \"./w.js\";\nconst s = \"a from b\";\n    this.worker = new Worker(new URL(\"./v.js\", import.meta.url), { type: \"module\" });"),
        ["./x.js", "./y.js", "./z.js", "./w.js", "./v.js"]
    );
    assert_eq!(resolve("app/main.js", "../art/ui/ui.js").as_deref(), Some("art/ui/ui.js"));
    assert_eq!(resolve("art/terrain/ground.js", "../art.js").as_deref(), Some("art/art.js"));
    assert_eq!(resolve("app/main.js", "../../x.js"), None);
    assert_eq!(resolve("app/main.js", "x.js"), None);
}

#[test]
fn client_files_are_served_with_their_types() {
    let (tx, _rx) = mpsc::channel();
    let ctx = HttpCtx { shared: Arc::new(Shared::new()), tx, assets: None, loopback: None };
    let get = |path: &str| {
        let req = HttpRequest { method: "GET".into(), path: path.into(), query: String::new(), headers: vec![("Host".into(), "127.0.0.1".into())], body: Vec::new() };
        route(&req, &ctx)
    };
    for (path, ctype, _) in ASSETS {
        let r = get(&format!("/{path}"));
        assert_eq!((r.status, r.content_type), (200, *ctype), "{path}");
    }
    let root = get("/");
    assert_eq!((root.status, root.content_type), (200, "text/html; charset=utf-8"));
    assert_eq!(get("/app/missing.js").status, 404);
    assert_eq!(get("/app/../index.html").status, 404);
    // The art: its modules and the sprite bundle; not its tools, previews or sources one by one.
    for (path, _) in ART_MODULES {
        let r = get(&format!("/{path}"));
        assert_eq!((r.status, r.content_type), (200, "text/javascript; charset=utf-8"), "{path}");
    }
    let r = get(&format!("/{ART_BUNDLE_PATH}"));
    assert_eq!((r.status, r.content_type), (200, "text/plain; charset=utf-8"));
    for missing in ["/art/tools/bundle.mjs", "/art/terrain/preview.js", "/art/palette.txt", "/art/preview/scene.js", "/art/../Cargo.toml"] {
        assert_eq!(get(missing).status, 404, "{missing}");
    }
    // `--assets DIR` serves the same layout from disk.
    let ctx = HttpCtx { shared: Arc::new(Shared::new()), tx: mpsc::channel().0, assets: Some(assets_dir()), loopback: None };
    let req = HttpRequest { method: "GET".into(), path: "/app/main.js".into(), query: String::new(), headers: Vec::new(), body: Vec::new() };
    let r = route(&req, &ctx);
    assert_eq!((r.status, r.content_type), (200, "text/javascript; charset=utf-8"));
    // ... and the art from `DIR/../web/art`.
    let req = |path: &str| HttpRequest { method: "GET".into(), path: path.into(), query: String::new(), headers: Vec::new(), body: Vec::new() };
    let r = route(&req("/art/compose.js"), &ctx);
    assert_eq!((r.status, r.content_type), (200, "text/javascript; charset=utf-8"));
    assert_eq!(route(&req("/art/art.bundle.txt"), &ctx).status, 200);
    assert_eq!(route(&req("/art/tools/validate.mjs"), &ctx).status, 404);
}

/// `ffh-play stage attack`: the warrior can attack the barbarian next to it, and the `combat`
/// and `path` queries the client shows say so.
#[test]
fn the_staged_attack_shows_odds() {
    let tmp = TempDir::new("web-stage-attack");
    let st = ffh_play::stage::attack(&tmp.0).expect("staged attack");
    let (bytes, sidecar) = ffh_play::save::read_bundle(&st.save).unwrap();
    let s = Session::load(&bytes, sidecar.as_ref(), Arc::new(ffh_play::human::AdvisorAnswerer), SessionOptions::default()).unwrap();
    let q = |j: Json| s.query_json(&j).unwrap();
    let c = q(ffh_play::jobj! { "q" => "combat", "units" => vec![st.unit], "x" => st.x, "y" => st.y });
    assert_eq!(c.get("valid").and_then(Json::as_bool), Some(true), "{c}");
    let odds: i64 = c.req("odds").unwrap();
    assert!((1..1000).contains(&odds), "{c}");
    assert_eq!(c.req::<i64>("defender").unwrap(), st.enemy.0 as i64);
    let p = q(ffh_play::jobj! { "q" => "path", "units" => vec![st.unit], "x" => st.x, "y" => st.y });
    assert_eq!((p.get("ok").and_then(Json::as_bool), p.get("attack").and_then(Json::as_bool)), (Some(true), Some(true)), "{p}");
    assert_eq!(p.get("declare_war_team"), Some(&Json::Null), "barbarians are always at war: no question");
    let json = st.to_json();
    assert_eq!(json.req_str("stage"), Ok("attack"));
}

/// `stage trade` (P6): a civilization met at peace, gold both ways in the trade tables, and a gift
/// of gold accepted (the browser smoke test's trade screen).
#[test]
fn the_staged_trade_has_gold_both_ways() {
    let tmp = TempDir::new("web-stage-trade");
    let st = ffh_play::stage::trade(&tmp.0).expect("staged trade");
    assert_eq!(files_under(&tmp.0), ["trade.ffhsave", "trade.ffhsess"]);
    let (bytes, sidecar) = ffh_play::save::read_bundle(&st.save).unwrap();
    let mut s = Session::load(&bytes, sidecar.as_ref(), Arc::new(ffh_play::human::AdvisorAnswerer), SessionOptions { journal: false, ..SessionOptions::default() }).unwrap();
    let t = s.query_json(&ffh_play::jobj! { "q" => "trade", "with" => st.with }).unwrap();
    let gold_item = ffh_data::TradeableItem::TRADE_GOLD.0 as i64;
    let gold = |side: &str| t.req_arr(side).unwrap().iter().find(|i| i.req::<i64>("item") == Ok(gold_item)).cloned();
    let ours = gold("we_give").expect("our gold in the table");
    assert!(gold("they_give").is_some(), "{t}");
    assert_eq!(ours.req::<i64>("max"), Ok(100), "{ours}");
    let before = s.g.players[s.human].gold;
    let item = ffh_play::jobj! { "item" => gold_item, "data" => 7 };
    let deal = ffh_play::jobj! { "c" => "propose_deal", "to" => st.with, "give" => Json::Arr(vec![item]), "get" => Json::Arr(Vec::new()) };
    let c = ffh_play::command::Command::from_json(s.g.d, &deal).unwrap();
    match s.execute(&c) {
        Ok(ffh_play::command::CmdOk::Applied { accepted, .. }) => assert_eq!(accepted, Some(true)),
        other => panic!("the gift: {other:?}"),
    }
    assert_eq!(s.g.players[s.human].gold, before - 7);
    assert_eq!(st.to_json().req_str("stage"), Ok("trade"));
}

/// The files a directory holds, relative, sorted.
fn files_under(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p);
            } else {
                out.push(p.strip_prefix(dir).unwrap().to_string_lossy().replace('\\', "/"));
            }
        }
    }
    out.sort();
    out
}

/// `ffh-play stage load`: the warrior's `load` action asks which of the two triremes (the
/// LOADUNIT picker), and the answer the client sends (`do_command COMMAND_LOAD_UNIT`, `data1` =
/// the transport's owner, `data2` = its id, P5 review) loads it. A stage writes only its save
/// and sidecar into the directory (no journal, no autosaves: P5 review), so a game directory
/// keeps its own.
#[test]
fn the_staged_load_asks_which_transport() {
    let tmp = TempDir::new("web-stage-load");
    let st = ffh_play::stage::load(&tmp.0).expect("staged load");
    assert_eq!(files_under(&tmp.0), ["load.ffhsave", "load.ffhsess"]);
    assert_eq!(st.transports.len(), 2);
    let (bytes, sidecar) = ffh_play::save::read_bundle(&st.save).unwrap();
    let mut s = Session::load(&bytes, sidecar.as_ref(), Arc::new(ffh_play::human::AdvisorAnswerer), SessionOptions { journal: false, ..SessionOptions::default() }).unwrap();
    let g = s.query_json(&ffh_play::jobj! { "q" => "group", "units" => vec![st.unit] }).unwrap();
    let load = g.req_arr("actions").unwrap().iter().find(|a| a.req_str("kind") == Ok("load")).cloned().expect("a load action");
    assert_eq!(load.get("enabled").and_then(Json::as_bool), Some(true), "{load}");
    let transports: Vec<i64> = load.req_arr("transports").unwrap().iter().map(|t| t.as_i64().unwrap()).collect();
    assert_eq!(transports, st.transports.iter().map(|t| t.0 as i64).collect::<Vec<_>>());
    let exec = |s: &mut Session, j: &Json| ffh_play::command::Command::from_json(s.g.d, j).and_then(|c| s.execute(&c));
    match exec(&mut s, load.get("cmd").unwrap()) {
        Ok(ffh_play::command::CmdOk::NeedsConfirm(c)) => assert_eq!(c.to_json().req_str("kind"), Ok("load_unit")),
        other => panic!("load: {other:?}"),
    }
    let second = st.transports[1];
    let owner = s.g.units[second].owner.0 as i64;
    let load_unit = |data1: i64, data2: i64| {
        ffh_play::jobj! { "c" => "do_command", "units" => vec![st.unit], "command" => "COMMAND_LOAD_UNIT", "data1" => data1, "data2" => data2, "alt" => false, "confirmed" => false }
    };
    // The id as `data1` (the P5 client's mistake) is refused.
    assert!(exec(&mut s, &load_unit(second.0 as i64, -1)).is_err());
    match exec(&mut s, &load_unit(owner, second.0 as i64)) {
        Ok(ffh_play::command::CmdOk::Applied { .. }) => {}
        other => panic!("load into {second:?}: {other:?}"),
    }
    assert_eq!(s.g.units[st.unit].transport_unit, Some(second));
    assert_eq!(st.to_json().req_str("stage"), Ok("load"));
}
