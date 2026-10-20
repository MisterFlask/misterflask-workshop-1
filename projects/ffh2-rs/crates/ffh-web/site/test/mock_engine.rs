// A stand-in for the engine (crates/ffh-web) with the same ABI, for engine.test.mjs: it answers
// a few test routes so the page side (boot.js EngineClient, engine.js workerMain, wasi.js) can be
// driven end to end without the game. Not part of the cargo workspace:
//   rustc --edition 2021 -O --target wasm32-wasip1 --crate-type cdylib mock_engine.rs -o mock.wasm
//
// Routes: GET /api/echo (the request back), GET /api/etag (ETag "v1", 304 on If-None-Match),
// GET /api/view (409 without a game); POST /api (needs x-ffh: 1) with `{"id", "op", ...}`:
// ping, new (a game: phase human), write / read (a file under /saves/g), slow (n slices, serving
// pulled requests between them), block (a blocking decision: pulled requests until an `answer`
// POST), panic; anything else 404 / 405 as `http::route`.
use std::cell::RefCell;

#[link(wasm_import_module = "ffh")]
extern "C" {
    fn reply(token: u32, ptr: *const u8, len: u32);
    fn publish(ptr: *const u8, len: u32);
    fn pull(block: u32) -> i32;
    fn pull_copy(ptr: *mut u8);
    fn crash(ptr: *const u8, len: u32);
}

struct State {
    version: u64,
    phase: &'static str,
    game: Option<String>,
    data: String,
}

thread_local! {
    static ST: RefCell<State> = const { RefCell::new(State { version: 0, phase: "no_game", game: None, data: String::new() }) };
}

fn q(s: &str) -> String {
    let mut o = String::from("\"");
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

fn send(token: u32, status: u16, headers: &[(&str, &str)], body: &str) {
    let h: Vec<String> = headers.iter().map(|(k, v)| format!("[{},{}]", q(k), q(v))).collect();
    let frame = format!("{{\"s\":{status},\"ct\":\"application/json; charset=utf-8\",\"h\":[{}]}}\n{body}", h.join(","));
    unsafe { reply(token, frame.as_ptr(), frame.len() as u32) }
}

fn announce() {
    let text = ST.with(|s| {
        let mut s = s.borrow_mut();
        s.version += 1;
        let game = match &s.game {
            Some(g) => format!("{{\"dir\":\"/saves/{g}\",\"journal\":\"/saves/{g}/journal.jsonl\",\"autosave\":\"/saves/{g}/auto/T0001.ffhsave\"}}"),
            None => "null".to_string(),
        };
        format!("{{\"version\":{},\"phase\":{},\"ms_in_phase\":5,\"game\":{game},\"data\":{}}}", s.version, q(s.phase), q(&s.data))
    });
    unsafe { publish(text.as_ptr(), text.len() as u32) }
}

fn set_phase(p: &'static str) {
    ST.with(|s| s.borrow_mut().phase = p);
    announce();
}

/// A member of a flat JSON object: a string (escapes kept) or a number.
fn field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let pat = format!("{}:", q(key));
    let at = json.find(&pat)? + pat.len();
    let rest = json[at..].trim_start();
    if let Some(r) = rest.strip_prefix('"') {
        let mut esc = false;
        for (i, c) in r.char_indices() {
            match c {
                '\\' if !esc => esc = true,
                '"' if !esc => return Some(&r[..i]),
                _ => esc = false,
            }
        }
        None
    } else {
        let end = rest.find(|c: char| !(c.is_ascii_digit() || c == '-')).unwrap_or(rest.len());
        Some(&rest[..end])
    }
}

struct Req {
    token: u32,
    head: String,
    method: String,
    path: String,
    query: String,
    body: String,
}

fn parse(frame: &[u8]) -> Req {
    let nl = frame.iter().position(|&b| b == b'\n').unwrap_or(frame.len());
    let head = String::from_utf8_lossy(&frame[..nl]).into_owned();
    let body = String::from_utf8_lossy(frame.get(nl + 1..).unwrap_or(&[])).into_owned();
    Req {
        token: field(&head, "t").and_then(|t| t.parse().ok()).unwrap_or(0),
        method: field(&head, "m").unwrap_or("").to_string(),
        path: field(&head, "p").unwrap_or("").to_string(),
        query: field(&head, "q").unwrap_or("").to_string(),
        head,
        body,
    }
}

fn pull_one(block: bool) -> Result<Option<Req>, ()> {
    let n = unsafe { pull(block as u32) };
    if n < 0 {
        return Err(());
    }
    if n == 0 {
        return Ok(None);
    }
    let mut buf = vec![0u8; n as usize];
    unsafe { pull_copy(buf.as_mut_ptr()) }
    Ok(Some(parse(&buf)))
}

fn ok(token: u32, id: &str, r: &str) {
    send(token, 200, &[], &format!("{{\"id\":{id},\"ok\":true,\"r\":{r}}}"));
}

