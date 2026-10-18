//! A small HTTP/1.1 server over `std::net` (design §4.2): one request per connection
//! (`Connection: close`), `Content-Length` bodies only, a thread per connection.
//!
//! | Request | Answer |
//! |---|---|
//! | `GET /`, `/app/..` | the client's files (embedded; `--assets DIR` serves them from disk) |
//! | `GET /art/..` | the art toolkit's modules and `/art/art.bundle.txt`, the sprite data (embedded by `build.rs`; with `--assets DIR`, from `DIR/../web/art` when it exists) |
//! | `GET /api/status` | the status ([`Shared`]) |
//! | `GET /api/wait?since=V&timeout_ms=T` | the status once its version is above `V` (long-poll) |
//! | `GET /api/view`, `/api/map` | the cached view and map of the last control point (409 without a game) |
//! | `GET /api/map?since=R&epoch=E` | the tiles changed since revision `R` of the map (phase 6), or the full map |
//! | `GET /api/data` | the static tables, with an ETag |
//! | `GET /api/setup` | the new-game lists |
//! | `POST /api` | the envelope, answered by the game thread; needs the header `X-FFH: 1` |
//!
//! Safety for a local server: the custom header stops cross-site form posts (a browser sends it
//! only after a CORS preflight, which this server never grants); on a loopback address every
//! request must name a loopback host (`Host: 127.0.0.1:8642`, `localhost`, `[::1]`, or the bound
//! address itself), which stops DNS rebinding; asset paths and save names cannot leave their
//! directories; a request must arrive whole within [`REQUEST_TIME`] (408), so a slow sender
//! cannot keep a connection thread.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, Shutdown, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use super::server::Envelope;
use super::shared::Shared;
use crate::json::Json;

/// Largest request head (request line and headers).
pub const MAX_HEAD: usize = 16 * 1024;
/// Largest request body.
pub const MAX_BODY: usize = 1 << 20;
/// The time a client has to send its whole request (head and body), from the connection's
/// start: a deadline, not a per-read timeout (a client trickling bytes is cut off too).
pub const REQUEST_TIME: Duration = Duration::from_secs(15);
/// How long a connection is drained after an answer sent before its request was read.
const DRAIN_TIME: Duration = Duration::from_millis(250);

const HTML: &str = "text/html; charset=utf-8";
const JS: &str = "text/javascript; charset=utf-8";
const CSS: &str = "text/css; charset=utf-8";
const TEXT: &str = "text/plain; charset=utf-8";
const PNG: &str = "image/png";

