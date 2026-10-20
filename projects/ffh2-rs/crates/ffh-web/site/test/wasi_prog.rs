// The WASI test program of wasi.test.mjs: compiled for wasm32-wasip1 and run under wasi.js (an
// in-memory file system), and compiled natively and run in a temporary directory; the two runs
// must print the same lines on stdout (std::fs semantics as Rust sees them on Linux). It prints
// a checksum of its lines last. Paths are under $PROG_DIR. Not part of the cargo workspace:
//   rustc --edition 2021 -O --target wasm32-wasip1 wasi_prog.rs -o prog.wasm
use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::hash::{BuildHasher, Hasher};
use std::io::{Read, Seek, SeekFrom, Write};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Out {
    lines: Vec<String>,
}

impl Out {
    fn line(&mut self, s: impl Into<String>) {
        let s = s.into();
        println!("{s}");
        self.lines.push(s);
    }
}

/// ENOTEMPTY: std's wasi ErrorKind table leaves it Uncategorized (its Display is still
/// "Directory not empty"), so it is told by number.
#[cfg(target_os = "wasi")]
const ENOTEMPTY: i32 = 55;
#[cfg(not(target_os = "wasi"))]
const ENOTEMPTY: i32 = 39;

fn kind<T>(r: std::io::Result<T>) -> String {
    match r {
        Ok(_) => "ok".to_string(),
        Err(e) if e.raw_os_error() == Some(ENOTEMPTY) => "DirectoryNotEmpty".to_string(),
        Err(e) => format!("{:?}", e.kind()),
    }
}

fn listing(dir: &str) -> String {
    let mut v: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .map(|e| {
            let e = e.unwrap();
            let t = e.file_type().unwrap();
            let name = e.file_name().into_string().unwrap();
            let len = if t.is_file() { e.metadata().unwrap().len().to_string() } else { "-".to_string() };
            format!("{}{}:{}", name, if t.is_dir() { "/" } else { "" }, len)
        })
        .collect();
    v.sort();
    v.join(",")
}

