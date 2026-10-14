//! Every Python callback named in the game XML must resolve to a Rust port.

#[test]
fn all_xml_python_callbacks_resolve() {
    let d = ffh_data::global();
    let t = ffh_core::ffh::dispatch::tables(d);
    if !t.unresolved.is_empty() {
        eprintln!("{} unresolved Python callbacks:", t.unresolved.len());
        for u in &t.unresolved {
            eprintln!("  {u}");
        }
    }
    assert!(t.unresolved.is_empty(), "{} unresolved Python callbacks", t.unresolved.len());
}