/// The client files compiled into the binary: `(path, content type, content)`, served at
/// `/<path>` (`/` is `index.html`). The layout of `assets/` is the URL layout, so that
/// `--assets crates/ffh-play/assets` serves the same files from disk; `tests/web.rs` checks that
/// every file of `assets/app/` is listed here and that the modules' imports resolve.
pub const ASSETS: &[(&str, &str, &str)] = &[
    ("index.html", HTML, include_str!("../../assets/index.html")),
    ("app/style.css", CSS, include_str!("../../assets/app/style.css")),
    ("app/main.js", JS, include_str!("../../assets/app/main.js")),
    ("app/api.js", JS, include_str!("../../assets/app/api.js")),
    ("app/state.js", JS, include_str!("../../assets/app/state.js")),
    ("app/tiles.js", JS, include_str!("../../assets/app/tiles.js")),
    ("app/markup.js", JS, include_str!("../../assets/app/markup.js")),
    ("app/dom.js", JS, include_str!("../../assets/app/dom.js")),
    ("app/modals.js", JS, include_str!("../../assets/app/modals.js")),
    ("app/commands.js", JS, include_str!("../../assets/app/commands.js")),
    ("app/map.js", JS, include_str!("../../assets/app/map.js")),
    ("app/iso.js", JS, include_str!("../../assets/app/iso.js")),
    ("app/units.js", JS, include_str!("../../assets/app/units.js")),
    ("app/city.js", JS, include_str!("../../assets/app/city.js")),
    ("app/screens.js", JS, include_str!("../../assets/app/screens.js")),
    ("app/hud.js", JS, include_str!("../../assets/app/hud.js")),
    ("app/hotkeys.js", JS, include_str!("../../assets/app/hotkeys.js")),
    ("app/empire.js", JS, include_str!("../../assets/app/empire.js")),
    ("app/diplo.js", JS, include_str!("../../assets/app/diplo.js")),
    ("app/tools.js", JS, include_str!("../../assets/app/tools.js")),
    ("app/look.js", JS, include_str!("../../assets/app/look.js")),
    ("app/artmap.js", JS, include_str!("../../assets/app/artmap.js")),
    ("app/stacks.js", JS, include_str!("../../assets/app/stacks.js")),
    ("app/chunks.js", JS, include_str!("../../assets/app/chunks.js")),
    ("app/artworker.js", JS, include_str!("../../assets/app/artworker.js")),
    // The experimental 3D renderer (docs/R3D.md), loaded by the client only when chosen.
    ("app/r3d/config.js", JS, include_str!("../../assets/app/r3d/config.js")),
    ("app/r3d/decals.js", JS, include_str!("../../assets/app/r3d/decals.js")),
    ("app/r3d/dress.js", JS, include_str!("../../assets/app/r3d/dress.js")),
    ("app/r3d/fields.js", JS, include_str!("../../assets/app/r3d/fields.js")),
    ("app/r3d/gl.js", JS, include_str!("../../assets/app/r3d/gl.js")),
    ("app/r3d/hud.js", JS, include_str!("../../assets/app/r3d/hud.js")),
    ("app/r3d/map3d.js", JS, include_str!("../../assets/app/r3d/map3d.js")),
    ("app/r3d/materials.js", JS, include_str!("../../assets/app/r3d/materials.js")),
    ("app/r3d/math.js", JS, include_str!("../../assets/app/r3d/math.js")),
    ("app/r3d/mesh.js", JS, include_str!("../../assets/app/r3d/mesh.js")),
    ("app/r3d/noise.js", JS, include_str!("../../assets/app/r3d/noise.js")),
    ("app/r3d/pipeline.js", JS, include_str!("../../assets/app/r3d/pipeline.js")),
    ("app/r3d/scene.js", JS, include_str!("../../assets/app/r3d/scene.js")),
    ("app/r3d/shaders.js", JS, include_str!("../../assets/app/r3d/shaders.js")),
    ("app/r3d/terrain.js", JS, include_str!("../../assets/app/r3d/terrain.js")),
    ("app/r3d/world.js", JS, include_str!("../../assets/app/r3d/world.js")),
    ("app/r3d/assets/architecture.js", JS, include_str!("../../assets/app/r3d/assets/architecture.js")),
    ("app/r3d/assets/basic.js", JS, include_str!("../../assets/app/r3d/assets/basic.js")),
    ("app/r3d/assets/catalog.js", JS, include_str!("../../assets/app/r3d/assets/catalog.js")),
    ("app/r3d/assets/fallbacks.js", JS, include_str!("../../assets/app/r3d/assets/fallbacks.js")),
    ("app/r3d/assets/fx.js", JS, include_str!("../../assets/app/r3d/assets/fx.js")),
    ("app/r3d/assets/index.js", JS, include_str!("../../assets/app/r3d/assets/index.js")),
    ("app/r3d/assets/palette.js", JS, include_str!("../../assets/app/r3d/assets/palette.js")),
    ("app/r3d/assets/props.js", JS, include_str!("../../assets/app/r3d/assets/props.js")),
    ("app/r3d/assets/registry.js", JS, include_str!("../../assets/app/r3d/assets/registry.js")),
    ("app/r3d/assets/units.js", JS, include_str!("../../assets/app/r3d/assets/units.js")),
    ("app/r3d/assets/vegetation.js", JS, include_str!("../../assets/app/r3d/assets/vegetation.js")),
];

