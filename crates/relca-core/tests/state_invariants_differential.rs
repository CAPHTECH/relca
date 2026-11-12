//! Each interpreter is checked against hand oracles before mutual comparison.
#[path = "support/state_invariant_acceptance.rs"]
mod h;

#[test]
fn all77_ordinary_runtime_inputs_reference_complete_without_exclusions() {
    let mut count = 0;
    for n in h::names() {
        if h::invocation(&n).is_none()
            && !["invalid", "unsupported"].contains(&h::expected(&n)["verdict"].as_str().unwrap())
        {
            h::both(&n);
            count += 1;
        }
    }
    assert_eq!(count, 77);
}

#[test]
fn selected_alias_compare_and_preserve_inputs_have_independent_reference_hand_oracles() {
    use relca_core::{Verdict, engine, lower, reference, syntax};
    let mut count = 0;
    for name in [
        "sinv-alias",
        "sinv-compare-exhaustive",
        "sinv-compare-exhaustive.broken-bad-plan",
        "sinv-preserve",
        "sinv-preserve.broken-vacuous",
    ] {
        let s = h::source(name);
        let a = syntax::parse(&s).unwrap();
        let plans = if name == "sinv-alias" {
            ["X", "Y"]
        } else if name.starts_with("sinv-preserve") {
            ["From", "To"]
        } else {
            ["A", "B"]
        };
        for plan in plans {
            let m = lower::lower_configuration(&a, plan).unwrap().model;
            let e = engine::explore(&m);
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete);
            count += 1;
            let want = if name.ends_with("broken-bad-plan") && plan == "B" {
                Verdict::Violated
            } else if name.ends_with("broken-vacuous") && plan == "To" {
                Verdict::Vacuous
            } else {
                Verdict::Satisfied
            };
            assert_eq!(r.verdict(), want);
            for o in [&e, &r.outcome] {
                assert_eq!(o.properties[0].verdict, want);
                if name.starts_with("sinv-preserve") {
                    assert_eq!(o.terminals[0].elapsed.min, 2);
                    assert_eq!(o.terminals[0].work.min, 2);
                } else {
                    assert_eq!(o.terminals.len(), 1);
                    assert_eq!(o.terminals[0].elapsed.min, 1);
                    assert_eq!(o.terminals[0].work.min, 1);
                }
            }
            assert_eq!(e.properties, r.outcome.properties);
            assert_eq!(e.terminals, r.outcome.terminals);
        }
    }
    assert_eq!(count, 10);
}
macro_rules! both {($($test:ident=>$base:literal),+ $(,)?)=>{$(#[test]fn $test(){
 for n in h::names().into_iter().filter(|n|n==$base||n.starts_with(&format!("{}.broken-",$base))) {
   if !["invalid","unsupported"].contains(&h::expected(&n)["verdict"].as_str().unwrap()) {h::both(&n);}
 }
})+};}
both! {
 approval=>"sinv-approval",rewrite=>"sinv-rewrite",vacuous=>"sinv-vacuous",clock=>"sinv-clock",
 same_tick=>"sinv-transient",latent=>"sinv-latent",cut=>"sinv-cut",shared=>"sinv-shared",
 ended=>"sinv-ended-case",ended_clean=>"sinv-ended-clean",range=>"sinv-range",receipt=>"sinv-initial",
 start=>"sinv-start-effect",labels=>"sinv-label",cancel=>"sinv-cancel",sticky=>"sinv-sticky",
 rejected=>"sinv-rejected",bool_type=>"sinv-bool-type",grouping=>"sinv-grouping",future=>"sinv-future",
 seed=>"sinv-seed",positive_mass_only=>"sinv-seed-mass",bundle=>"sinv-bundle",no_prune=>"sinv-no-prune",
 conjunction=>"sinv-conjunction",integer_collision=>"sinv-integer-collision"
}
#[test]
fn immediate_cancel_and_one_arrival_routes_are_independently_complete() {
    use relca_core::{engine, reference};
    let s = h::source("sinv-cancel");
    let s = h::change(&s, "cooperative ack_within 1min", "immediate");
    for arrivals in ["", "arrivals fixed [0min]"] {
        let t = h::change(
            &s,
            "  contract Controls",
            &format!("{arrivals}\n  contract Controls"),
        );
        let m = h::model(&t);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(r.verdict(), relca_core::Verdict::Satisfied);
        assert_eq!(e.properties, r.outcome.properties);
        for o in [&e, &r.outcome] {
            let v = h::projection(&m, o);
            if arrivals.is_empty() {
                assert_eq!(v["terminals"][0]["elapsed"]["min"], 1);
                assert_eq!(v["terminals"][0]["work"]["min"], 2);
            } else {
                h::case_terminal(&v, 1, "done", (1, 1), (2, 2));
            }
        }
    }
}
