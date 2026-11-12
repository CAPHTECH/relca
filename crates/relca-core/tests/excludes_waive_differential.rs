//! D1: 12 dirs / 12 inputs / 9 runtime Complete9 / 3 static / exclusions0.
//! Existing APIs compile before implementation; new syntax must fail at parse,
//! never by requiring a speculative Rust type in this acceptance target.
#[path = "excludes_waive.rs"]
mod acceptance;
#[path = "support/completion_oracle.rs"]
mod completion_oracle;
use relca_core::{Verdict, engine, ir, lower, outcome::Outcome, reference, syntax};
use serde_json::{Value, json};

fn hand(model: &ir::Model, actual: &Outcome, expected: &Value, added: &[&str]) {
    assert_eq!(
        actual.truncated,
        expected["bounds"]["truncated"].as_bool().unwrap()
    );
    assert_eq!(
        actual.properties.len(),
        expected["properties"].as_array().unwrap().len()
    );
    for (p, e) in actual
        .properties
        .iter()
        .zip(expected["properties"].as_array().unwrap())
    {
        assert_eq!(
            p.verdict,
            serde_json::from_value::<Verdict>(e["verdict"].clone()).unwrap(),
            "hand clause {}",
            e["id"]
        );
    }
    let ts: Vec<_> = actual.terminals.iter().map(|t| json!({"end":model.ends[t.end.0],"elapsed":{"min":t.elapsed.min,"max":t.elapsed.max},"work":{"min":t.work.min,"max":t.work.max},"result_at":t.result_at.map(|r| json!({"min":r.min,"max":r.max}))})).collect();
    assert_eq!(
        json!(ts),
        expected["terminals"],
        "hand completion arithmetic"
    );
    // ADR0025 migration: retain the old no-other-runtime-diagnostic assertion,
    // but first require every newly scoped warning from an independent old-IR
    // counterfactual query. Never silently filter by a W-UNREACH prefix.
    let hand = completion_oracle::warnings(model);
    let new: Vec<_> = actual
        .diagnostics
        .iter()
        .filter(|d| d.code.starts_with("W-UNREACH-"))
        .collect();
    assert_eq!(
        new.len(),
        hand.len(),
        "independent completion warning inventory"
    );
    for (d, h) in new.iter().zip(&hand) {
        assert_eq!(d.code, h["code"].as_str().unwrap());
        assert_eq!(serde_json::to_value(d.severity).unwrap(), "warning");
        assert_eq!(serde_json::to_value(d.span).unwrap(), h["span"]);
        assert!(
            d.message.contains(h["target"].as_str().unwrap())
                && d.message.contains(&model.process.name)
        );
    }
    // ADR 0043 (plan C06): the lowered model also carries exactly the hand
    // W-AGY-2/W-RESP-0 additions, in that order; nothing else is new.
    let c06: Vec<_> = actual
        .diagnostics
        .iter()
        .filter(|d| d.code == "W-AGY-2" || d.code == "W-RESP-0")
        .map(|d| d.code.as_str())
        .collect();
    assert_eq!(c06, added, "exact C06 additions");
    assert!(
        actual
            .diagnostics
            .iter()
            .all(|d| d.code == "W-CUT-0" || new.contains(&d) || added.contains(&d.code.as_str()))
    );
}

/// Hand C06 additions for acceptance::model: Officer [p] commits release, so a
/// called Officer.release is concentrated on one person (W-AGY-2) and, with
/// people and no responsibility metadata, its effect Actor warns (W-RESP-0).
fn inline_added(src: &str) -> &'static [&'static str] {
    if src.contains("Officer.release()") {
        &["W-AGY-2", "W-RESP-0"]
    } else {
        &[]
    }
}

fn both(src: &str, expected: &Value, added: &[&str]) {
    let model = lower::lower(&syntax::parse(src).expect("D1 accepted hand syntax"))
        .expect("D1 accepted hand semantics");
    let r = reference::explore(&model);
    assert_eq!(
        r.stop,
        reference::StopReason::Complete,
        "zero reference exclusions"
    );
    let e = engine::explore(&model);
    // Separate oracle assertions BEFORE inter-backend equality.
    // The reference outcome starts with the lowered model diagnostics (C06
    // additions included); the engine outcome holds runtime diagnostics only.
    hand(&model, &r.outcome, expected, added);
    hand(&model, &e, expected, &[]);
    assert_eq!(
        serde_json::to_value(r.verdict()).unwrap(),
        expected["verdict"]
    );
    let mut reference = r.outcome;
    assert!(reference.diagnostics.starts_with(&model.diagnostics));
    reference.diagnostics.drain(..model.diagnostics.len());
    // Includes the new ledger: Outcome::Eq must not omit obligations.
    assert_eq!(e, reference, "all runtime fields, not merely root verdict");
}

