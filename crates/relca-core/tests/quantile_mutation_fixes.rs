#[path = "support/quantile_acceptance.rs"]
mod h;
#[path = "support/quantile_mutation_inputs.rs"]
mod input;
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::Value;

fn check_case(id: &str) {
    let c = input::cases().into_iter().find(|c| c.id == id).unwrap();
    let m = lower::lower(&syntax::parse(&c.source).unwrap()).unwrap();
    for independent in [false, true] {
        let q = if c.sampled {
            if independent {
                let r = reference::simulate(&m);
                assert_eq!(r.stop, reference::StopReason::Complete, "{id}");
                r.outcome.quality
            } else {
                engine::simulate(&m).quality
            }
        } else if independent {
            let r = reference::explore(&m);
            assert_eq!(r.stop, reference::StopReason::Complete, "{id}");
            r.outcome.quality
        } else {
            engine::explore(&m).quality
        };
        assert_eq!(
            serde_json::to_value(q).unwrap()["quantiles"],
            c.rows,
            "{id}, reference={independent}: hand oracle"
        );
    }
    let v = if c.sampled {
        serde_json::to_value(report::simulate(&c.source)).unwrap()
    } else {
        serde_json::to_value(report::check(&c.source)).unwrap()
    };
    assert_eq!(
        v["verdict"], c.verdict,
        "{id}: quantity cannot change verdict"
    );
}
macro_rules! cases {
    ($($id:ident),* $(,)?) => {$ (#[test] fn $id() {check_case(stringify!($id));})*};
}
cases!(
    f16a,
    f18l,
    f18s1,
    f18r,
    f18s,
    f19sa_censor_never,
    f25s,
    f24i,
    f40b,
    within_censor_arrivals,
    within_censor_legacy,
    float_p_sample,
    float_p_exhaustive,
    never_censor_reference
);

#[test]
fn arithmetic_cut_preserves_old_diagnostics_verdict_and_whole_report() {
    let source = input::arithmetic();
    let plain = h::no_quantiles(&source);
    let full = serde_json::to_value(report::check(&source)).unwrap();
    let old = serde_json::to_value(report::check(&plain)).unwrap();
    assert_eq!(
        full["quality"]["quantiles"][0]["value"]["reason"],
        "truncated"
    );
    assert_eq!(old["verdict"], "violated");
    let codes: Vec<_> = old["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].clone())
        .collect();
    assert_eq!(
        codes,
        vec![Value::from("W-DEADLOCK-0"), Value::from("W-BUDGET-0")]
    );
    assert_eq!(full["diagnostics"], old["diagnostics"]);
    assert_eq!(h::old_projection(full), h::old_projection(old));
}