/// Binary client files (the isometric renderer's PNG tiles and unit sprites,
/// docs/art/SOC_DIRECTION.md): `(path, content type, bytes)`, served like [`ASSETS`] but from raw
/// bytes. `tests/web.rs` checks that every non-text file under `assets/app/` is listed here.
pub const ASSET_BYTES: &[(&str, &str, &[u8])] = &[
    ("app/iso/terrain/grass.png", PNG, include_bytes!("../../assets/app/iso/terrain/grass.png")),
    ("app/iso/terrain/plains.png", PNG, include_bytes!("../../assets/app/iso/terrain/plains.png")),
    ("app/iso/terrain/desert.png", PNG, include_bytes!("../../assets/app/iso/terrain/desert.png")),
    ("app/iso/terrain/snow.png", PNG, include_bytes!("../../assets/app/iso/terrain/snow.png")),
    ("app/iso/terrain/marsh.png", PNG, include_bytes!("../../assets/app/iso/terrain/marsh.png")),
    ("app/iso/terrain/tundra.png", PNG, include_bytes!("../../assets/app/iso/terrain/tundra.png")),
    ("app/iso/terrain/coast.png", PNG, include_bytes!("../../assets/app/iso/terrain/coast.png")),
    ("app/iso/terrain/ocean.png", PNG, include_bytes!("../../assets/app/iso/terrain/ocean.png")),
    ("app/iso/terrain/hell.png", PNG, include_bytes!("../../assets/app/iso/terrain/hell.png")),
    ("app/iso/terrain/burning.png", PNG, include_bytes!("../../assets/app/iso/terrain/burning.png")),
    ("app/iso/terrain/perdition.png", PNG, include_bytes!("../../assets/app/iso/terrain/perdition.png")),
    ("app/iso/trees/pine_a.png", PNG, include_bytes!("../../assets/app/iso/trees/pine_a.png")),
    ("app/iso/trees/pine_b.png", PNG, include_bytes!("../../assets/app/iso/trees/pine_b.png")),
    ("app/iso/trees/palm_a.png", PNG, include_bytes!("../../assets/app/iso/trees/palm_a.png")),
    ("app/iso/trees/palm_b.png", PNG, include_bytes!("../../assets/app/iso/trees/palm_b.png")),
    ("app/iso/trees/ancient_a.png", PNG, include_bytes!("../../assets/app/iso/trees/ancient_a.png")),
    ("app/iso/trees/ancient_b.png", PNG, include_bytes!("../../assets/app/iso/trees/ancient_b.png")),
    ("app/iso/trees/dead_a.png", PNG, include_bytes!("../../assets/app/iso/trees/dead_a.png")),
    ("app/iso/trees/dead_b.png", PNG, include_bytes!("../../assets/app/iso/trees/dead_b.png")),
    ("app/iso/features/oasis.png", PNG, include_bytes!("../../assets/app/iso/features/oasis.png")),
    ("app/iso/features/ice.png", PNG, include_bytes!("../../assets/app/iso/features/ice.png")),
    ("app/iso/features/scrub.png", PNG, include_bytes!("../../assets/app/iso/features/scrub.png")),
    ("app/iso/features/flood.png", PNG, include_bytes!("../../assets/app/iso/features/flood.png")),
    ("app/iso/features/volcano.png", PNG, include_bytes!("../../assets/app/iso/features/volcano.png")),
    ("app/iso/features/flames.png", PNG, include_bytes!("../../assets/app/iso/features/flames.png")),
    ("app/iso/relief/peak.png", PNG, include_bytes!("../../assets/app/iso/relief/peak.png")),
    ("app/iso/cities/town.png", PNG, include_bytes!("../../assets/app/iso/cities/town.png")),
    ("app/iso/resources/mana.png", PNG, include_bytes!("../../assets/app/iso/resources/mana.png")),
    ("app/iso/resources/crop.png", PNG, include_bytes!("../../assets/app/iso/resources/crop.png")),
    ("app/iso/resources/animal.png", PNG, include_bytes!("../../assets/app/iso/resources/animal.png")),
    ("app/iso/resources/fish.png", PNG, include_bytes!("../../assets/app/iso/resources/fish.png")),
    ("app/iso/resources/ore.png", PNG, include_bytes!("../../assets/app/iso/resources/ore.png")),
    ("app/iso/resources/goods.png", PNG, include_bytes!("../../assets/app/iso/resources/goods.png")),
    ("app/iso/improvements/lair.png", PNG, include_bytes!("../../assets/app/iso/improvements/lair.png")),
    ("app/iso/improvements/fort.png", PNG, include_bytes!("../../assets/app/iso/improvements/fort.png")),
    ("app/iso/improvements/mananode.png", PNG, include_bytes!("../../assets/app/iso/improvements/mananode.png")),
    ("app/iso/improvements/cottage.png", PNG, include_bytes!("../../assets/app/iso/improvements/cottage.png")),
    ("app/iso/improvements/farm.png", PNG, include_bytes!("../../assets/app/iso/improvements/farm.png")),
    ("app/iso/improvements/mine.png", PNG, include_bytes!("../../assets/app/iso/improvements/mine.png")),
    ("app/iso/units/knight.png", PNG, include_bytes!("../../assets/app/iso/units/knight.png")),
    ("app/iso/units/lich.png", PNG, include_bytes!("../../assets/app/iso/units/lich.png")),
    ("app/iso/units/archer.png", PNG, include_bytes!("../../assets/app/iso/units/archer.png")),
    ("app/iso/units/spider.png", PNG, include_bytes!("../../assets/app/iso/units/spider.png")),
    ("app/iso/units/settler.png", PNG, include_bytes!("../../assets/app/iso/units/settler.png")),
    ("app/iso/units/scout.png", PNG, include_bytes!("../../assets/app/iso/units/scout.png")),
    ("app/iso/units/worker.png", PNG, include_bytes!("../../assets/app/iso/units/worker.png")),
    ("app/iso/units/cavalry.png", PNG, include_bytes!("../../assets/app/iso/units/cavalry.png")),
    ("app/iso/units/siege.png", PNG, include_bytes!("../../assets/app/iso/units/siege.png")),
    ("app/iso/units/ship.png", PNG, include_bytes!("../../assets/app/iso/units/ship.png")),
    ("app/iso/units/demon.png", PNG, include_bytes!("../../assets/app/iso/units/demon.png")),
];

// `ART_MODULES`: the art toolkit's modules (`web/art`, docs/ART.md 15-16), embedded by `build.rs`
// from the list `art_files::art_modules` makes; served at `/art/<path>` with the JS type.
include!(concat!(env!("OUT_DIR"), "/art_assets.rs"));

/// The art's sprite data in one text (`web/art/tools/bundle.mjs`'s format, made by
/// `art_files::art_bundle` in `build.rs`), served at `/art/art.bundle.txt`.
pub const ART_BUNDLE: &str = include_str!(concat!(env!("OUT_DIR"), "/art.bundle.txt"));

/// The URL path of the art bundle (without the leading `/`).
pub const ART_BUNDLE_PATH: &str = "art/art.bundle.txt";

/// A parsed request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpRequest {
    pub method: String,
    /// The path of the target (before `?`).
    pub path: String,
    /// The query string (after `?`, without it).
    pub query: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// A header's value (names are case-insensitive).
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// A query parameter (no percent-decoding: the parameters are numbers).
    pub fn param(&self, name: &str) -> Option<&str> {
        self.query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| *k == name).map(|(_, v)| v)
    }
}

/// A request that cannot be served: the status and why.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HttpError {
    pub status: u16,
    pub msg: String,
}

