//! Hand-derived acceptance for ADR 0057 review rulings M1–M3 and m1–m4.
//! Production fault injections require separate execution evidence.
#[path = "support/log_conformance_inputs.rs"]
mod input;
#[path = "support/test_tmp.rs"]
mod test_tmp;
#[path = "support/log_conformance_wire.rs"]
mod wire;

use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// Check both entrypoints against hand expectations, not against each other.
/// Static validity is a prerequisite: parser/lower failures cannot masquerade as RED.
fn observe(c: &input::Case, check: impl Fn(&Value, i32)) {
    let ast = relca_core::syntax::parse(&c.model).expect("review model parses");
    relca_core::lower::lower(&ast).expect("review model lowers without executing it");
    let log = c.log();
    let tmp = test_tmp::root();
    let dir = tmp.join(format!(
        "q2-review-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let model_path = dir.join("model.relca");
    let log_path = dir.join("events.jsonl");
    std::fs::write(&model_path, &c.model).unwrap();
    std::fs::write(&log_path, &log).unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["conform"])
        .arg(&model_path)
        .arg("--log")
        .arg(&log_path)
        .args(["--scope", "event-contracts"])
        .output()
        .unwrap();
    assert_eq!(std::fs::read_to_string(&model_path).unwrap(), c.model);
    assert_eq!(std::fs::read_to_string(&log_path).unwrap(), log);
    assert_eq!(out.stdout.iter().filter(|b| **b == b'\n').count(), 1);
    assert!(out.stdout.ends_with(b"\n"));
    let v: Value = serde_json::from_slice(&out.stdout).expect("one full CLI report");
    let exit = out.status.code().expect("normal CLI exit");
    wire::validate(&v, exit).unwrap_or_else(|e| panic!("CLI wire: {e}"));
    let stderr = String::from_utf8(out.stderr).unwrap();
    for d in v["diagnostics"].as_array().unwrap() {
        assert!(
            stderr
                .lines()
                .any(|l| l.starts_with(&format!("{}:", d["code"].as_str().unwrap())))
        );
    }
    // A RED CLI check must not prevent observing the core boundary as well.
    // Keep both panics visible, and still fail the test if either failed.
    let cli_check = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&v, exit)));

    let core = relca_core::conformance::conform(&c.model, log.as_bytes(), None, None);
    let exit = core.verdict.exit_code();
    let v = serde_json::to_value(core).unwrap();
    wire::validate(&v, exit).unwrap_or_else(|e| panic!("core wire: {e}"));
    let core_check = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| check(&v, exit)));
    assert!(
        cli_check.is_ok() && core_check.is_ok(),
        "hand-value check: CLI={}, core={}",
        cli_check.is_ok(),
        core_check.is_ok()
    );
}

fn result(v: &Value, exit: i32, expected: i32) {
    let (tri, verdict) = match expected {
        0 => ("conformant", "satisfied"),
        1 => ("violating", "violated"),
        2 => ("unknown", "inconclusive"),
        3 => ("unknown", "invalid"),
        4 => ("unknown", "unsupported"),
        _ => unreachable!(),
    };
    assert_eq!(
        (&v["conformance"], &v["verdict"], exit),
        (&json!(tri), &json!(verdict), expected),
        "review hand classification: summary={}, limits={}",
        v["summary"],
        v["limits"]
    );
}

fn diagnostic(v: &Value, code: &str, reason: &str) {
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == code && d["reason"] == reason),
        "machine diagnostic {code}/{reason} absent: {}",
        v["diagnostics"]
    );
}

fn permission() -> input::Case {
    let mut c = input::case("l09_once_ok");
    c.model = c
        .model
        .replace("process Flow", "permit p to Poster.post\nprocess Flow");
    c
}

#[test]
fn m1_permit_inside_is_conformant() {
    observe(&permission(), |v, e| result(v, e, 0));
}

#[test]
fn m1_permit_outside_known_person_is_violating() {
    let mut c = permission();
    c.apply(&input::Edit::AssignTask {
        task: "t1",
        to: "person-q",
    });
    observe(&c, |v, e| {
        result(v, e, 1);
        let row = v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["reason"] == "permission" && r["conformance"] == "violating")
            .expect("positive permission witness");
        let ids: BTreeSet<_> = row["evidence"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| e["event_id"].as_str().unwrap())
            .collect();
        assert!(ids.contains("t1-start"));
        assert!(ids.contains("t1-finish"));
        assert!(row["model_span"].is_object());
        assert!(wire::has_code(v, "E-LOG-2"));
        assert!(
            v["unmapped_actor"].as_array().unwrap().is_empty(),
            "known person must not become unmapped"
        );
    });
}

