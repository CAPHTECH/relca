//! Hand-written source edits, not a call into the production AST generator.
#[allow(dead_code)]
#[path = "mutation.rs"]
mod acceptance;
use acceptance::{FIXTURES, hand_edits, source};
use relca_core::{engine, lower, reference, report, syntax};

fn compare(name: &str, s: &str) {
    let ast = syntax::parse(s).expect("independent input must parse");
    let m = lower::lower(&ast).expect("independent input must lower");
    let e = engine::explore(&m);
    let mut r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::Complete, "no exclusions");
    // Reference includes the lowerer's static diagnostic prefix; engine returns
    // runtime diagnostics only. Assert that prefix before aligning the seams.
    assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
    r.outcome.diagnostics.drain(..m.diagnostics.len());
    for out in [&e, &r.outcome] {
        assert!(!out.truncated);
        let expected: Vec<_> = if name == "mut-guard" {
            let first = !s.contains("if po == pass");
            let second = !s.contains("if approval == pass");
            vec![
                relca_core::Verdict::Satisfied,
                if first {
                    relca_core::Verdict::Violated
                } else {
                    relca_core::Verdict::Satisfied
                },
                if second {
                    relca_core::Verdict::Violated
                } else {
                    relca_core::Verdict::Satisfied
                },
            ]
        } else if name == "mut-origin.broken-route-only" {
            // safety, static contest, runtime contest_response (ADR 0022).
            vec![
                relca_core::Verdict::Satisfied,
                relca_core::Verdict::Satisfied,
                relca_core::Verdict::Satisfied,
            ]
        } else if name == "mut-gate" {
            vec![relca_core::Verdict::Satisfied, relca_core::Verdict::Vacuous]
        } else {
            vec![relca_core::Verdict::Satisfied]
        };
        assert_eq!(
            out.properties.iter().map(|p| p.verdict).collect::<Vec<_>>(),
            expected,
            "{name}: hand clauses before mutual comparison"
        );
    }
    assert_eq!(e, r.outcome, "full Outcome, not just verdict");
}
#[test]
fn m_ten_baselines_and_286_hand_edits_complete296_exclusions0() {
    let (mut baselines, mut edits) = (0, 0);
    for name in FIXTURES {
        if name == "mut-empty.broken-invalid" || name == "mut-gate.broken-unsupported" {
            continue;
        }
        compare(name, &source(name));
        baselines += 1;
        for s in hand_edits(name) {
            compare(name, &s);
            edits += 1;
            let v = report::check(&s);
            assert!(matches!(
                v.verdict,
                relca_core::Verdict::Satisfied | relca_core::Verdict::Violated
            ));
        }
    }
    assert_eq!((baselines, edits, baselines + edits), (10, 286, 296));
}

#[test]
fn m_every_generated_report_matches_independent_source_semantics() {
    for name in FIXTURES {
        let v = acceptance::run(&source(name));
        acceptance::hand(name, &v);
        let expected = hand_edits(name);
        let actual: Vec<_> = v["mutations"]
            .as_array()
            .unwrap()
            .iter()
            .enumerate()
            .filter(|(i, _)| [0, 2, 8, 9].contains(i))
            .map(|(_, r)| r)
            .flat_map(|r| r["mutants"].as_array().unwrap())
            .filter(|m| m["result"] != "not_run")
            .collect();
        assert_eq!(actual.len(), expected.len());
        for (m, s) in actual.iter().zip(expected) {
            let mut want = serde_json::to_value(report::check(&s)).unwrap();
            let mut got = m["report"].clone();
            scrub(&mut want);
            scrub(&mut got);
            assert_eq!(got, want, "{name}/{}: independent edit", m["id"]);
        }
    }
}
fn scrub(v: &mut serde_json::Value) {
    match v {
        serde_json::Value::Object(o) => {
            o.remove("sha256");
            o.remove("span");
            for x in o.values_mut() {
                scrub(x)
            }
        }
        serde_json::Value::Array(a) => {
            for x in a {
                scrub(x)
            }
        }
        _ => {}
    }
}
