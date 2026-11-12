//! Each interpreter is checked against hand arithmetic before pairwise equality.
#[path = "authority_scope.rs"]
mod acceptance;
use relca_core::{engine, lower, reference, syntax};
use serde_json::json;
#[test]
fn runtime31_complete_static9_excluded_and_both_match_hand_oracles() {
    let (mut complete, mut excluded) = (0, 0);
    for name in acceptance::NAMES {
        let hand = acceptance::expected(name);
        if hand["verdict"] == "invalid" || hand["verdict"] == "unsupported" {
            excluded += 1;
            continue;
        }
        let src = acceptance::source(name);
        let m = lower::lower(
            &syntax::parse(&src).unwrap_or_else(|e| panic!("{name} feature parse: {e:?}")),
        )
        .unwrap();
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete, "{name}");
        if name.starts_with("auth-requires-integration") {
            assert_eq!(
                r.statistics.normal_completions,
                usize::from(name.ends_with("broken-starts")),
                "rejection emits no finish/commit, normal completion does"
            );
            assert_eq!(r.statistics.cancellation_completions, 0);
        }
        assert_eq!(
            serde_json::to_value(r.verdict()).unwrap(),
            hand["verdict"],
            "reference hand verdict {name}"
        );
        let e = engine::explore(&m);
        for (who, o) in [("reference", &r.outcome), ("engine", &e)] {
            let hp = hand["properties"].as_array().unwrap();
            assert_eq!(o.properties.len(), hp.len());
            for (actual, want) in o.properties.iter().zip(hp) {
                assert_eq!(
                    serde_json::to_value(actual.verdict).unwrap(),
                    want["verdict"],
                    "{who} hand property {name}"
                );
                if let Some(cs) = want["cases"].as_array() {
                    let ac = actual.cases.as_ref().unwrap();
                    assert_eq!(ac.len(), cs.len());
                    for (a, e) in ac.iter().zip(cs) {
                        assert_eq!(json!(a.case_id.0 + 1), e["case_id"]);
                        assert_eq!(json!(a.arrival), e["arrival"]);
                        assert_eq!(json!(a.verdict), e["verdict"]);
                    }
                }
            }
            if let Some(cut) = hand["bounds"].get("truncated") {
                assert_eq!(json!(o.truncated), *cut, "{who} cut {name}");
            }
            let ts:Vec<_>=o.terminals.iter().map(|t|json!({"end":m.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|x|json!({"min":x.min,"max":x.max}))})).collect();
            assert_eq!(json!(ts), hand["terminals"], "{who} business {name}");
            if name.starts_with("auth-expiry-absolute") {
                let cs = o.case_outcomes.as_ref().unwrap();
                assert_eq!(cs.len(), 2);
                for (c, t) in cs.iter().zip([2, 7]) {
                    let x = c.terminals[0].completed_at.unwrap();
                    assert_eq!((x.min, x.max), (t, t), "{who} absolute completion {name}");
                }
            }
        }
        let mut ro = r.outcome;
        assert!(ro.diagnostics.starts_with(&m.diagnostics));
        ro.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, ro, "differential all fields {name}");
        complete += 1;
    }
    assert_eq!(
        (complete, excluded),
        (31, 9),
        "9 static rejection cases only; zero runtime exclusions"
    );
}