#[test]
fn m1_permit_unknown_person_stays_unknown() {
    let mut c = permission();
    c.apply(&input::Edit::AssignTask {
        task: "t1",
        to: "unmapped-person",
    });
    observe(&c, |v, e| {
        result(v, e, 2);
        assert!(
            v["unmapped_actor"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["reason"] == "no_executor_mapping")
        );
        assert!(!wire::has_code(v, "E-LOG-2"));
    });
}

#[test]
fn m1_without_permit_executors_are_the_fallback() {
    let mut c = input::case("l09_once_ok");
    c.apply(&input::Edit::AssignTask {
        task: "t1",
        to: "person-q",
    });
    observe(&c, |v, e| result(v, e, 0));
}

#[test]
fn m1_multiple_permits_form_a_union() {
    let mut c = permission();
    c.model = c
        .model
        .replace("process Flow", "permit q to Poster.post\nprocess Flow");
    c.apply(&input::Edit::AssignTask {
        task: "t1",
        to: "person-q",
    });
    observe(&c, |v, e| result(v, e, 0));
}

fn feature_base() -> input::Case {
    let mut c = input::case("l09_once_ok");
    c.model = c.model
        .replace("enum { committed }", "enum { committed, denied }")
        .replace("people [p,q]", "resource Desk capacity 1\nobserved case { snapshot: Verdict = pass }\nlatent case { hidden: bool = false }\npeople [p,q]")
        .replace("actor Poster { executors [p,q];", "actor Poster { executors [p]; capacity 1; state marker: bool = false;")
        .replace("capability post() -> Ack {", "capability post() -> Ack { cancel forbidden_after_start;");
    c.model = c
        .model
        .replace("actor Clerk {", "actor Clerk { source document;");
    c
}

/// Each row adds one semantic feature, including syntactic dependencies required
/// by lower (e.g. version_check must name a written shared state).
fn feature(field: &str) -> input::Case {
    let mut c = feature_base();
    let prefix = "capability post() -> Ack {";
    let extra = match field {
        "permission" => "permit p to Poster.post\n",
        "uses" => "uses Desk: 1;",
        "reads" => "reads [Clerk];",
        "read_variables" => "reads [snapshot];",
        "requires" => "requires marker == false else denied;",
        "start_writes" => "start { marker = true };",
        "writes" => "writes marker; finish { marker = true };",
        "version_check" => {
            "version_check [marker] else denied; writes marker; finish { marker = true };"
        }
        _ => "",
    };
    match field {
        "params" => {
            c.model = c
                .model
                .replace("post() -> Ack", "post(key: CaseId) -> Ack")
                .replace("Poster.post()", "Poster.post(case.id)")
        }
        "permission" => {
            c.model = c
                .model
                .replace("process Flow", &format!("{extra}process Flow"))
        }
        "duration_distribution" => {
            c.model = c.model.replace(
                "commits; duration fixed 1min",
                "commits; duration {1min:0.5,2min:0.5}",
            )
        }
        "atomic" => {
            c.model = c.model.replace(
                "result nondet { committed }",
                "writes marker; atomic { if marker == true { result denied } else { marker = true; result committed } }",
            )
        }
        "kernel" | "kernel_reads_latent" | "kernel_writes" => {
            let declaration = match field {
                "kernel_reads_latent" => {
                    "kernel K given [hidden] { true -> { 1: committed }; false -> { 1: committed } }"
                }
                "kernel_writes" => "kernel K { * -> { 1: committed with hidden = true } }",
                _ => "kernel K { * -> { 1: committed } }",
            };
            let outcome = match field {
                "kernel_reads_latent" => "outcomes kernel K; kernel_reads latent [hidden]",
                "kernel_writes" => "outcomes kernel K; writes hidden",
                _ => "outcomes kernel K",
            };
            c.model = c
                .model
                .replace("actor Clerk", &format!("{declaration}\nactor Clerk"))
                .replace("result nondet { committed }", outcome);
        }
        _ => {
            assert!(!extra.is_empty(), "unlisted feature {field}");
            c.model = c.model.replace(prefix, &format!("{prefix} {extra}"));
        }
    }
    c
}