fn http_err(status: u16, msg: impl Into<String>) -> HttpError {
    HttpError { status, msg: msg.into() }
}

/// Where the head ends: the index after the blank line.
fn head_end(buf: &[u8]) -> Option<usize> {
    let crlf = buf.windows(4).position(|w| w == b"\r\n\r\n").map(|i| i + 4);
    let lf = buf.windows(2).position(|w| w == b"\n\n").map(|i| i + 2);
    match (crlf, lf) {
        (Some(a), Some(b)) => Some(a.min(b)),
        (a, b) => a.or(b),
    }
}

/// A stream read against a deadline: each read waits at most the time left, and fails with
/// `TimedOut` once it has passed.
pub struct Deadline<'a> {
    pub stream: &'a TcpStream,
    pub deadline: Instant,
}

impl Read for Deadline<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let left = self.deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "deadline passed"));
        }
        self.stream.set_read_timeout(Some(left))?;
        let mut stream = self.stream;
        stream.read(buf)
    }
}

/// A read error: 408 when the time ran out, else 400.
fn read_err(what: &str, e: std::io::Error) -> HttpError {
    match e.kind() {
        std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock => {
            http_err(408, format!("the request did not arrive within {} s", REQUEST_TIME.as_secs()))
        }
        _ => http_err(400, format!("reading {what}: {e}")),
    }
}

/// [`read_request`] from a connection, which must send its whole request within `within`.
pub fn read_request_within(stream: &TcpStream, within: Duration) -> Result<HttpRequest, HttpError> {
    read_request(&mut Deadline { stream, deadline: Instant::now() + within })
}

/// Reads and drops what the client still sends, for at most `within`: a close with input unread
/// makes the kernel reset the connection, and the client could lose the answer.
pub fn drain(stream: &TcpStream, within: Duration) {
    let mut r = Deadline { stream, deadline: Instant::now() + within };
    let mut sink = [0u8; 8192];
    let mut total = 0;
    while let Ok(n) = r.read(&mut sink) {
        total += n;
        if n == 0 || total > 2 * MAX_BODY {
            break;
        }
    }
}

/// Reads one request: the head (at most [`MAX_HEAD`]), then a `Content-Length` body (at most
/// [`MAX_BODY`]). A chunked body is refused (400), a POST without a length too (411); a read
/// that times out is 408.
pub fn read_request(r: &mut impl Read) -> Result<HttpRequest, HttpError> {
    let mut buf = Vec::with_capacity(1024);
    let mut chunk = [0u8; 4096];
    let end = loop {
        if let Some(e) = head_end(&buf) {
            break e;
        }
        if buf.len() > MAX_HEAD {
            return Err(http_err(431, "the request head is too large"));
        }
        let n = r.read(&mut chunk).map_err(|e| read_err("the request", e))?;
        if n == 0 {
            return Err(http_err(400, "the connection closed before the end of the request head"));
        }
        buf.extend_from_slice(&chunk[..n]);
    };
    if end > MAX_HEAD {
        return Err(http_err(431, "the request head is too large"));
    }
    let head = std::str::from_utf8(&buf[..end]).map_err(|_| http_err(400, "the request head is not UTF-8"))?;
    let mut lines = head.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l));
    let request_line = lines.next().unwrap_or_default();
    let mut parts = request_line.split(' ');
    let (Some(method), Some(target), Some(version), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else {
        return Err(http_err(400, format!("bad request line {request_line:?}")));
    };
    if !version.starts_with("HTTP/1.") {
        return Err(http_err(505, format!("{version} is not supported")));
    }
    if method.is_empty() || !method.bytes().all(|b| b.is_ascii_uppercase()) {
        return Err(http_err(400, format!("bad method {method:?}")));
    }
    if !target.starts_with('/') {
        return Err(http_err(400, "the target must be a path"));
    }
    let (path, query) = target.split_once('?').unwrap_or((target, ""));
    let mut headers = Vec::new();
    for l in lines {
        if l.is_empty() {
            continue;
        }
        if l.starts_with(' ') || l.starts_with('\t') {
            return Err(http_err(400, "folded header lines are not accepted"));
        }
        let Some((k, v)) = l.split_once(':') else { return Err(http_err(400, format!("bad header line {l:?}"))) };
        if k.is_empty() || !k.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_!#$%&'*+.^`|~".contains(&b)) {
            return Err(http_err(400, format!("bad header name {k:?}")));
        }
        headers.push((k.to_string(), v.trim().to_string()));
    }
    let mut req = HttpRequest { method: method.to_string(), path: path.to_string(), query: query.to_string(), headers, body: Vec::new() };
    if req.header("transfer-encoding").is_some() {
        return Err(http_err(400, "only Content-Length bodies are accepted"));
    }
    let lengths: Vec<&str> = req.headers.iter().filter(|(k, _)| k.eq_ignore_ascii_case("content-length")).map(|(_, v)| v.as_str()).collect();
    let length = match lengths.as_slice() {
        [] if req.method == "POST" => return Err(http_err(411, "a POST needs a Content-Length")),
        [] => 0,
        [v] if !v.is_empty() && v.len() <= 12 && v.bytes().all(|b| b.is_ascii_digit()) => v.parse::<usize>().unwrap_or(usize::MAX),
        _ => return Err(http_err(400, "bad Content-Length")),
    };
    if length > MAX_BODY {
        return Err(http_err(413, format!("the body is larger than {MAX_BODY} bytes")));
    }
    let mut body = buf[end..].to_vec();
    if body.len() > length {
        body.truncate(length);
    }
    if body.len() < length {
        let have = body.len();
        body.resize(length, 0);
        r.read_exact(&mut body[have..]).map_err(|e| read_err("the body", e))?;
    }
    req.body = body;
    Ok(req)
}

