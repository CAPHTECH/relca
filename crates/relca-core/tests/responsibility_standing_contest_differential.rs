//! The 13th population: hand runtime oracle before engine/reference equality.
use relca_core::{
    Verdict, engine, ir, lower,
    reference::{self, StopReason},
    report, syntax,
};
use serde_json::{Value, json};
use std::{fs, path::Path};

const FIXTURES: [&str; 19] = [
    "resp-contest",
    "resp-contest.broken-no-authority",
    "resp-contest.broken-no-permission",
    "resp-contest.broken-route-excluded",
    "resp-contest.broken-zero-deadline",
    "resp-cut",
    "resp-cut.broken-no-explicit-cut",
    "resp-diff",
    "resp-diff.broken-no-authorized-decision",
    "resp-missing-duty",
    "resp-missing-duty.broken-no-duty",
    "resp-origin",
    "resp-origin.broken-no-direct-read",
    "resp-refusal",
    "resp-refusal.broken-cut-guard",
    "resp-standing",
    "resp-standing.broken-not-party",
    "resp-state",
    "resp-state.broken-no-direct-read",
];
fn hand(model: &ir::Model, out: &relca_core::outcome::Outcome, expected: &Value, label: &str) {
    assert_eq!(
        out.truncated,
        expected["bounds"]["truncated"].as_bool().unwrap(),
        "{label}"
    );
    assert!(
        out.diagnostics
            .iter()
            .all(|d| d.severity != relca_core::diag::Severity::Error),
        "{label}"
    );
    let expected_props = expected["properties"].as_array().unwrap();
    assert_eq!(
        out.properties.len(),
        expected_props.len(),
        "{label}: property inventory"
    );
    for (p, e) in out.properties.iter().zip(expected_props) {
        let verdict: Verdict = serde_json::from_value(e["verdict"].clone()).unwrap();
        assert_eq!(p.verdict, verdict, "{label}: {e}");
        assert!(p.cases.is_none(), "legacy and structural rows are global");
        assert!(p.probability.is_none());
    }
    let mut terms:Vec<_>=out.terminals.iter().map(|t|json!({"end":model.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r|json!({"min":r.min,"max":r.max}))})).collect();
    let mut want = expected["terminals"].as_array().unwrap().clone();
    terms.sort_by(|a, b| a["end"].as_str().cmp(&b["end"].as_str()));
    want.sort_by(|a, b| a["end"].as_str().cmp(&b["end"].as_str()));
    assert_eq!(terms, want, "{label}: hand time/work/result arithmetic");
}
#[test]
fn r_population_19_dirs_21_inputs_20_runtime_1_static_complete20_exclusions0() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut names: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir() && e.file_name().to_string_lossy().starts_with("resp-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    let (mut inputs, mut accepted, mut rejected, mut compared, mut complete) = (0, 0, 0, 0, 0);
    let mut errors = Vec::new();
    let mut exclusions = Vec::new();
    for name in names {
        let dir = root.join(&name);
        for file in ["model.relca", "calc.md", "expected.json"] {
            assert!(dir.join(file).is_file());
        }
        let s = fs::read_to_string(dir.join("model.relca")).unwrap();
        let expected: Value =
            serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
        let choices: &[Option<&str>] = if name.starts_with("resp-diff") {
            &[Some("A"), Some("B")]
        } else {
            &[None]
        };
        for &selection in choices {
            inputs += 1;
            let label = format!("{name}[{selection:?}]");
            let e = match selection {
                Some("A") => &expected["from"]["report"],
                Some("B") => &expected["to"]["report"],
                _ => &expected,
            };
            let parsed = syntax::parse(&s).and_then(|a| match selection {
                Some(c) => lower::lower_configuration(&a, c).map(|c| c.model),
                None => lower::lower(&a),
            });
            if name == "resp-standing.broken-not-party" {
                rejected += 1;
                let d = parsed.expect_err("nonparty Standing must fail before exploration");
                assert!(d.iter().any(|d| d.code == "E-CON-0"), "{label}: {d:?}");
                assert_eq!(report::check(&s).verdict, Verdict::Invalid);
                continue;
            }
            let model = match parsed {
                Ok(m) => m,
                Err(d) => {
                    errors.push(format!("{label}: unexpected static rejection {d:?}"));
                    continue;
                }
            };
            accepted += 1;
            let r = reference::explore(&model);
            let en = engine::explore(&model);
            if r.stop == StopReason::Complete {
                complete += 1;
            } else {
                exclusions.push(format!("{label}: {:?}", r.stop));
            }
            hand(&model, &r.outcome, e, &format!("{label}/reference"));
            hand(&model, &en, e, &format!("{label}/engine"));
            assert_eq!(
                serde_json::to_value(r.verdict()).unwrap(),
                e["verdict"],
                "{label}"
            );
            assert_eq!(
                en.properties, r.outcome.properties,
                "{label}: differential property"
            );
            assert_eq!(
                en.terminals, r.outcome.terminals,
                "{label}: differential terminals"
            );
            assert_eq!(en.truncated, r.outcome.truncated);
            let mut independent = r.outcome.clone();
            assert!(independent.diagnostics.starts_with(&model.diagnostics));
            independent.diagnostics.drain(..model.diagnostics.len());
            assert_eq!(
                en, independent,
                "{label}: whole Outcome includes obligation and runtime tables"
            );
            let public = serde_json::to_value(match selection {
                Some(c) => report::check_configuration(&s, c),
                None => report::check(&s),
            })
            .unwrap();
            assert_eq!(public["properties"], e["properties"], "{label}: public IDs");
            assert_eq!(public["verdict"], e["verdict"]);
            compared += 1;
        }
    }
    assert_eq!(inputs, 21);
    assert_eq!(rejected, 1);
    assert!(errors.is_empty(), "{}", errors.join("\n"));
    assert_eq!(accepted, 20);
    assert_eq!(compared, 20);
    assert_eq!(complete, 20);
    assert!(
        exclusions.is_empty(),
        "zero exclusions: {}",
        exclusions.join("\n")
    );
}

#[test]
fn r_static_properties_survive_public_budget_zero() {
    for name in ["resp-contest", "resp-contest.broken-no-authority"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca");
        let s = fs::read_to_string(path).unwrap();
        let model = lower::lower(&syntax::parse(&s).unwrap()).unwrap();
        let full = engine::explore(&model);
        let cut = engine::explore_with_budget(&model, 0);
        assert!(cut.truncated);
        assert_eq!(cut.properties.len(), 3);
        assert_eq!(cut.properties[0].verdict, Verdict::Inconclusive);
        assert_eq!(cut.properties[1].verdict, full.properties[1].verdict);
        assert_eq!(cut.properties[1].cases, None);
        assert_eq!(cut.properties[2].verdict, Verdict::Inconclusive);
    }
}
