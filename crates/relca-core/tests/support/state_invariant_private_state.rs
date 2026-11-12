//! Link as an engine child module under cfg(test), not as a public integration.
use super::*;
#[test]
fn state_invariant_monitor_history_key_distinguishes_activation_and_violation() {
    use std::hash::{Hash, Hasher};
    let source = include_str!("../../../../fixtures/sinv-initial/model.relca");
    let m = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
    let a = State::initial(&m, 0);
    let mut b = a.clone();
    b.monitors[0].activated = true;
    let mut c = b.clone();
    c.monitors[0].violated = true;
    assert_eq!(a.state_values, b.state_values);
    assert_eq!(b.state_values, c.state_values);
    assert_ne!(a, b);
    assert_ne!(b, c);
    let mut states = std::collections::HashSet::new();
    states.insert(a.clone());
    states.insert(b.clone());
    states.insert(c.clone());
    assert_eq!(states.len(), 3);
    let fingerprint = |s: &State| {
        let mut h = std::collections::hash_map::DefaultHasher::new();
        s.hash(&mut h);
        h.finish()
    };
    // Separate structural hashing requirement: a Hash-only omission can be a
    // collision without changing Eq-based semantics. Do not call it a semantic kill.
    assert_ne!(fingerprint(&a), fingerprint(&b));
    assert_ne!(fingerprint(&b), fingerprint(&c));
}
