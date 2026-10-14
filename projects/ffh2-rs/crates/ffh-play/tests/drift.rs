//! T9: controller drift (design §8). Every method of ffh-core's `Controller` trait has a human
//! policy in `human::POLICIES` (and `HumanController` implements it explicitly). Brittle by
//! design: a new hook added to the trait fails here until the human side decides "deferred or
//! blocking" (docs/HUMAN.md checklist).

use ffh_play::human::{Policy, POLICIES};

const CONTROLLER_RS: &str = include_str!("../../ffh-core/src/controller.rs");
const HUMAN_RS: &str = include_str!("../src/human.rs");

/// `fn` names declared at the top level of the block that starts at `header`.
fn fns_in_block(src: &str, header: &str) -> Vec<String> {
    let start = src.find(header).unwrap_or_else(|| panic!("{header:?} not found"));
    let body = &src[start..];
    let open = body.find('{').unwrap();
    let mut depth = 0;
    let mut names = Vec::new();
    let mut line_start = true;
    let bytes = body.as_bytes();
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    break;
                }
            }
            b'\n' => line_start = true,
            b' ' | b'\t' => {}
            _ => {
                if line_start && depth == 1 && body[i..].starts_with("fn ") {
                    let rest = &body[i + 3..];
                    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_')).unwrap();
                    names.push(rest[..end].to_string());
                }
                line_start = false;
            }
        }
        i += 1;
    }
    names
}

#[test]
fn t9_every_controller_method_has_a_human_policy() {
    let trait_fns = fns_in_block(CONTROLLER_RS, "pub trait Controller");
    assert!(trait_fns.len() >= 15, "parsed {trait_fns:?}");
    let policies: Vec<&str> = POLICIES.iter().map(|(n, _)| *n).collect();
    for f in &trait_fns {
        assert!(policies.contains(&f.as_str()), "Controller::{f} has no human policy: add it to human::POLICIES (deferred or blocking?)");
    }
    for p in &policies {
        assert!(trait_fns.iter().any(|f| f == p), "human::POLICIES lists {p}, which Controller no longer has");
    }
    let mut sorted = policies.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(sorted.len(), policies.len(), "duplicate policy entries");

    // HumanController implements every method itself (no silently inherited default).
    let impl_fns = fns_in_block(HUMAN_RS, "impl Controller for HumanController");
    for f in &trait_fns {
        assert!(impl_fns.contains(f), "HumanController does not implement Controller::{f}");
    }
    assert!(POLICIES.iter().any(|(_, p)| *p == Policy::Deferred) && POLICIES.iter().any(|(_, p)| *p == Policy::Blocking));
}
