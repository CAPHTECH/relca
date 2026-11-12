use super::*;
use crate::degradation::input;

#[test]
fn degradation_cut_gate_does_not_read_an_optional_agency_table() {
    let src = input::edit(&input::base(), "clock 10min", "clock 3min");
    let mut r = check(&src);
    assert_eq!(r.properties[0].verdict, Verdict::Satisfied);
    assert_eq!(r.bounds.truncated, Some(true));
    r.agency = None;
    assert_eq!(finish(r, Vec::new()).verdict, Verdict::Inconclusive);

    let mut s = simulate(&input::source("latent-ok"));
    assert_eq!(s.verdict, Verdict::Satisfied);
    s.agency = None;
    s.bounds.truncated = Some(true);
    assert_eq!(seal_simulation(s).verdict, Verdict::Inconclusive);
}