fn serve(req: Req) {
    let xffh = req.head.contains("\"x-ffh\":\"1\"");
    match (req.method.as_str(), req.path.as_str()) {
        ("GET", "/api/echo") => {
            let body = format!("{{\"m\":{},\"p\":{},\"q\":{},\"xffh\":{xffh},\"body\":{}}}", q(&req.method), q(&req.path), q(&req.query), req.body.len());
            send(req.token, 200, &[("Cache-Control", "no-store")], &body);
        }
        ("GET", "/api/etag") => {
            if req.head.contains("\"if-none-match\":\"\\\"v1\\\"\"") {
                send(req.token, 304, &[("ETag", "\"v1\""), ("Cache-Control", "no-cache")], "");
            } else {
                send(req.token, 200, &[("ETag", "\"v1\""), ("Cache-Control", "no-cache")], "{\"tables\":1}");
            }
        }
        ("GET", "/api/view") => match ST.with(|s| s.borrow().game.clone()) {
            Some(g) => send(req.token, 200, &[], &format!("{{\"game\":{}}}", q(&g))),
            None => send(req.token, 409, &[], "{\"ok\":false,\"error\":\"no_game\",\"detail\":\"no game is loaded\"}"),
        },
        ("POST", "/api") if !xffh => send(req.token, 403, &[], "{\"ok\":false,\"error\":\"forbidden\",\"detail\":\"POST /api needs the header X-FFH: 1\"}"),
        ("POST", "/api") => post(req),
        (_, "/api" | "/api/echo" | "/api/etag" | "/api/view") => send(req.token, 405, &[("Allow", "GET")], "{\"ok\":false,\"error\":\"method\"}"),
        (_, p) => send(req.token, 404, &[], &format!("{{\"ok\":false,\"error\":\"not_found\",\"detail\":{}}}", q(&format!("no {p}")))),
    }
}

fn post(req: Req) {
    let id = field(&req.body, "id").unwrap_or("null").to_string();
    let op = field(&req.body, "op").unwrap_or("").to_string();
    let t = req.token;
    match op.as_str() {
        "ping" => ok(t, &id, "\"pong\""),
        "new" => {
            let name = field(&req.body, "name").unwrap_or("game").to_string();
            std::fs::create_dir_all(format!("/saves/{name}/auto")).unwrap();
            ST.with(|s| s.borrow_mut().game = Some(name));
            set_phase("human");
            ok(t, &id, "true");
        }
        "write" => {
            let name = field(&req.body, "name").unwrap_or("x");
            std::fs::create_dir_all("/saves/g").unwrap();
            std::fs::write(format!("/saves/g/{name}"), field(&req.body, "text").unwrap_or("")).unwrap();
            ok(t, &id, "true");
        }
        "read" => {
            let name = field(&req.body, "name").unwrap_or("x");
            let r = std::fs::read_to_string(format!("/saves/g/{name}")).map(|s| q(&s)).unwrap_or_else(|_| "null".into());
            ok(t, &id, &r);
        }
        "slow" => {
            let n: u32 = field(&req.body, "n").and_then(|v| v.parse().ok()).unwrap_or(10);
            set_phase("running");
            let mut pulled = 0;
            for _ in 0..n {
                while let Ok(Some(r)) = pull_one(false) {
                    pulled += 1;
                    serve(r);
                }
                std::thread::sleep(std::time::Duration::from_millis(5));
            }
            set_phase("human");
            ok(t, &id, &format!("{{\"pulled\":{pulled}}}"));
        }
        "block" => {
            set_phase("decision");
            loop {
                match pull_one(true) {
                    Err(()) => {
                        ok(t, &id, "\"advisor\"");
                        break;
                    }
                    Ok(None) => continue,
                    Ok(Some(r)) if r.method == "POST" && field(&r.body, "op") == Some("answer") => {
                        let rid = field(&r.body, "id").unwrap_or("null").to_string();
                        ok(r.token, &rid, "\"answered\"");
                        ok(t, &id, "\"decided\"");
                        break;
                    }
                    Ok(Some(r)) => serve(r),
                }
            }
            set_phase("human");
        }
        "panic" => panic!("mock panic for request {id}"),
        _ => send(t, 200, &[], &format!("{{\"id\":{id},\"ok\":false,\"error\":\"invalid\",\"detail\":\"no op\"}}")),
    }
}

#[no_mangle]
pub extern "C" fn ffh_alloc(len: u32) -> u32 {
    let mut v = Vec::<u8>::with_capacity(len.max(1) as usize);
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p as usize as u32
}

/// # Safety
/// From one `ffh_alloc(len)`.
#[no_mangle]
pub unsafe extern "C" fn ffh_free(ptr: u32, len: u32) {
    drop(unsafe { Vec::from_raw_parts(ptr as usize as *mut u8, 0, len.max(1) as usize) });
}

#[no_mangle]
pub extern "C" fn ffh_init(ptr: u32, len: u32) -> u32 {
    std::panic::set_hook(Box::new(|info| {
        let msg = info.payload().downcast_ref::<String>().cloned().or_else(|| info.payload().downcast_ref::<&str>().map(|s| s.to_string())).unwrap_or_default();
        let text = match info.location() {
            Some(l) => format!("{msg} (at mock_engine.rs:{})", l.line()),
            None => msg,
        };
        unsafe { crash(text.as_ptr(), text.len() as u32) }
    }));
    let opts = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) }).into_owned();
    if !opts.contains("\"saves\":\"/saves\"") {
        eprintln!("ffh_init: bad options {opts}");
        return 1;
    }
    let dir = std::env::var("FFH_DATA_DIR").unwrap_or_default();
    let data = std::fs::read_to_string(format!("{dir}/xml/test.xml")).unwrap_or_else(|e| format!("error: {e}"));
    let blocking = opts.contains("\"blocking\":true");
    ST.with(|s| s.borrow_mut().data = format!("{data}|blocking={blocking}"));
    println!("mock engine up");
    announce();
    0
}

#[no_mangle]
pub extern "C" fn ffh_request(ptr: u32, len: u32) {
    let frame = unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) }.to_vec();
    serve(parse(&frame));
}
