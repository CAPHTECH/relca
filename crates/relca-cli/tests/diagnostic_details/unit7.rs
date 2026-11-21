use super::{
    blocker, invoke, location,
    unit5::{accepts, rejected},
};
use serde_json::Value;

#[test]
fn syntax_refusals_name_the_first_token() {
    for (input, message, reason, token, line, start, end, stderr) in [
        (
            "scenario S { property }",
            "scenario item",
            "unsupported_scenario_item",
            "property",
            2,
            16,
            24,
            "  at 2:16\n  kind: analysis_not_connected\n  cause: unsupported_token\n  blocked by: feature property at 2:16\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
        ),
        (
            "contract C { invariant always ok }",
            "state invariant expression",
            "unsupported_invariant_expression",
            "always",
            2,
            26,
            32,
            "  at 2:26\n  kind: analysis_not_connected\n  cause: unsupported_token\n  blocked by: feature always at 2:26\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
        ),
        (
            "actor A { standing over [Other] basis \"x\" }",
            "standing supports only end and commit targets",
            "unsupported_standing_target",
            "Other",
            2,
            28,
            33,
            "  at 2:28\n  kind: analysis_not_connected\n  cause: unsupported_token\n  blocked by: feature Other at 2:28\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
        ),
        (
            "actor A { state keys: set<PersonId> }",
            "set element type is unsupported",
            "unsupported_set_element_type",
            "PersonId",
            2,
            29,
            37,
            "  at 2:29\n  kind: analysis_not_connected\n  cause: unsupported_token\n  blocked by: feature PersonId at 2:29\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
        ),
    ] {
        let base = "model M version \"0.2\" {\n  process P { end done }\n  bounds { cases 1; live_tasks 1; clock 1min }\n  verify reachable end.done\n}\n";
        let source = base.replacen(
            "\n",
            &format!("\n  {input}\n  actor Z {{ state keys: set<PersonId> }}\n"),
            1,
        );
        let result = invoke("check", &source, &[]);
        assert_eq!(result.1["diagnostics"].as_array().unwrap().len(), 1);
        let loc = location(line, start, line, end);
        rejected(
            result,
            &format!("unsupported syntax: {message}"),
            reason,
            "unsupported_token",
            loc.clone(),
            vec![blocker("feature", Some(token), loc)],
            stderr,
        );
        accepts("check", base, &[], reason);
    }
}

#[test]
fn conformance_scope_has_no_model_location() {
    let source = include_str!("../snapshots/inputs/r2-recovery.relca");
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/inputs/r2-conform.jsonl");
    rejected(
        invoke(
            "conform",
            source,
            &["--log", log.to_str().unwrap(), "--scope", "other"],
        ),
        "scope",
        "scope",
        "log_scope",
        Value::Null,
        vec![],
        "  kind: analysis_not_connected\n  cause: log_scope\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
    assert!(
        !invoke(
            "conform",
            source,
            &[
                "--log",
                log.to_str().unwrap(),
                "--scope",
                "event-contracts",
                "--scenario",
                "Full"
            ]
        )
        .2
        .contains("cause: log_scope")
    );
}

#[test]
fn conformance_q1_uses_the_model_declaration() {
    let source = include_str!("../snapshots/inputs/r2-recovery.relca");
    let log = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/snapshots/inputs/r2-conform.jsonl");
    let result = invoke(
        "conform",
        source,
        &[
            "--log",
            log.to_str().unwrap(),
            "--scope",
            "event-contracts",
            "--scenario",
            "Full",
        ],
    );
    let reason = "unsupported_q1_model_feature";
    rejected(
        result,
        "q1:capability:Psp.charge_keyed:idempotency: unsupported_q1_model_feature",
        reason,
        "log_model_feature",
        location(9, 7, 9, 26),
        vec![blocker("feature", None, location(9, 7, 9, 26))],
        "  at 9:7\n  kind: analysis_not_connected\n  cause: log_model_feature\n  blocked by: feature at 9:7\n  see: docs/guide/dsl.md#解析が未接続のとき\n",
    );
}