#[test]
fn population_12_inputs_9_runtime_complete9_static3_excluded0() {
    let mut names: Vec<_> = std::fs::read_dir(acceptance::root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("ctx-"))
        .collect();
    names.sort();
    let d1: std::collections::BTreeSet<_> = acceptance::NAMES.into_iter().collect();
    const D2: [&str; 12] = [
        "ctx-contest",
        "ctx-contest-branches",
        "ctx-contest-branches.broken-zero-capacity",
        "ctx-contest-clock",
        "ctx-contest-clock.broken-short-clock",
        "ctx-contest-metrics",
        "ctx-contest-metrics.broken-missing-route-rate",
        "ctx-contest-origin",
        "ctx-contest-origin.broken-arrival-clock",
        "ctx-contest-resource",
        "ctx-contest-resource.broken-slow-route",
        "ctx-contest.broken-late",
    ];
    let d2: std::collections::BTreeSet<_> = D2.into_iter().collect();
    assert!(d1.is_disjoint(&d2));
    let all: Vec<_> = d1.union(&d2).copied().collect();
    assert_eq!(names, all, "unknown ctx prefix is not silently excluded");
    let (mut inputs, mut runtime, mut rejected, mut complete) = (0, 0, 0, 0);
    for name in acceptance::NAMES {
        inputs += 1;
        let src = acceptance::source(name);
        let expected = acceptance::expected(name);
        if expected["verdict"] == "invalid" {
            let diagnostics = syntax::parse(&src)
                .and_then(|ast| lower::lower(&ast))
                .expect_err("static fault must fail closed");
            let code = expected["diagnostics"][0]["code"].as_str().unwrap();
            assert!(
                diagnostics.iter().any(|d| d.code == code),
                "{name}: {diagnostics:?}"
            );
            rejected += 1;
        } else {
            both(&src, &expected, acceptance::c06_added(name));
            runtime += 1;
            complete += 1;
        }
    }
    assert_eq!((inputs, runtime, rejected, complete), (12, 9, 3, 9));
}

#[test]
fn running_b_same_bundle_and_all_pending_have_independent_runtime_oracles() {
    let src = acceptance::model(
        "fork f { a = Gate.run(); b = B.run() }; join all f; end done",
        "excludes Gate.run.finish, B.run.start",
    );
    let expected = json!({"verdict":"satisfied","bounds":{"truncated":false},"properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"contract Controls #1 excludes","verdict":"satisfied"}],"terminals":[{"end":"done","elapsed":{"min":3,"max":3},"work":{"min":4,"max":4},"result_at":null}]});
    both(&src, &expected, inline_added(&src));
    for (target, status) in [
        ("Officer.release.commit", "satisfied"),
        ("B.run.finish", "satisfied"),
    ] {
        let src = acceptance::model(
            "o = Officer.release(); end done",
            &format!(
                "response delivery: Officer.release.finish -> one_of({target}) by 0min after Officer.release.finish otherwise unresolved; waive delivery on Officer.release.finish reason \"exception\" authorized_by G"
            ),
        );
        let expected = json!({"verdict":status,"bounds":{"truncated":false},"properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"contract Controls #1 response","verdict":"satisfied"},{"id":"contract Controls #2 waive","verdict":"satisfied"}],"terminals":[{"end":"done","elapsed":{"min":1,"max":1},"work":{"min":1,"max":1},"result_at":null}]});
        both(&src, &expected, inline_added(&src));
    }
    let src = acceptance::model(
        "g1 = Gate.run(); g2 = Gate.run(); o = Officer.release(); g3 = Gate.run(); w = Wait.run(); end done",
        "response delivery: Gate.run.finish -> one_of(B.run.finish) by 5min after Gate.run.finish otherwise unresolved; waive delivery on Officer.release.finish reason \"exception\" authorized_by G",
    );
    let expected = json!({"verdict":"violated","bounds":{"truncated":false},"properties":[{"id":"safety no_resource_overcommit","verdict":"satisfied"},{"id":"contract Controls #1 response","verdict":"violated"},{"id":"contract Controls #2 waive","verdict":"satisfied"}],"terminals":[{"end":"done","elapsed":{"min":10,"max":10},"work":{"min":10,"max":10},"result_at":null}]});
    both(&src, &expected, inline_added(&src));
}