fn not_checked(v: &Value, field: &str) {
    assert!(
        v["not_checked"].as_array().unwrap().iter().any(|r| {
            let item = r["item"].as_str().unwrap();
            item.contains("Poster.post")
                && item.contains(field)
                && r["reason"] == "outside_scope"
                && r["model_span"].is_object()
        }),
        "missing field-specific not_checked for {field}: {}",
        v["not_checked"]
    );
}

fn unsupported(v: &Value) {
    assert!(wire::has_code(v, "U-LOG-1"));
    assert!(
        v["checks"].as_array().unwrap().iter().any(|r| {
            r["conformance"] == "unknown"
                && r["reason"]
                    .as_str()
                    .is_some_and(|s| s.starts_with("unsupported_capability"))
                && r["model_span"].is_object()
                && r["evidence"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|ev| ev["event_id"] == "t1-start")
        }),
        "unsupported Capability row/evidence was hidden: {}",
        v["checks"]
    );
    assert!(
        v["not_checked"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["reason"] == "unsupported_semantics")
    );
    assert!(v["summary"]["unsupported"].as_u64().unwrap() > 0);
}

#[test]
fn m2_fieldless_control_has_no_invented_feature_rows() {
    observe(&feature_base(), |v, e| {
        result(v, e, 0);
        for field in ["uses", "params", "reads", "read_variables"] {
            assert!(
                !v["not_checked"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|r| r["item"].as_str().unwrap().contains("Poster.post")
                        && r["item"].as_str().unwrap().contains(field))
            );
        }
    });
}

#[test]
fn green_review_wire_control_rejects_wrong_exit() {
    let control = wire::control();
    wire::validate(&control, 0).unwrap();
    assert!(wire::validate(&control, 2).is_err());
}

macro_rules! outside_scope_tests {
    ($($name:ident: $field:literal),+ $(,)?) => {$(
        #[test] fn $name() {
            observe(&feature($field), |v, e| { result(v, e, 0); not_checked(v, $field); assert!(!wire::has_code(v, "U-LOG-1")); });
        }
    )+};
}
outside_scope_tests!(
    m2_uses_are_explicitly_not_checked: "uses",
    m2_params_are_explicitly_not_checked: "params",
    m2_reads_are_explicitly_not_checked: "reads",
    m2_read_variables_are_explicitly_not_checked: "read_variables",
);

macro_rules! unsupported_tests {
    ($($name:ident: $field:literal),+ $(,)?) => {$(
        #[test] fn $name() {
            observe(&feature($field), |v, e| { result(v, e, 4); unsupported(v); });
        }
    )+};
}
unsupported_tests!(
    m2_requires_fail_closed: "requires",
    m2_atomic_fails_closed: "atomic",
    m2_version_check_fails_closed: "version_check",
    m2_start_writes_fail_closed: "start_writes",
    m2_finish_writes_fail_closed: "writes",
    m2_kernel_fails_closed: "kernel",
    m2_kernel_reads_latent_fail_closed: "kernel_reads_latent",
    m2_kernel_writes_fail_closed: "kernel_writes",
    m2_duration_distribution_fails_closed: "duration_distribution",
);

