//! The engine of the static site (docs/WEB.md): `ffh-play`'s web host
//! ([`ffh_play::transport::web`]) compiled to WebAssembly (`wasm32-wasip1`, a WASI reactor) for a
//! Web Worker, with no server: the page (`crates/ffh-web/site`) routes the client's `/api`
//! requests to it.
//!
//! Build: `cargo build --release --target wasm32-wasip1 -p ffh-web` (`tools/web-site.sh` builds
//! the whole site). Natively the crate is empty: everything is `cfg(target_arch = "wasm32")`.
//!
//! Exports: `ffh_alloc(len) -> ptr`, `ffh_free(ptr, len)`, `ffh_init(ptr, len) -> 0 | 1`,
//! `ffh_request(ptr, len)`. Imports (module `ffh`): `reply(token, ptr, len)`, `publish(ptr, len)`,
//! `pull(block) -> len | 0 | -1`, `pull_copy(ptr)`, `crash(ptr, len)`; plus
//! `wasi_snapshot_preview1` (std). Buffers: the page owns what it allocates with `ffh_alloc`
//! (it frees them with `ffh_free` once the call returns); the engine owns what it passes to an
//! import (valid during that call only: copy it).

#[cfg(target_arch = "wasm32")]
mod wasm;
