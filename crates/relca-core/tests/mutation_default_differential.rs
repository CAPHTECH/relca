//! Independent source edits and reference exploration, not mutation helpers.
#[path = "support/mutation_default_acceptance.rs"]
mod hand;
use relca_core::{engine, lower, mutation, reference, report, syntax};
use serde_json::Value;

fn scrub(v: &mut Value) {
    match v {
        Value::Object(o) => {
            o.remove("sha256");
            o.remove("span");
            for x in o.values_mut() {
                scrub(x);
            }
        }
        Value::Array(a) => {
            for x in a {
                scrub(x);
            }
        }
        _ => {}
    }
}

#[test]
fn every_new_report_matches_an_independently_parsed_source_edit() {
    for (name, e) in hand::oracle().as_object().unwrap() {
        let v = serde_json::to_value(mutation::mutate(&hand::source(name))).unwrap();
        for edit in e["hand_edits"].as_array().unwrap() {
            let m = v["mutations"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|r| r["mutants"].as_array().unwrap())
                .find(|m| m["id"] == edit["id"])
                .unwrap_or_else(|| panic!("missing {} for {name}", edit["id"]));
            let mut want =
                serde_json::to_value(report::check(edit["source"].as_str().unwrap())).unwrap();
            let mut got = m["report"].clone();
            scrub(&mut want);
            scrub(&mut got);
            assert_eq!(
                got, want,
                "{name}/{}: independently parsed edit",
                edit["id"]
            );
        }
    }
}

#[test]
fn hand_originals_and_source_edits_compare_full_outcome_with_reference() {
    let mut runtime = 0;
    let mut static_refusals = 0;
    let mut zero_time = 0;
    for (name, e) in hand::oracle().as_object().unwrap() {
        let mut sources = vec![hand::source(name)];
        sources.extend(
            e["hand_edits"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x["source"].as_str().unwrap().to_owned()),
        );
        for source in sources {
            let ast = match syntax::parse(&source) {
                Ok(a) => a,
                Err(_) => {
                    static_refusals += 1;
                    continue;
                }
            };
            let m = match lower::lower(&ast) {
                Ok(m) => m,
                Err(_) => {
                    static_refusals += 1;
                    continue;
                }
            };
            if name == "mut2-expiry.broken-zero-duration"
                || name.starts_with("mut2-expiry-zero-finish")
            {
                // Existing reference explicitly refuses invoked zero-duration
                // Tasks. Engine/report/CLI still check inclusive time0.
                zero_time += 1;
                continue;
            }
            let e = engine::explore(&m);
            let mut r = reference::explore(&m);
            assert_eq!(
                r.stop,
                reference::StopReason::Complete,
                "{name}: no silent runtime exclusions"
            );
            assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
            r.outcome.diagnostics.drain(..m.diagnostics.len());
            assert_eq!(e, r.outcome, "{name}: full runtime Outcome");
            runtime += 1;
        }
    }
    // ADR 0068 removes the two zero-time expiry mutants, not their originals:
    // 79 original inputs + 59 applicable manual edits. The 129 reference
    // comparisons and six static refusals are unchanged.
    assert_eq!(runtime + static_refusals + zero_time, 138);
    assert_eq!(zero_time, 3);
    assert_eq!(static_refusals, 6);
    assert_eq!(runtime, 129);
}