#[test]
fn m2_feature_inputs_really_populate_the_named_ir_fields() {
    // An exhaustive destructure protects the acceptance ledger too. Production
    // must independently destructure Capability in its own feature gate (M2).
    for field in [
        "permission",
        "params",
        "reads",
        "read_variables",
        "uses",
        "requires",
        "atomic",
        "version_check",
        "start_writes",
        "writes",
        "kernel",
        "kernel_reads_latent",
        "kernel_writes",
        "duration_distribution",
    ] {
        let c = feature(field);
        let m = relca_core::lower::lower(&relca_core::syntax::parse(&c.model).unwrap())
            .unwrap_or_else(|d| panic!("{field}: {d:?}"));
        let cap = m.capabilities.iter().find(|c| c.name == "post").unwrap();
        let relca_core::ir::Capability {
            qualified_executors,
            performance,
            version_check,
            read_variables,
            atomic,
            uses,
            requires,
            start_writes,
            kernel,
            kernel_reads_latent,
            kernel_writes,
            writes,
            permission,
            params,
            reads,
            commits,
            name,
            actor,
            return_type,
            duration,
            duration_distribution,
            results,
            cancel,
            span,
        } = cap;
        assert!(qualified_executors.is_none());
        assert!(performance.is_none());
        let present = match field {
            "permission" => permission.is_some(),
            "params" => !params.is_empty(),
            "reads" => !reads.is_empty(),
            "read_variables" => !read_variables.is_empty(),
            "uses" => !uses.is_empty(),
            "requires" => requires.is_some(),
            "atomic" => atomic.is_some(),
            "version_check" => version_check.is_some(),
            "start_writes" => !start_writes.is_empty(),
            "writes" => !writes.is_empty(),
            "kernel" => kernel.is_some(),
            "kernel_reads_latent" => !kernel_reads_latent.is_empty(),
            "kernel_writes" => !kernel_writes.is_empty(),
            "duration_distribution" => duration_distribution.is_some(),
            _ => unreachable!(),
        };
        assert!(present, "feature input failed to exercise {field}");
        assert!(*commits && name == "post" && m.actors[actor.0].name == "Poster");
        assert!(!m.types[return_type.0].members.is_empty());
        assert!(*duration == 1 || duration_distribution.is_some());
        assert!(!results.is_empty());
        assert!(matches!(
            cancel,
            relca_core::ir::Cancel::ForbiddenAfterStart
        ));
        assert!(span.start < span.end);
    }
}

/// A small full-read counterpart: a late second finish destroys the Task's
/// uniqueness, hence the earlier apparent duration violation is not certain.
fn duplicate_suffix() -> input::Case {
    let mut c = input::case("l23_duration_violation");
    let mut duplicate = c.records[2].clone();
    duplicate["event_id"] = json!("late-duplicate");
    c.records.as_array_mut().unwrap().push(duplicate);
    c
}

#[test]
fn m3_full_duplicate_finish_overturns_the_apparent_prefix_violation() {
    observe(&duplicate_suffix(), |v, e| {
        result(v, e, 2);
        assert!(wire::has_reason(v, "duplicate_phase"));
        assert_eq!(v["summary"]["checks"]["violating"], 0);
    });
}

#[test]
fn m3_untruncated_prefix_has_one_duration_violation() {
    observe(&input::case("l23_duration_violation"), |v, e| {
        result(v, e, 1);
        assert_eq!(v["summary"]["checks"]["violating"], 1);
    });
}

fn input_cut(v: &Value, e: i32, limit: &str) {
    result(v, e, 2);
    assert_eq!(v["limits"][limit], true);
    assert_eq!(v["limits"]["truncated"], true);
    assert_eq!(
        v["summary"]["checks"]["violating"], 1,
        "prefix observation must remain separately visible"
    );
    assert!(
        v["checks"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["conformance"] == "violating" && r["reason"] == "duration")
    );
    assert!(wire::has_code(v, "W-LOG-4"));
    assert!(v["log"]["sha256"].is_null() && v["log"]["lines"].is_null());
    assert!(!wire::evidence_ids(v).contains("late-duplicate"));
}

#[test]
fn m3_event_cut_with_prefix_violation_and_unread_duplicate_is_unknown() {
    let mut c = input::case("l23_event_boundary");
    c.records[2]["time"] = json!({"earliest":2,"latest":2});
    let mut late = c.records[2].clone();
    late["event_id"] = json!("late-duplicate");
    c.records.as_array_mut().unwrap().push(late);
    observe(&c, |v, e| {
        input_cut(v, e, "event_limit");
        assert_eq!(v["log"]["events"], 10000);
    });
}

#[test]
fn m3_line_cut_with_prefix_violation_and_unread_duplicate_is_unknown() {
    let mut c = duplicate_suffix();
    let mut oversized = input::event("blocker", "Poster.post", "start", 3, "person-p");
    oversized["input_version"] = json!("x".repeat(65537));
    c.records.as_array_mut().unwrap().insert(3, oversized);
    observe(&c, |v, e| {
        input_cut(v, e, "line_limit");
        assert_eq!(v["log"]["events"], 2);
    });
}

#[test]
fn m1_future_header_with_new_field_is_unsupported_not_invalid() {
    let mut c = input::case("l20_future_schema");
    c.records[0]["future_extension"] = json!({"version":2});
    observe(&c, |v, e| {
        result(v, e, 4);
        assert!(wire::has_code(v, "U-LOG-0"));
    });
}