/// A response body.
pub enum Body {
    Bytes(Vec<u8>),
    Shared(Arc<String>),
    Static(&'static str),
}

impl Body {
    fn bytes(&self) -> &[u8] {
        match self {
            Body::Bytes(b) => b,
            Body::Shared(s) => s.as_bytes(),
            Body::Static(s) => s.as_bytes(),
        }
    }
}

/// A response: every one closes the connection.
pub struct Response {
    pub status: u16,
    pub content_type: &'static str,
    pub headers: Vec<(&'static str, String)>,
    pub body: Body,
}

const JSON_TYPE: &str = "application/json; charset=utf-8";

impl Response {
    pub fn json(status: u16, j: &Json) -> Response {
        Response { status, content_type: JSON_TYPE, headers: Vec::new(), body: Body::Bytes(j.to_string().into_bytes()) }
    }

    fn shared_json(s: Arc<String>) -> Response {
        Response { status: 200, content_type: JSON_TYPE, headers: Vec::new(), body: Body::Shared(s) }
    }

    /// An error as JSON: `{"ok": false, "error", "detail"}`.
    pub fn error(status: u16, code: &str, detail: impl Into<String>) -> Response {
        let detail: String = detail.into();
        Response::json(status, &crate::jobj! { "ok" => false, "error" => code, "detail" => detail })
    }

    fn with(mut self, k: &'static str, v: impl Into<String>) -> Response {
        self.headers.push((k, v.into()));
        self
    }

    /// Writes the response.
    pub fn write_to(&self, w: &mut impl Write) -> std::io::Result<()> {
        let body = self.body.bytes();
        let mut head = format!("HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\nX-Content-Type-Options: nosniff\r\n", self.status, reason(self.status), self.content_type, body.len());
        if !self.headers.iter().any(|(k, _)| *k == "Cache-Control") {
            head += "Cache-Control: no-store\r\n";
        }
        for (k, v) in &self.headers {
            head += &format!("{k}: {v}\r\n");
        }
        head += "\r\n";
        w.write_all(head.as_bytes())?;
        if self.status != 304 {
            w.write_all(body)?;
        }
        w.flush()
    }
}

fn reason(status: u16) -> &'static str {
    match status {
        200 => "OK",
        304 => "Not Modified",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        411 => "Length Required",
        413 => "Content Too Large",
        431 => "Request Header Fields Too Large",
        500 => "Internal Server Error",
        503 => "Service Unavailable",
        505 => "HTTP Version Not Supported",
        _ => "Status",
    }
}

/// What the HTTP threads need.
pub struct HttpCtx {
    pub shared: Arc<Shared>,
    /// To the game thread.
    pub tx: Sender<Envelope>,
    /// `--assets DIR`: the client files from disk instead of the embedded ones.
    pub assets: Option<PathBuf>,
    /// Bound to this loopback address: requests must name a loopback host (or this address).
    pub loopback: Option<IpAddr>,
}

/// Serves one connection: one request (which must arrive within [`REQUEST_TIME`]), one response,
/// then close.
pub fn handle_connection(mut stream: TcpStream, ctx: &HttpCtx) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(30)));
    let (resp, unread) = match read_request_within(&stream, REQUEST_TIME) {
        Ok(req) => (route(&req, ctx), false),
        Err(e) => (Response::error(e.status, "http", e.msg), true),
    };
    let _ = resp.write_to(&mut stream);
    let _ = stream.shutdown(Shutdown::Write);
    if unread {
        drain(&stream, DRAIN_TIME);
    }
}

/// Answers a connection the server has no room for with 503, without reading its request (then
/// drains it, so that the client gets the answer rather than a reset).
pub fn refuse(mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(Duration::from_secs(1)));
    let _ = Response::error(503, "busy", "too many open connections").write_to(&mut stream);
    let _ = stream.shutdown(Shutdown::Write);
    drain(&stream, DRAIN_TIME);
}

/// `Host:` names a loopback host (a port may follow): `localhost`, `127.0.0.1`, `[::1]`, or
/// `bound`, the address the server is bound to (another loopback address such as 127.0.0.2).
pub fn loopback_host(host: &str, bound: Option<IpAddr>) -> bool {
    let name = match host.rsplit_once(':') {
        Some((h, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) && (!h.contains(':') || h.ends_with(']')) => h,
        _ => host,
    };
    if name.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let ip = match name.strip_prefix('[').and_then(|n| n.strip_suffix(']')) {
        Some(v6) => v6.parse::<Ipv6Addr>().ok().map(IpAddr::V6),
        None => name.parse::<Ipv4Addr>().ok().map(IpAddr::V4),
    };
    ip.is_some_and(|ip| ip == IpAddr::V4(Ipv4Addr::LOCALHOST) || ip == IpAddr::V6(Ipv6Addr::LOCALHOST) || Some(ip) == bound)
}

fn data_json() -> &'static (Arc<String>, String) {
    static DATA: OnceLock<(Arc<String>, String)> = OnceLock::new();
    DATA.get_or_init(|| {
        let s = crate::view::data::data(ffh_data::global()).to_string();
        let etag = format!("\"{}\"", crate::journal::fnv_hex(s.as_bytes()));
        (Arc::new(s), etag)
    })
}

