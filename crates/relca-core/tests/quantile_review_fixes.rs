//! Hand calculations and review boundaries: see quantile_review_fixes.md.
#[path = "support/quantile_acceptance.rs"]
mod h;
#[path = "support/quantile_review_inputs.rs"]
mod input;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};

fn model(s: &str) -> relca_core::ir::Model {
    lower::lower(&syntax::parse(s).expect("review input syntax")).expect("review input semantics")
}
fn strip_quantiles(q: &mut Option<relca_core::quality::Quality>) {
    let q = q.as_mut().expect("declared quality outcome");
    q.quantiles = None;
    q.checked.retain(|s| s != "quantiles");
    q.unchecked.retain(|s| s != "quantiles");
}
fn quality(s: &str, sampled: bool, independent: bool) -> Value {
    let m = model(s);
    if sampled {
        let out = if independent {
            let r = reference::simulate(&m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "no reference exclusion"
            );
            r.outcome
        } else {
            engine::simulate(&m)
        };
        serde_json::to_value(out.quality).unwrap()
    } else {
        let out = if independent {
            let r = reference::explore(&m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "no reference exclusion"
            );
            r.outcome
        } else {
            engine::explore(&m)
        };
        serde_json::to_value(out.quality).unwrap()
    }
}

macro_rules! closure_test {
    ($name:ident, $sampled:expr, $reference:expr, $positive:expr, $desk:expr) => {
        #[test]
        fn $name() {
            let s = if $desk {
                input::DESK.to_owned()
            } else if $positive {
                input::edit(input::CLOSED, "Ledger.n == 2", "Ledger.n == 1")
            } else {
                input::CLOSED.to_owned()
            };
            assert_eq!(
                quality(&s, $sampled, $reference)["quantiles"],
                input::rows($sampled, $positive, $desk),
                "closed miss is absorbing, never a late shared hit"
            );
        }
    };
}
closure_test!(r1_engine_exhaustive, false, false, false, false);
closure_test!(r1_reference_exhaustive, false, true, false, false);
closure_test!(r1_engine_simulation, true, false, false, false);
closure_test!(r1_reference_simulation, true, true, false, false);
closure_test!(r1_positive_engine_exhaustive, false, false, true, false);
closure_test!(r1_positive_reference_exhaustive, false, true, true, false);
closure_test!(r1_positive_engine_simulation, true, false, true, false);
closure_test!(r1_positive_reference_simulation, true, true, true, false);
closure_test!(m1_engine_exhaustive, false, false, false, true);
closure_test!(m1_reference_exhaustive, false, true, false, true);
closure_test!(m1_engine_simulation, true, false, false, true);
closure_test!(m1_reference_simulation, true, true, false, true);

#[test]
fn m1_b6_exhaustive_cases_are_hand_miss_hit_in_both_interpreters() {
    let expected = json!([
        {"case_id":1,"arrival":0,"min":"0","max":"0","reason":null},
        {"case_id":2,"arrival":5,"min":"1","max":"1","reason":null}
    ]);
    let e = quality(input::DESK, false, false);
    let r = quality(input::DESK, false, true);
    assert_eq!(e["properties"][0]["cases"], expected, "engine vs hand");
    assert_eq!(r["properties"][0]["cases"], expected, "reference vs hand");
    assert_eq!(e, r, "complete independent quality, not merely quantiles");
}

macro_rules! b6_sample {
    ($name:ident, $reference:expr) => {
        #[test]
        fn $name() {
            let q = quality(input::DESK, true, $reference);
            assert_eq!(q["properties"][0]["cases"], json!([
                {"case_id":1,"arrival":0,"hits":0,"unknown":0,"trials":1},
                {"case_id":2,"arrival":5,"hits":1,"unknown":0,"trials":1}
            ]), "B6 also cannot reopen a closed miss");
            assert_eq!(q["properties"][0]["expected_count"], json!({"mean":"1","min":"1","max":"1","samples":1,"reason":null}));
        }
    };
}
b6_sample!(m1_b6_engine_simulation, false);
b6_sample!(m1_b6_reference_simulation, true);