#[test]
fn m1_future_header_does_not_require_v1_clock() {
    let mut c = input::case("l20_future_schema");
    c.records[0].as_object_mut().unwrap().remove("clock");
    observe(&c, |v, e| {
        result(v, e, 4);
        assert!(wire::has_code(v, "U-LOG-0"));
    });
}

#[test]
fn m1_v1_header_with_new_field_is_still_invalid() {
    let mut c = input::case("l09_once_ok");
    c.records[0]["future_extension"] = json!({"version":2});
    observe(&c, |v, e| {
        result(v, e, 3);
        assert!(wire::has_code(v, "E-LOG-0"));
    });
}

#[test]
fn m2_order_cycle_has_its_own_machine_reason() {
    let mut c = input::case("wire_source_cycle");
    // Remove the independent time contradiction so only source-cycle remains.
    c.records[2]["time"] = c.records[1]["time"].clone();
    observe(&c, |v, e| {
        result(v, e, 3);
        diagnostic(v, "E-LOG-1", "order_cycle");
    });
}

#[test]
fn m2_inconsistent_time_has_a_different_machine_reason() {
    observe(&input::case("wire_after_time_contradiction"), |v, e| {
        result(v, e, 3);
        diagnostic(v, "E-LOG-1", "inconsistent_time_evidence");
    });
}

#[test]
fn m2_unknown_after_has_a_machine_reason() {
    observe(&input::case("wire_after_unknown"), |v, e| {
        result(v, e, 3);
        diagnostic(v, "E-LOG-0", "unknown_after");
    });
}

#[test]
fn m3_unsupported_capability_survives_duplicate_phase_distrust() {
    let mut c = feature("requires");
    let mut late = c.records[2].clone();
    late["event_id"] = json!("late-duplicate");
    c.records.as_array_mut().unwrap().push(late);
    observe(&c, |v, e| {
        result(v, e, 4);
        unsupported(v);
        assert!(wire::has_reason(v, "duplicate_phase"));
    });
}

#[test]
fn m3_unsupported_capability_survives_unmapped_actor_distrust() {
    let mut c = feature("requires");
    c.apply(&input::Edit::ActorTask {
        task: "t1",
        to: "unmapped-actor",
    });
    observe(&c, |v, e| {
        result(v, e, 4);
        unsupported(v);
        assert!(!v["unmapped_actor"].as_array().unwrap().is_empty());
    });
}

fn padded_header(bytes: usize) -> input::Case {
    let mut c = input::case("l09_once_ok");
    let old_len = c.records[0].to_string().len();
    let old_clock = c.records[0]["clock"].as_str().unwrap().len();
    c.records[0]["clock"] = json!(format!("あ{}", "x".repeat(bytes - old_len + old_clock - 3)));
    assert_eq!(c.records[0].to_string().len(), bytes);
    c
}

#[test]
fn m3_header_exact_byte_boundary_is_accepted() {
    observe(&padded_header(65536), |v, e| {
        result(v, e, 0);
        assert_eq!(v["limits"]["truncated"], false);
    });
}

#[test]
fn m3_header_over_byte_boundary_is_invalid() {
    observe(&padded_header(65537), |v, e| {
        result(v, e, 3);
        diagnostic(v, "E-LOG-0", "header_line_limit");
        assert!(v["checks"].as_array().unwrap().is_empty());
    });
}

#[test]
fn m4_explicit_finish_before_start_after_is_invalid_even_at_same_tick() {
    let mut c = input::case("l09_once_ok");
    c.records[2]["time"] = c.records[1]["time"].clone();
    c.records[1]["after"] = json!(["t1-finish"]);
    observe(&c, |v, e| {
        result(v, e, 3);
        assert!(wire::has_code(v, "E-LOG-1"));
    });
}

#[test]
fn m4_time_only_lifecycle_reversal_is_violating_not_invalid() {
    observe(&input::case("wire_reversed_lifecycle"), |v, e| {
        result(v, e, 1);
        assert!(wire::has_reason(v, "lifecycle"));
        assert!(!wire::has_code(v, "E-LOG-1"));
    });
}

#[test]
fn m4_explicit_normal_lifecycle_after_is_conformant() {
    observe(&input::case("wire_explicit_task_edge"), |v, e| {
        result(v, e, 0)
    });
}