fn setup_json() -> Arc<String> {
    static SETUP: OnceLock<Arc<String>> = OnceLock::new();
    SETUP.get_or_init(|| Arc::new(crate::setup::setup_lists(ffh_data::global()).to_string())).clone()
}

const ROUTES: &[(&str, &str)] = &[
    ("/api", "POST"),
    ("/api/status", "GET"),
    ("/api/wait", "GET"),
    ("/api/view", "GET"),
    ("/api/map", "GET"),
    ("/api/data", "GET"),
    ("/api/setup", "GET"),
    ("/", "GET"),
    ("/index.html", "GET"),
];

/// Answers a request (the game thread only for `POST /api`).
pub fn route(req: &HttpRequest, ctx: &HttpCtx) -> Response {
    if let Some(bound) = ctx.loopback {
        if !req.header("host").is_some_and(|h| loopback_host(h, Some(bound))) {
            return Response::error(403, "forbidden", format!("the Host header must name this computer (127.0.0.1, localhost, [::1] or {bound})"));
        }
    }
    let path = req.path.as_str();
    let known = ROUTES.iter().find(|(p, _)| *p == path).map(|(_, m)| *m).or_else(|| (path.starts_with("/app/") || path.starts_with("/art/")).then_some("GET"));
    match known {
        None => return Response::error(404, "not_found", format!("no {path}")),
        Some(m) if m != req.method => return Response::error(405, "method", format!("{path} answers {m} only")).with("Allow", m),
        Some(_) => {}
    }
    match path {
        "/api" => post_api(req, ctx),
        "/api/status" => Response::json(200, &ctx.shared.status()),
        "/api/wait" => {
            let since = req.param("since").and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let ms = req.param("timeout_ms").and_then(|v| v.parse::<u64>().ok()).unwrap_or(25_000);
            Response::json(200, &ctx.shared.wait(since, Duration::from_millis(ms)))
        }
        "/api/view" => ctx.shared.view().map_or_else(|| Response::error(409, "no_game", "no game is loaded"), Response::shared_json),
        "/api/map" => {
            // `?since=R&epoch=E`: the tile deltas (an epoch is hex digits and `-`, a revision a
            // number: nothing to decode).
            let since = req.param("since").and_then(|v| v.parse::<i64>().ok());
            ctx.shared.map_since(since, req.param("epoch")).map_or_else(|| Response::error(409, "no_game", "no game is loaded"), Response::shared_json)
        }
        "/api/data" => {
            let (data, etag) = data_json();
            if req.header("if-none-match") == Some(etag.as_str()) {
                Response { status: 304, content_type: JSON_TYPE, headers: Vec::new(), body: Body::Static("") }
            } else {
                Response::shared_json(data.clone())
            }
            .with("ETag", etag.clone())
            .with("Cache-Control", "no-cache")
        }
        "/api/setup" => Response::shared_json(setup_json()),
        "/" => asset("index.html", ctx),
        p => asset(p.trim_start_matches('/'), ctx),
    }
}

fn post_api(req: &HttpRequest, ctx: &HttpCtx) -> Response {
    if req.header("x-ffh") != Some("1") {
        return Response::error(403, "forbidden", "POST /api needs the header X-FFH: 1");
    }
    let Ok(text) = std::str::from_utf8(&req.body) else { return Response::error(400, "invalid", "the body is not UTF-8") };
    let j = match Json::parse(text) {
        Ok(j) => j,
        Err(e) => return Response::json(200, &super::err_reply(Json::Null, &super::ProtoErr::invalid(format!("not JSON: {e}")))),
    };
    let id = j.get("id").cloned().unwrap_or(Json::Null);
    let (tx, rx) = mpsc::channel();
    if ctx.tx.send(Envelope { req: j, reply: tx }).is_err() {
        return Response::error(503, "stopped", "the game thread has stopped");
    }
    match rx.recv() {
        Ok(reply) => Response::json(200, &reply),
        // The game thread dropped the request: it crashed while handling it (see the status).
        Err(_) => Response::json(200, &super::err_reply(id, &super::ProtoErr::new("crashed", "the game crashed while handling this request: see /api/status"))),
    }
}