fn main() {
    let base = std::env::var("PROG_DIR").expect("PROG_DIR");
    let p = |rel: &str| format!("{base}/{rel}");
    let mut o = Out { lines: Vec::new() };

    // The environment.
    o.line(format!("env greeting={}", std::env::var("PROG_GREETING").unwrap_or_default()));
    o.line(format!("env missing={:?}", std::env::var("PROG_MISSING").is_err()));

    // Directories.
    fs::create_dir_all(p("a/b/c")).unwrap();
    fs::create_dir_all(p("a/b/c")).unwrap();
    o.line(format!("create_dir existing: {}", kind(fs::create_dir(p("a/b")))));
    o.line(format!("create_dir no parent: {}", kind(fs::create_dir(p("x/y")))));
    o.line(format!("is_dir a/b: {} is_file: {}", fs::metadata(p("a/b")).unwrap().is_dir(), fs::metadata(p("a/b")).unwrap().is_file()));

    // Whole files.
    fs::write(p("a/hello.txt"), "hello world\n").unwrap();
    o.line(format!("read: {:?}", fs::read_to_string(p("a/hello.txt")).unwrap()));
    o.line(format!("len: {}", fs::metadata(p("a/hello.txt")).unwrap().len()));

    // Append.
    for i in 0..2 {
        let mut f = OpenOptions::new().append(true).open(p("a/hello.txt")).unwrap();
        writeln!(f, "line{i}").unwrap();
    }
    let mut f = OpenOptions::new().append(true).create(true).open(p("a/new.txt")).unwrap();
    f.write_all(b"appended").unwrap();
    drop(f);
    o.line(format!("after append: {:?} {:?}", fs::read_to_string(p("a/hello.txt")).unwrap(), fs::read_to_string(p("a/new.txt")).unwrap()));

    // Truncate on create.
    File::create(p("a/new.txt")).unwrap();
    o.line(format!("truncated len: {}", fs::metadata(p("a/new.txt")).unwrap().len()));

    // Seek, positional writes, set_len.
    let mut f = OpenOptions::new().read(true).write(true).create(true).truncate(true).open(p("a/seek.bin")).unwrap();
    f.write_all(b"0123456789").unwrap();
    f.seek(SeekFrom::Start(3)).unwrap();
    f.write_all(b"xx").unwrap();
    let here = f.stream_position().unwrap();
    f.seek(SeekFrom::End(-2)).unwrap();
    let mut two = [0u8; 2];
    f.read_exact(&mut two).unwrap();
    o.line(format!("seek: pos {} tail {:?}", here, String::from_utf8_lossy(&two)));
    f.seek(SeekFrom::Start(14)).unwrap();
    f.write_all(b"!").unwrap();
    f.set_len(16).unwrap();
    f.flush().unwrap();
    o.line(format!("hole: {:?}", fs::read(p("a/seek.bin")).unwrap()));
    f.set_len(4).unwrap();
    o.line(format!("set_len 4: {:?} meta {}", fs::read_to_string(p("a/seek.bin")).unwrap(), f.metadata().unwrap().len()));
    f.set_len(6).unwrap();
    o.line(format!("set_len 6: {:?}", fs::read(p("a/seek.bin")).unwrap()));
    o.line(format!("seek negative: {}", kind(f.seek(SeekFrom::Current(-100)))));
    drop(f);

    // A read-only handle refuses writes; a write-only one reads nothing.
    let mut ro = File::open(p("a/hello.txt")).unwrap();
    o.line(format!("write to read-only: err {}", ro.write_all(b"x").is_err()));
    let mut wo = OpenOptions::new().write(true).open(p("a/hello.txt")).unwrap();
    let mut s = String::new();
    o.line(format!("read from write-only: err {}", wo.read_to_string(&mut s).is_err()));

    // Errors.
    o.line(format!("open missing: {}", kind(File::open(p("a/nope.txt")))));
    o.line(format!("open in missing dir: {}", kind(File::open(p("zz/nope.txt")))));
    o.line(format!("create_new existing: {}", kind(OpenOptions::new().write(true).create_new(true).open(p("a/hello.txt")))));
    o.line(format!("remove missing: {}", kind(fs::remove_file(p("a/nope.txt")))));
    o.line(format!("remove_dir not empty: {}", kind(fs::remove_dir(p("a/b")))));
    o.line(format!("remove_file on dir: {}", kind(fs::remove_file(p("a/b")))));
    o.line(format!("read_dir on file: {}", kind(fs::read_dir(p("a/hello.txt")))));
    o.line(format!("file as dir: {}", kind(File::open(p("a/hello.txt/x")))));
    o.line(format!("create over dir: {}", kind(File::create(p("a/b")))));
    o.line(format!("exists: {} {}", fs::metadata(p("a/hello.txt")).is_ok(), fs::metadata(p("a/nope")).is_ok()));

    // Rename: a file, over a file, a directory with its contents, the error cases.
    fs::rename(p("a/hello.txt"), p("a/b/moved.txt")).unwrap();
    o.line(format!("renamed: old {} new {:?}", fs::metadata(p("a/hello.txt")).is_ok(), fs::read_to_string(p("a/b/moved.txt")).unwrap()));
    fs::write(p("a/tmp.txt"), "replacement").unwrap();
    fs::rename(p("a/tmp.txt"), p("a/b/moved.txt")).unwrap();
    o.line(format!("replaced: {:?} tmp {}", fs::read_to_string(p("a/b/moved.txt")).unwrap(), fs::metadata(p("a/tmp.txt")).is_ok()));
    fs::rename(p("a/b"), p("a/b2")).unwrap();
    o.line(format!("dir renamed: {} | {}", listing(&p("a")), listing(&p("a/b2"))));
    o.line(format!("via ..: {:?}", fs::read_to_string(p("a/../a/b2/./moved.txt")).unwrap()));
    fs::create_dir(p("a/empty")).unwrap();
    fs::create_dir_all(p("a/full/x")).unwrap();
    o.line(format!("rename dir over file: {}", kind(fs::rename(p("a/empty"), p("a/b2/moved.txt")))));
    o.line(format!("rename file over dir: {}", kind(fs::rename(p("a/b2/moved.txt"), p("a/empty")))));
    o.line(format!("rename dir over full dir: {}", kind(fs::rename(p("a/empty"), p("a/full")))));
    o.line(format!("rename dir into itself: {}", kind(fs::rename(p("a/full"), p("a/full/x/y")))));
    fs::create_dir(p("a/full/e2")).unwrap();
    fs::rename(p("a/empty"), p("a/full/e2")).unwrap();
    o.line(format!("dir over empty dir: {}", listing(&p("a/full"))));
    o.line(format!("rename missing: {}", kind(fs::rename(p("a/none"), p("a/none2")))));

    // Copy, metadata times.
    fs::copy(p("a/b2/moved.txt"), p("a/copy.txt")).unwrap();
    o.line(format!("copy: {:?}", fs::read_to_string(p("a/copy.txt")).unwrap()));
    let m = fs::metadata(p("a/copy.txt")).unwrap();
    let age = SystemTime::now().duration_since(m.modified().unwrap()).unwrap_or(Duration::ZERO);
    o.line(format!("mtime recent: {}", age < Duration::from_secs(60)));

    // An open file outlives its name.
    fs::write(p("a/ghost.txt"), "boo").unwrap();
    let mut g = File::open(p("a/ghost.txt")).unwrap();
    fs::remove_file(p("a/ghost.txt")).unwrap();
    let mut s = String::new();
    g.read_to_string(&mut s).unwrap();
    o.line(format!("unlinked but open: {s:?} exists {}", fs::metadata(p("a/ghost.txt")).is_ok()));

    // Many entries with long names (readdir refills its buffer), then remove_dir_all (removes
    // while it lists).
    fs::create_dir_all(p("many/sub")).unwrap();
    for i in 0..60 {
        let name = format!("{:03}-{}", i, "n".repeat(i * 3));
        fs::write(p(&format!("many/{name}")), vec![b'z'; i]).unwrap();
        if i % 7 == 0 {
            fs::write(p(&format!("many/sub/{name}")), "s").unwrap();
        }
    }
    let n = fs::read_dir(p("many")).unwrap().count();
    let total: u64 = fs::read_dir(p("many")).unwrap().map(|e| e.unwrap().metadata().unwrap().len()).filter(|_| true).sum::<u64>() - fs::metadata(p("many/sub")).unwrap().len();
    o.line(format!("many: {n} entries, {total} bytes, sub {}", fs::read_dir(p("many/sub")).unwrap().count()));
    fs::remove_dir_all(p("many")).unwrap();
    o.line(format!("remove_dir_all: exists {}", fs::metadata(p("many")).is_ok()));

    // Remove files and directories.
    fs::remove_file(p("a/copy.txt")).unwrap();
    fs::remove_dir(p("a/b/../full/x")).unwrap_err();
    fs::remove_dir(p("a/full/x")).unwrap();
    o.line(format!("final: {}", listing(&p("a"))));

    // Clocks and sleep.
    let t0 = Instant::now();
    std::thread::sleep(Duration::from_millis(30));
    let slept = t0.elapsed();
    o.line(format!("sleep ok: {}", slept >= Duration::from_millis(30) && slept < Duration::from_secs(5)));
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
    o.line(format!("systemtime ok: {}", now > 1_577_836_800));

    // HashMap (its keys come from random_get): the contents are deterministic, the hasher's
    // output is not (printed on stderr, compared across runs by the test).
    let mut h: HashMap<u32, u64> = HashMap::new();
    for i in 0..1000u32 {
        *h.entry(i % 97).or_default() += i as u64;
    }
    let sum: u64 = h.values().sum();
    o.line(format!("hashmap: {} keys, sum {}", h.len(), sum));
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u64(42);
    eprintln!("random hash {:016x}", hasher.finish());
    eprint!("stderr without newline");

    let mut ck: u64 = 0xcbf29ce484222325;
    for l in &o.lines {
        for b in l.bytes().chain(std::iter::once(b'\n')) {
            ck ^= b as u64;
            ck = ck.wrapping_mul(0x100000001b3);
        }
    }
    println!("checksum {ck:016x} over {} lines", o.lines.len());
}
