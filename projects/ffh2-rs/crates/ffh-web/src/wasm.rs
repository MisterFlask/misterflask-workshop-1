//! The wasm glue: the exports, the imports, the [`WebIo`] over them, the panic hook.

use std::alloc::Layout;
use std::cell::RefCell;
use std::sync::Arc;

use ffh_play::json::Json;
use ffh_play::transport::http::Response;
use ffh_play::transport::web::{decode_request, encode_reply, Pulled, WebGame, WebIo, WebOptions};

#[link(wasm_import_module = "ffh")]
extern "C" {
    /// The reply to request `token` (framed: `transport::web::encode_reply`).
    #[link_name = "reply"]
    fn host_reply(token: u32, ptr: *const u8, len: u32);
    /// The status JSON after it changed.
    #[link_name = "publish"]
    fn host_publish(ptr: *const u8, len: u32);
    /// The next queued request's length (> 0, then `pull_copy`), 0 (none queued; not with
    /// `block`) or -1 (the page cannot deliver requests while the engine runs).
    #[link_name = "pull"]
    fn host_pull(block: u32) -> i32;
    /// Copies the request announced by the last `pull` to `ptr`.
    #[link_name = "pull_copy"]
    fn host_pull_copy(ptr: *mut u8);
    /// The panic message, just before the module traps.
    #[link_name = "crash"]
    fn host_crash(ptr: *const u8, len: u32);
}

/// The page, through the imports.
struct PageIo;

impl WebIo for PageIo {
    fn reply(&self, token: u32, resp: &Response) {
        let frame = encode_reply(resp);
        // SAFETY: the page copies the bytes during the call.
        unsafe { host_reply(token, frame.as_ptr(), frame.len() as u32) }
    }

    fn publish(&self, status: &Json) {
        let text = status.to_string();
        // SAFETY: as `reply`.
        unsafe { host_publish(text.as_ptr(), text.len() as u32) }
    }

    fn pull(&self, block: bool) -> Pulled {
        loop {
            // SAFETY: plain values.
            let n = unsafe { host_pull(u32::from(block)) };
            if n < 0 {
                return Pulled::Unsupported;
            }
            if n == 0 {
                return Pulled::Empty;
            }
            let mut frame = vec![0u8; n as usize];
            // SAFETY: the page writes exactly the `n` bytes it announced.
            unsafe { host_pull_copy(frame.as_mut_ptr()) }
            match decode_request(&frame) {
                Ok((token, req)) => return Pulled::Request(token, req),
                // A malformed request is answered (or reported) and the next one pulled.
                Err(e) => match e.token {
                    Some(token) => self.reply(token, &e.response()),
                    None => eprintln!("ffh-web: a request without a token: {}", e.msg),
                },
            }
        }
    }
}

thread_local! {
    /// The host, once `ffh_init` made it (wasm32-wasip1 has one thread).
    static GAME: RefCell<Option<WebGame>> = const { RefCell::new(None) };
}

/// The bytes the page put at `ptr` (valid during the call).
fn input<'a>(ptr: u32, len: u32) -> &'a [u8] {
    if len == 0 {
        return &[];
    }
    // SAFETY: the page allocated `len` bytes at `ptr` with `ffh_alloc` and keeps them during the
    // call.
    unsafe { std::slice::from_raw_parts(ptr as usize as *const u8, len as usize) }
}

/// `len` bytes for the page to write a request or the options into; it frees them with
/// [`ffh_free`].
#[no_mangle]
pub extern "C" fn ffh_alloc(len: u32) -> u32 {
    if len == 0 {
        return std::ptr::NonNull::<u8>::dangling().as_ptr() as usize as u32;
    }
    let layout = Layout::array::<u8>(len as usize).expect("ffh_alloc: too large");
    // SAFETY: the layout is not zero-sized.
    let p = unsafe { std::alloc::alloc(layout) };
    if p.is_null() {
        std::alloc::handle_alloc_error(layout);
    }
    p as usize as u32
}

/// Frees what [`ffh_alloc`] returned (with the same `len`).
///
/// # Safety
/// `ptr` and `len` come from one `ffh_alloc` call, freed once.
#[no_mangle]
pub unsafe extern "C" fn ffh_free(ptr: u32, len: u32) {
    if len == 0 || ptr == 0 {
        return;
    }
    // SAFETY: the caller's contract; the layout is `ffh_alloc`'s.
    unsafe { std::alloc::dealloc(ptr as usize as *mut u8, Layout::array::<u8>(len as usize).expect("ffh_free: too large")) }
}

/// The options (`transport::web::WebOptions::from_json`; an empty buffer is `{}`): makes the host
/// (no game: the client starts one), loads the game data (`FFH_DATA_DIR`, the page mounts the
/// pack at `/data`) and publishes the first status. 0, or 1 when an option is bad or the data
/// directory is missing (the reason on stderr).
#[no_mangle]
pub extern "C" fn ffh_init(ptr: u32, len: u32) -> u32 {
    install_panic_hook();
    let bytes = input(ptr, len);
    let opts = if bytes.is_empty() { Ok(Json::obj()) } else { Json::parse_bytes(bytes).map_err(|e| format!("the options are not JSON: {e}")) };
    let opts = match opts.and_then(|j| WebOptions::from_json(&j)) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("ffh_init: {e}");
            return 1;
        }
    };
    let data = ffh_data::default_data_dir();
    if !data.join("xml").is_dir() {
        eprintln!("ffh_init: no game data in {} (FFH_DATA_DIR)", data.display());
        return 1;
    }
    // Loaded now rather than by the first request (a broken pack panics: `crash`).
    let _ = ffh_data::global();
    let game = WebGame::start(opts, Arc::new(PageIo));
    GAME.with(|g| *g.borrow_mut() = Some(game));
    0
}

/// One HTTP request (framed: `transport::web::decode_request`). Its reply comes through the
/// import `reply`, as do the replies to the requests pulled while it runs.
#[no_mangle]
pub extern "C" fn ffh_request(ptr: u32, len: u32) {
    let frame = input(ptr, len).to_vec();
    GAME.with(|g| match g.borrow_mut().as_mut() {
        Some(game) => game.request_frame(&frame),
        None => match decode_request(&frame) {
            Ok((token, _)) => PageIo.reply(token, &Response::error(503, "starting", "ffh_init has not run")),
            Err(e) => match e.token {
                Some(token) => PageIo.reply(token, &e.response()),
                None => eprintln!("ffh-web: a request without a token: {}", e.msg),
            },
        },
    });
}

/// A panic aborts on wasm: the message (and where) goes to the page through `crash`, after the
/// default hook printed it on stderr; then the module traps.
fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let prev = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            prev(info);
            let message = if let Some(s) = info.payload().downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = info.payload().downcast_ref::<String>() {
                s.clone()
            } else {
                "<non-string panic>".to_string()
            };
            let text = match info.location() {
                Some(l) => {
                    let f = l.file();
                    let f = f.rsplit_once("/crates/").map_or(f, |(_, r)| r);
                    let f = f.strip_prefix("crates/").unwrap_or(f);
                    format!("{message} (at {f}:{})", l.line())
                }
                None => message,
            };
            // SAFETY: the page copies the bytes during the call.
            unsafe { host_crash(text.as_ptr(), text.len() as u32) }
        }));
    });
}