/// A client file: embedded, or from `--assets DIR`. Paths are 1-4 parts of `[A-Za-z0-9_.-]`
/// not starting with a dot, so a request cannot leave the directory. The art (`art/..`) comes
/// from `DIR/../web/art` with `--assets DIR` when that directory exists (the repository's
/// layout: `crates/ffh-play/assets` and `crates/ffh-play/web/art`), else from the binary; only
/// the art's modules and its bundle are served, not its tools or previews.
fn asset(path: &str, ctx: &HttpCtx) -> Response {
    let parts: Vec<&str> = path.split('/').collect();
    let ok = (1..=4).contains(&parts.len()) && parts.iter().all(|p| crate::save::sanitize_name(p).is_ok());
    if !ok {
        return Response::error(404, "not_found", format!("no /{path}"));
    }
    if let Some(rest) = path.strip_prefix("art/") {
        return art_asset(path, rest, ctx);
    }
    let content_type = content_type(path);
    match &ctx.assets {
        Some(dir) => match std::fs::read(asset_path(dir, &parts)) {
            Ok(bytes) => Response { status: 200, content_type, headers: Vec::new(), body: Body::Bytes(bytes) },
            Err(_) => Response::error(404, "not_found", format!("no /{path} in {}", dir.display())),
        },
        None => {
            if let Some((_, ct, body)) = ASSETS.iter().find(|(p, _, _)| *p == path) {
                Response { status: 200, content_type: ct, headers: Vec::new(), body: Body::Static(body) }
            } else if let Some((_, ct, bytes)) = ASSET_BYTES.iter().find(|(p, _, _)| *p == path) {
                Response { status: 200, content_type: ct, headers: Vec::new(), body: Body::Bytes(bytes.to_vec()) }
            } else {
                Response::error(404, "not_found", format!("no /{path}"))
            }
        }
    }
}

/// The art directory on disk for `--assets DIR`: `DIR/../web/art`, when it exists.
fn art_dir(ctx: &HttpCtx) -> Option<PathBuf> {
    let dir = ctx.assets.as_ref()?.join("..").join("web").join("art");
    dir.join("art.js").is_file().then_some(dir)
}

/// `/art/<rest>`: a module of [`ART_MODULES`] or the bundle ([`ART_BUNDLE_PATH`]).
fn art_asset(path: &str, rest: &str, ctx: &HttpCtx) -> Response {
    let ok = |content_type, body| Response { status: 200, content_type, headers: vec![("Cache-Control", "no-cache".to_string())], body };
    match art_dir(ctx) {
        Some(dir) if path == ART_BUNDLE_PATH => match super::art_files::art_bundle(&dir) {
            Ok(text) => ok(TEXT, Body::Bytes(text.into_bytes())),
            Err(e) => Response::error(500, "art", format!("the art bundle of {}: {e}", dir.display())),
        },
        Some(dir) => {
            let listed = super::art_files::art_modules(&dir).unwrap_or_default();
            match listed.iter().any(|m| m == rest).then(|| std::fs::read(dir.join(rest))) {
                Some(Ok(bytes)) => ok(JS, Body::Bytes(bytes)),
                _ => Response::error(404, "not_found", format!("no /{path} in {}", dir.display())),
            }
        }
        None if path == ART_BUNDLE_PATH => ok(TEXT, Body::Static(ART_BUNDLE)),
        None => match ART_MODULES.iter().find(|(p, _)| *p == path) {
            Some((_, body)) => ok(JS, Body::Static(body)),
            None => Response::error(404, "not_found", format!("no /{path}")),
        },
    }
}