#[test]
fn declarations_preserve_old_roots_statistics_diagnostics_and_actual_draws() {
    for s in [
        input::CLOSED.to_owned(),
        input::edit(input::CLOSED, "Ledger.n == 2", "Ledger.n == 1"),
        input::DESK.to_owned(),
        input::SELECTED.to_owned(),
    ] {
        let plain = h::no_quantiles(&s);
        for sampled in [false, true] {
            let run = |text: &str| {
                if sampled {
                    serde_json::to_value(report::simulate(text)).unwrap()
                } else {
                    serde_json::to_value(report::check(text)).unwrap()
                }
            };
            let full = run(&s);
            let old = run(&plain);
            assert_eq!(
                full["verdict"], old["verdict"],
                "quantile null is not a verdict"
            );
            assert_eq!(h::old_projection(full), h::old_projection(old));
        }
        let m = model(&s);
        let p = model(&plain);
        let mut e = engine::simulate(&m);
        let mut ep = engine::simulate(&p);
        assert_eq!(
            e.replications, ep.replications,
            "all selected-path statistics and complete RNG audit keys"
        );
        strip_quantiles(&mut e.quality);
        strip_quantiles(&mut ep.quality);
        assert_eq!(
            e, ep,
            "complete engine sample outcome, including nonserialized fields"
        );
        let mut r = reference::simulate(&m);
        let mut rp = reference::simulate(&p);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(rp.stop, reference::StopReason::Complete);
        assert_eq!(r.outcome.replications, rp.outcome.replications);
        strip_quantiles(&mut r.outcome.quality);
        strip_quantiles(&mut rp.outcome.quality);
        assert_eq!(
            r.outcome, rp.outcome,
            "complete reference sample statistics and diagnostics"
        );
        let mut e = engine::explore(&m);
        let mut ep = engine::explore(&p);
        strip_quantiles(&mut e.quality);
        strip_quantiles(&mut ep.quality);
        assert_eq!(e, ep, "complete engine exhaustive outcome");
        let mut r = reference::explore(&m);
        let mut rp = reference::explore(&p);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(rp.stop, reference::StopReason::Complete);
        strip_quantiles(&mut r.outcome.quality);
        strip_quantiles(&mut rp.outcome.quality);
        assert_eq!(
            r.outcome, rp.outcome,
            "complete reference exhaustive statistics and lower diagnostic prefix preserved"
        );
    }
}

#[test]
fn n1_unknown_cannot_hide_a_provably_insufficient_best_reached_mass() {
    // At clock 2: case 1 is a closed miss, case 2 is unknown. Even its
    // best completion gives D_hi/n=1/2 < p=1, so this is not truncated.
    let s = input::edit(input::CLOSED, "clock 5min", "clock 2min");
    for sampled in [false, true] {
        for independent in [false, true] {
            let q = quality(&s, sampled, independent);
            let v = if sampled {
                json!({"point":null,"observations":2,"censored":1,"reason":"beyond_reached_mass"})
            } else {
                json!({"lower":null,"upper":null,"basis":null,"reason":"beyond_reached_mass"})
            };
            assert_eq!(q["quantiles"][0]["value"], v);
            assert_eq!(q["quantiles"][0]["lower_bound"], Value::Null);
            assert_eq!(
                q["quantiles"][1]["value"]["reason"], "truncated",
                "positive uncertainty control for the conditional denominator"
            );
        }
    }
}

#[test]
fn m1_driver_observes_only_the_selected_arrivals_successor_not_its_early_alternative() {
    use sha2::{Digest, Sha256};
    // Independently reconstruct the documented categorical coin BEFORE running
    // the product. First half=finish at2; second half=survive until4.
    let key = "relca-sim-v1|seed=2|rep=1|case=1|site=duration.P.job|n=1";
    let hash = Sha256::digest(key.as_bytes());
    let coin = u64::from_be_bytes(hash[..8].try_into().unwrap());
    assert!(coin >= 1u64 << 63, "chosen seed must select the late half");
    let fixed = input::edit(
        input::SELECTED,
        "duration {2min:0.5, 4min:0.5}",
        "duration fixed 4min",
    );
    for s in [fixed.as_str(), input::SELECTED] {
        for independent in [false, true] {
            assert_eq!(
                quality(s, true, independent)["quantiles"],
                input::selected_rows()
            );
        }
    }
    // The public exhaustive control proves that a genuine earlier successor
    // exists only in the distributed member, not merely a fake observer flag.
    let median = input::edit(
        input::SELECTED,
        "metric quantile.All = quantile(1, case.Done)",
        "metric quantile.All = quantile(0.5, case.Done)",
    );
    let e = quality(&median, false, false);
    assert_eq!(
        e["quantiles"][0]["value"],
        json!({"lower":"2","upper":"2","basis":"exact","reason":null})
    );
    assert_eq!(
        e["quantiles"][1]["value"],
        json!({"lower":"4","upper":"4","basis":"exact","reason":null})
    );
    let sampled = engine::simulate(&model(input::SELECTED));
    assert!(
        sampled.replications[0].draws.iter().any(|d| d.key == key),
        "actual arrivals driver exercised the hand-audited coin"
    );
}