fn asset_path(dir: &Path, parts: &[&str]) -> PathBuf {
    parts.iter().fold(dir.to_path_buf(), |p, part| p.join(part))
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit_once('.').map(|(_, e)| e) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => JSON_TYPE,
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(s: &str) -> Result<HttpRequest, HttpError> {
        read_request(&mut std::io::Cursor::new(s.as_bytes().to_vec()))
    }

    #[test]
    fn requests_parse_strictly() {
        let r = parse("GET /api/wait?since=4&timeout_ms=10 HTTP/1.1\r\nHost: 127.0.0.1:8642\r\nX-FFH: 1\r\n\r\n").unwrap();
        assert_eq!((r.method.as_str(), r.path.as_str(), r.param("since"), r.param("timeout_ms"), r.param("x")), ("GET", "/api/wait", Some("4"), Some("10"), None));
        assert_eq!(r.header("x-ffh"), Some("1"));
        assert_eq!(r.header("HOST"), Some("127.0.0.1:8642"));
        let r = parse("POST /api HTTP/1.1\nHost: localhost\nContent-Length: 7\n\n{\"k\":1}trailing").unwrap();
        assert_eq!(r.body, b"{\"k\":1}");
        // Errors.
        let cases: &[(&str, u16)] = &[
            ("POST /api HTTP/1.1\r\nHost: x\r\n\r\n{}", 411),
            ("POST /api HTTP/1.1\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n", 400),
            ("POST /api HTTP/1.1\r\nContent-Length: 99999999\r\n\r\n", 413),
            ("POST /api HTTP/1.1\r\nContent-Length: 2\r\nContent-Length: 3\r\n\r\n{}", 400),
            ("POST /api HTTP/1.1\r\nContent-Length: -1\r\n\r\n", 400),
            ("POST /api HTTP/1.1\r\nContent-Length: 10\r\n\r\n{}", 400),
            ("GET /api HTTP/2\r\n\r\n", 505),
            ("GET api HTTP/1.1\r\n\r\n", 400),
            ("GET /a b HTTP/1.1\r\n\r\n", 400),
            ("get / HTTP/1.1\r\n\r\n", 400),
            ("GET / HTTP/1.1\r\n folded\r\n\r\n", 400),
            ("GET / HTTP/1.1\r\nno colon\r\n\r\n", 400),
            ("GET / HTTP/1.1\r\n", 400),
        ];
        for (s, status) in cases {
            assert_eq!(parse(s).map_err(|e| e.status), Err(*status), "{s:?}");
        }
        let big = format!("GET / HTTP/1.1\r\nX: {}\r\n\r\n", "a".repeat(MAX_HEAD));
        assert_eq!(parse(&big).map_err(|e| e.status), Err(431));
    }

    /// The request time is a deadline for the whole request: a client that sends a byte every
    /// 50 ms is cut off when it passes (P4 review: it was a per-read timeout).
    #[test]
    fn a_slow_sender_gets_408() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let sender = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            for b in b"GET /api/status HTTP/1.1\r\nHost: 127.0.0.1\r\n\r\n" {
                if c.write_all(&[*b]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let (stream, _) = listener.accept().unwrap();
        let t0 = Instant::now();
        let e = read_request_within(&stream, Duration::from_millis(400)).unwrap_err();
        assert_eq!(e.status, 408, "{e:?}");
        assert!(t0.elapsed() < Duration::from_millis(1500), "{:?}", t0.elapsed());
        drop(stream);
        sender.join().unwrap();
        // A request sent at once is read.
        let sender = std::thread::spawn(move || {
            let mut c = TcpStream::connect(addr).unwrap();
            c.write_all(b"POST /api HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}").unwrap();
            c
        });
        let (stream, _) = listener.accept().unwrap();
        assert_eq!(read_request_within(&stream, Duration::from_millis(400)).map(|r| r.body), Ok(b"{}".to_vec()));
        drop(sender.join().unwrap());
    }

    #[test]
    fn hosts_and_assets() {
        let home = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
        for ok in ["127.0.0.1", "127.0.0.1:8642", "localhost", "LOCALHOST:1", "[::1]", "[::1]:8642", "[0:0:0:0:0:0:0:1]:80"] {
            assert!(loopback_host(ok, home), "{ok}");
        }
        for bad in ["", "evil.example", "evil.example:8642", "127.0.0.1.evil.example", "localhost.evil", "[::2]:80", "127.0.0.2", "::1", "127.0.0.01", "[127.0.0.1]"] {
            assert!(!loopback_host(bad, home), "{bad}");
        }
        // Bound to another loopback address: that one too (P4 review).
        let other = Some(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2)));
        for ok in ["127.0.0.2", "127.0.0.2:8642", "127.0.0.1:8642", "localhost"] {
            assert!(loopback_host(ok, other), "{ok}");
        }
        assert!(!loopback_host("127.0.0.3:8642", other));
        let (tx, _rx) = mpsc::channel();
        let ctx = HttpCtx { shared: Arc::new(Shared::new()), tx, assets: None, loopback: home };
        let get = |path: &str, host: &str| {
            let req = HttpRequest { method: "GET".into(), path: path.into(), query: String::new(), headers: vec![("Host".into(), host.into())], body: Vec::new() };
            route(&req, &ctx)
        };
        assert_eq!(get("/", "127.0.0.1").status, 200);
        assert_eq!(get("/", "evil.example").status, 403);
        for bad in ["/app/../Cargo.toml", "/app/.hidden", "/app/a/b/c/d/e", "/app/%2e%2e/x", "/etc/passwd", "/app/"] {
            assert_eq!(get(bad, "localhost").status, 404, "{bad}");
        }
        assert_eq!(get("/api", "localhost").status, 405);
        assert_eq!(get("/api/view", "localhost").status, 409);
        let r = get("/api/data", "localhost");
        assert_eq!(r.status, 200);
        let etag = r.headers.iter().find(|(k, _)| *k == "ETag").unwrap().1.clone();
        let req = HttpRequest {
            method: "GET".into(),
            path: "/api/data".into(),
            query: String::new(),
            headers: vec![("Host".into(), "localhost".into()), ("If-None-Match".into(), etag)],
            body: Vec::new(),
        };
        assert_eq!(route(&req, &ctx).status, 304);
        // POST /api without the header.
        let req = HttpRequest { method: "POST".into(), path: "/api".into(), query: String::new(), headers: vec![("Host".into(), "localhost".into())], body: b"{}".to_vec() };
        assert_eq!(route(&req, &ctx).status, 403);
        // The game thread is gone (the receiver dropped): 503.
        let (tx, rx) = mpsc::channel();
        drop(rx);
        let ctx = HttpCtx { shared: Arc::new(Shared::new()), tx, assets: None, loopback: None };
        let req = HttpRequest {
            method: "POST".into(),
            path: "/api".into(),
            query: String::new(),
            headers: vec![("X-FFH".into(), "1".into())],
            body: b"{\"k\":\"q\",\"q\":\"status\"}".to_vec(),
        };
        assert_eq!(route(&req, &ctx).status, 503);
        assert_eq!(content_type("app/x.js"), "text/javascript; charset=utf-8");
    }
}
