//! ADR 0012 public acceptance. Oracles are the ADR/addendum and hand paths.
//! No proposed state AST/IR shape, private engine state or implementation output
//! is used to construct expectations. Internal obligations are handed off separately.
use relca_core::{diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{collections::BTreeMap, fs, path::Path};
#[path = "support/diff_increment.rs"]
mod diff_increment;

const POLICY: &str = include_str!("../../../fixtures/state-policy/model.relca");
const PRODUCT: &str = include_str!("../../../fixtures/state-latent-product/model.relca");
const INVOICE: &str = include_str!("../../../fixtures/state-invoice-write/model.relca");
const CANCEL: &str = include_str!("../../../fixtures/state-cancel/model.relca");
const ALIAS: &str = include_str!("../../../fixtures/state-name-alias/model.relca");
const ORDER: &str = include_str!("../../../fixtures/state-writer-order/model.relca");
const AGENCY: &str = include_str!("../../../fixtures/state-agency/model.relca");
const SAFETY: &str = "verify safety no_resource_overcommit";

fn check(source: &str) -> Value {
    serde_json::to_value(report::check(source)).unwrap()
}

fn has_code(value: &Value, code: &str) -> bool {
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

fn failure_value(source: &str, value: &Value, code: &str) {
    assert!(has_code(value, code), "missing {code}: {source}\n{value}");
    assert_eq!(
        value["verdict"],
        if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        },
        "{source}\n{value}"
    );
    assert_eq!(value["properties"], json!([]));
    assert_eq!(value["terminals"], json!([]));
    assert_eq!(value["agency"], Value::Null);
    for field in ["cases", "live_tasks", "clock", "truncated"] {
        assert_eq!(value["bounds"][field], Value::Null, "{field}: {value}");
    }
    for d in value["diagnostics"].as_array().unwrap() {
        if d["severity"] == "error" {
            assert_eq!(
                d["code"], code,
                "a second fault must not mask the intended fault: {source}\n{value}"
            );
        }
        if d["code"] == code {
            assert_eq!(d["severity"], "error");
            let start = d["span"]["start"].as_u64().unwrap() as usize;
            let end = d["span"]["end"].as_u64().unwrap() as usize;
            assert!(start <= end && end <= source.len());
            assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
        }
    }
}

fn failure(source: &str, code: &str) {
    failure_value(source, &check(source), code);
}

fn failure_at(source: &str, code: &str, fault: &str) {
    let value = check(source);
    failure_value(source, &value, code);
    let start = source.find(fault).expect("fault site exists");
    let end = start + fault.len();
    assert!(
        value["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == code
                && d["span"]["start"]
                    .as_u64()
                    .is_some_and(|s| (s as usize) < end)
                && d["span"]["end"]
                    .as_u64()
                    .is_some_and(|e| e as usize > start)
        }),
        "{code} must point to {fault}, not another unsupported feature: {value}"
    );
}

fn clauses(value: &Value, expected: &[&str]) {
    let actual: Vec<_> = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"].as_str().unwrap().starts_with("contract "))
        .map(|p| p["verdict"].as_str().unwrap())
        .collect();
    assert_eq!(actual, expected, "{value}");
}

fn rows(value: &Value, key: &str, id: &str) -> BTreeMap<String, Value> {
    let array = value[key].as_array().unwrap();
    let rows: BTreeMap<_, _> = array
        .iter()
        .map(|r| (r[id].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    assert_eq!(rows.len(), array.len(), "duplicate {key}");
    rows
}

fn hand_report(actual: &Value, expected: &Value) {
    assert_eq!(ledger_shape::original_field_count(actual), 13);
    for key in ["verdict", "mode", "unit", "bounds", "cut"] {
        assert_eq!(actual[key], expected[key], "{key}: {actual}");
    }
    for (key, id) in [("properties", "id"), ("terminals", "end")] {
        assert_eq!(
            rows(actual, key, id),
            rows(expected, key, id),
            "{key}: {actual}"
        );
    }
    if let Some(agency) = expected.get("agency") {
        #[path = "support/agency_viewpoints_legacy.rs"]
        mod agency_legacy;
        assert_eq!(
            &agency_legacy::agency(&actual["agency"]),
            agency,
            "hand Agency: {actual}"
        );
    }
    for d in expected["diagnostics"].as_array().unwrap() {
        assert!(has_code(actual, d["code"].as_str().unwrap()), "{actual}");
        if let Some(severity) = d.get("severity") {
            assert!(
                actual["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|a| a["code"] == d["code"] && &a["severity"] == severity)
            );
        }
    }
    for code in expected["absent_diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        assert!(!has_code(actual, code.as_str().unwrap()), "{actual}");
    }
}

fn fixture(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = fs::read_to_string(dir.join("model.relca")).unwrap();
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    hand_report(&check(&source), &expected);
}

macro_rules! fixtures {
    ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test() { fixture($name); })+};
}
fixtures! {
    f1_policy => "state-policy",
    f1_latent_difficulty => "state-policy.broken-latent-difficulty",
    f2_product => "state-latent-product",
    f2_partial_assertion => "state-latent-product.broken-partial-assertion",
    f3_invoice => "state-invoice-write",
    f3_failed_approval => "state-invoice-write.broken-failed-approval",
    f4_cancel => "state-cancel",
    f4_ignore_cancel => "state-cancel.broken-ignore-cancel",
    f5_alias => "state-name-alias",
    f5_missing_write => "state-name-alias.broken-missing-write",
    f6_serial => "state-writer-order",
    f6_parallel => "state-writer-order.broken-parallel-writers",
    f7_binding_agency => "state-agency",
    f7_state_agency => "state-agency.broken-state-guard",
    f8_observed_range => "state-observed-range",
    f8_fixed_route => "state-observed-range.broken-fixed-route",
}

fn model(items: &str, cap: &str, flow: &str, contracts: &str) -> String {
    format!(
        r#"model M version "0.2" {{
      type T = enum {{ pass, fail }}
      type Q = enum {{ pass, fail }}
      {items}
      actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ pass }}; {cap} }} }}
      actor B {{ capacity 1; capability other() -> T {{ duration fixed 1min; result nondet {{ fail }} }} }}
      process Flow {{ {flow} }}
      bounds {{ cases 1; live_tasks 2; clock 20min }}
      {SAFETY}
      contract Controls {{ {contracts} }}
    }}"#
    )
}

fn base() -> String {
    model(
        "observed case { approval: optional<T>; flag: bool = false }",
        "writes approval",
        "x = A.run(); end done",
        "",
    )
}

// Each macro expansion is its own test: the first missing feature must not hide
// a later diagnostic obligation. Every input changes one condition only.
macro_rules! static_matrix {
    ($($name:ident: $from:literal => $to:literal, $code:literal;)+) => {$(
        #[test] fn $name() {
            let source = base();
            assert_eq!(source.matches($from).count(), 1);
            failure(&source.replacen($from, $to, 1), $code);
        }
    )+};
}
static_matrix! {
    s_missing_named_type: "optional<T>" => "Missing", "E-NAME-0";
    s_missing_optional_type: "optional<T>" => "optional<Missing>", "E-NAME-0";
    s_writes_unknown: "writes approval" => "writes missing", "E-NAME-0";
    s_finish_unknown_target: "writes approval" => "writes approval; finish { missing = pass }", "E-NAME-0";
    s_finish_outside_writes: "writes approval" => "writes approval; finish { flag = true }", "E-NAME-0";
    s_condition_unknown: "end done" => "if missing == pass {} else {}; end done", "E-NAME-0";
    s_condition_unknown_actor: "end done" => "if Missing.local == pass {} else {}; end done", "E-NAME-0";
    s_condition_unknown_local: "end done" => "if A.missing == pass {} else {}; end done", "E-NAME-0";
    s_default_enum_mismatch: "approval: optional<T>" => "approval: T = unknown", "E-TYPE-0";
    s_default_bool_mismatch: "flag: bool = false" => "flag: bool = pass", "E-TYPE-0";
    s_default_nonoptional_none: "approval: optional<T>" => "approval: T = none", "E-TYPE-0";
    s_default_optional_mismatch: "approval: optional<T>" => "approval: optional<T> = unknown", "E-TYPE-0";
    s_comparison_enum_mismatch: "end done" => "if approval == unknown {} else {}; end done", "E-TYPE-0";
    s_comparison_bool_mismatch: "end done" => "if flag == pass {} else {}; end done", "E-TYPE-0";
    s_comparison_nonoptional_none: "end done" => "if flag == none {} else {}; end done", "E-TYPE-0";
    s_finish_constant_mismatch: "writes approval" => "writes approval; finish { approval = unknown }", "E-TYPE-0";
    s_finish_nonoptional_none: "writes approval" => "writes [approval,flag]; finish { flag = none }", "E-TYPE-0";
    s_result_base_mismatch: "optional<T>" => "optional<Q>", "E-TYPE-0";
    s_result_bool_not_builtin: "writes approval" => "writes flag", "E-TYPE-0";
    s_observed_missing_bool_default: "flag: bool = false" => "flag: bool", "U-SEM-0";
    s_observed_missing_enum_default: "flag: bool = false" => "flag: T", "U-SEM-0";
    s_duplicate_observed_block: "observed case" => "observed case {}; observed case", "U-SEM-0";
    s_duplicate_latent_block: "process Flow" => "latent case {}; latent case {}; process Flow", "U-SEM-0";
    s_duplicate_same_declaration: "flag: bool" => "flag, flag: bool", "U-SEM-0";
    s_duplicate_separate_declaration: "flag: bool = false" => "flag: bool = false; flag: bool = true", "U-SEM-0";
    s_duplicate_across_kinds: "process Flow" => "latent case { flag: bool }; process Flow", "U-SEM-0";
    s_duplicate_actor_local: "actor A {" => "actor A { latent local: T; latent local: T;", "U-SEM-0";
    s_latent_optional_implicit: "process Flow" => "latent case { truth: optional<T> }; process Flow", "U-SEM-0";
    s_latent_optional_explicit: "process Flow" => "latent case { truth: optional<T> = pass }; process Flow", "U-SEM-0";
    s_actor_latent_optional: "actor A {" => "actor A { latent local: optional<T>;", "U-SEM-0";
    s_duplicate_writes: "writes approval" => "writes approval; writes approval", "U-SEM-0";
    s_duplicate_writes_list: "writes approval" => "writes [approval,approval]", "U-SEM-0";
    s_duplicate_finish: "writes approval" => "writes approval; finish {}; finish {}", "U-SEM-0";
    s_duplicate_finish_target: "writes approval" => "writes approval; finish { approval = pass; approval = pass }", "U-SEM-0";
}

// A constant source is not a Result writer, even if its only value coincides.
#[test]
fn s_alias_constant_writer_is_not_result_even_when_all_results_equal() {
    let source = base()
        .replace(
            "writes approval",
            "writes approval; finish { approval = pass }",
        )
        .replace("x = A.run()", "approval = A.run()");
    failure(&source, "U-SEM-0");
}

#[test]
fn s_alias_missing_writer_rejected_without_condition_or_when_uncalled_suffix() {
    for flow in [
        "approval = A.run(); end done",
        "end done; approval = A.run()",
    ] {
        failure(
            &model("observed case { approval: optional<T> }", "", flow, ""),
            "U-SEM-0",
        );
    }
}

#[test]
fn s_local_bare_name_is_not_case_state_and_invariant_names_checked_when_uncalled() {
    let source = model(
        "",
        "",
        "end done",
        "invariant A.run.finish -> local == pass",
    )
    .replace("actor A {", "actor A { latent local: T = pass;");
    failure(&source, "E-NAME-0");
    for cond in [
        "missing == pass",
        "Missing.local == pass",
        "A.missing == pass",
    ] {
        failure(
            &model(
                "",
                "",
                "end done",
                &format!("invariant A.run.finish -> {cond}"),
            ),
            "E-NAME-0",
        );
    }
}

macro_rules! outside_matrix {
    ($($name:ident: $items:literal, $cap:literal, $fault:literal;)+) => {$(
        #[test] fn $name() {
            failure_at(&model($items, $cap, "end done", ""), "U-SYN-0", $fault);
        }
    )+};
}
// 条件付き確率で受ける 2 つの形: 未宣言の Actor 局所の潜在(E-NAME-0)と、result nondet との併記(E-PROB-0)。
#[test]
fn s_out_kernel_reads() {
    failure_at(
        &model("", "kernel_reads latent [A.local]", "end done", ""),
        "E-NAME-0",
        "A.local",
    );
}
// ADR 0035 で移した形: atomic は Capability の項目として受理する構文になった(ADR 0035 の
// 「入れる」の atomic の行)。`atomic {}` は必須の `if … else …` を欠くので、ADR 0035 の構文の
// 「構造の欠けは E-SYN-0」に当たる。未対応の atomic の形(入れ子・3 枝以上・else if 等)の
// U-SYN-0 と受理の正例は fixtures/aset-static-* と fixtures/aset-dup が持つ。
#[test]
fn s_out_atomic() {
    failure_at(
        &model("", "atomic {}", "end done", ""),
        "E-SYN-0",
        "atomic {}",
    );
}
// ADR 0035 で移した形の正例(旧い入力をそのまま残す): case の観測の optional<PersonId> は
// 受理する(ADR 0035 U11)。値域は people の人(ADR 0035 の値域の行。ここでは無い)と
// optional の none なので、none だけの値域になり、終端だけのモデルは satisfied。
// ADR 0035 で移した形: finish の代入の源の executor.id は受理する構文になった。A は
// executors を持たない匿名の Actor なので、executor.id は U-SEM-0(ADR 0035:69。匿名の
// 実行者に ID を作らない)。単一の故障にするため、書込み先は観測の optional<PersonId> にし、
// people を宣言して空の値域を混ぜない。PersonId でない書込み先の E-TYPE-0 は
// requires_start_cancel_wait.rs の start_executor_expression が持つ。
#[test]
fn s_out_executor_finish() {
    failure_at(
        &model(
            "people [p]; observed case { who: optional<PersonId> }",
            "writes who; finish { who = executor.id }",
            "end done",
            "",
        ),
        "U-SEM-0",
        "executor.id",
    );
}
#[test]
fn s_out_personid_observed_is_accepted() {
    let source = model(
        "observed case { person: optional<PersonId> }",
        "",
        "end done",
        "",
    );
    assert_eq!(check(&source)["verdict"], "satisfied", "{source}");
}
#[test]
fn s_out_outcomes_kernel() {
    let declared = model(
        "kernel K { * -> { 1: pass } }",
        "outcomes kernel K",
        "end done",
        "",
    );
    failure_at(&declared, "E-PROB-0", "outcomes kernel K");
}
outside_matrix! {
    s_out_distribution: "", "duration discrete { 1min: 1.0 }", "discrete";
    s_out_requires: "", "requires flag == true", "requires";
    // ADR 0035 で観測の optional<PersonId> は受理になった(下の s_out_personid_observed_is_accepted)。
    // 未対応のまま残る PersonId の潜在へ移す(ADR 0035 U11「潜在は U-SYN-0」)。
    // people を宣言して、空の値域という別の故障を混ぜない。
    s_out_personid: "people [p]; latent case { person: PersonId }", "", "PersonId";
    // ADR0038 accepts named records; keep this rejection on inline records.
    s_out_record: "observed case { value: record { x: T } }", "", "record";
    s_out_decides: "", "decides flag: bool = true", "decides";
    s_out_input: "input x: T", "", "input";
    s_out_latent_writes: "latent case { truth: T }", "writes truth", "writes truth";
    s_out_latent_finish: "latent case { truth: T }", "finish { truth = pass }", "finish { truth";
    s_out_nested_optional: "observed case { x: optional<optional<T>> }", "", "optional<optional<T>>";
    s_out_unknown_generic: "observed case { x: box<T> }", "", "box<T>";
    s_out_set: "observed case { x: set<T> }", "", "set<T>";
    s_out_expression_finish: "observed case { x: T = pass }", "writes x; finish { x = pass + fail }", "finish { x = pass + fail }";
    s_out_model_latent_without_case: "latent truth: T", "", "latent truth";
    s_out_cap_latent: "", "latent truth: T", "latent truth";
    s_out_qualified_writes: "", "writes A.local", "writes A.local";
}

#[test]
fn s_outside_actor_state_and_observed_and_local_writes() {
    for addition in ["state x: set<T>", "observed case { x: T = pass }"] {
        failure_at(
            &base().replace("actor A {", &format!("actor A {{ {addition};")),
            "U-SYN-0",
            addition,
        );
    }
    for cap in ["writes local", "finish { local = pass }"] {
        let source =
            model("", cap, "end done", "").replace("actor A {", "actor A { latent local: T;");
        failure_at(&source, "U-SYN-0", cap);
    }
    failure_at(
        &model(
            "latent case { truth: T }",
            "",
            "if A.local.field == pass {} else {}; end done",
            "",
        ),
        "U-SYN-0",
        "A.local.field",
    );
    failure_at(
        &model(
            "observed case { flag: bool = false }",
            "",
            "end done",
            "invariant always flag -> flag == true",
        ),
        "U-SYN-0",
        "invariant always",
    );
}

macro_rules! syntax_matrix {
    ($($name:ident: $from:literal => $to:literal;)+) => {$(
        #[test] fn $name() { failure(&base().replacen($from, $to, 1), "E-SYN-0"); }
    )+};
}
syntax_matrix! {
    s_syn_missing_case: "observed case" => "observed";
    s_syn_missing_block: "observed case {" => "observed case";
    s_syn_missing_name: "approval: optional<T>" => ": optional<T>";
    s_syn_missing_colon: "approval: optional<T>" => "approval optional<T>";
    s_syn_missing_type: "approval: optional<T>" => "approval:";
    s_syn_missing_default: "flag: bool = false" => "flag: bool =";
    s_syn_empty_optional: "optional<T>" => "optional<>";
    s_syn_unclosed_optional: "optional<T>" => "optional<T";
    s_syn_empty_writes: "writes approval" => "writes []";
    s_syn_unclosed_writes: "writes approval" => "writes [approval";
    s_syn_missing_assignment_equals: "writes approval" => "writes approval; finish { approval pass }";
    s_syn_missing_assignment_value: "writes approval" => "writes approval; finish { approval = }";
    s_syn_missing_assignment_separator: "writes approval" => "writes approval; finish { approval = pass approval = pass }";
    s_syn_missing_var_separator: "flag: bool = false" => "flag: bool = false other: bool = true";
    s_syn_missing_condition_equals: "end done" => "if flag true {} else {}; end done";
    s_syn_unclosed_condition: "end done" => "if (flag == true {} else {}; end done";
}

#[test]
fn s_bool_and_optional_positions_follow_addendum_not_plan_recommendation() {
    // A named legacy enum bool remains a named type; builtins are var_decl only.
    let legacy = model(
        "type bool = enum { yes,no }",
        "",
        "x = A.run(); end done",
        "",
    )
    .replace("run() -> T", "run() -> bool")
    .replace("result nondet { pass }", "result nondet { yes }");
    assert_eq!(check(&legacy)["verdict"], "satisfied");
    failure(&base().replace("run() -> T", "run() -> bool"), "E-NAME-0");
    failure(
        &base()
            .replace("run() -> T", "run(p: bool) -> T")
            .replace("x = A.run();", ""),
        "E-NAME-0",
    );
    for signature in ["run() -> optional<T>", "run(p: optional<T>) -> T"] {
        failure(
            &base()
                .replace("run() -> T", signature)
                .replace("x = A.run();", ""),
            "U-SYN-0",
        );
    }
    failure(
        &base().replace(
            "type Q = enum { pass, fail }",
            "type bool = enum { yes,no }",
        ),
        "U-SEM-0",
    );
    // An old user enum named record is not record syntax.
    assert_eq!(
        check(&legacy.replace("bool", "record"))["verdict"],
        "satisfied"
    );
    // ADR 0035 で移した形: PersonId は組込みの型名になり、`type PersonId = …` の宣言は
    // 未使用でも U-SEM-0(ADR 0035 の構文の「CaseId と PersonId は組込みの型名」の行と U13。
    // 上の bool の衝突と同じ扱い)。単一の故障の入力で分類を固定し、旧い入力(宣言を
    // 結果の型に使う)は unsupported と U-SEM-0 だけを確かめる。
    failure(
        &base().replace(
            "type Q = enum { pass, fail }",
            "type PersonId = enum { yes,no }",
        ),
        "U-SEM-0",
    );
    let former = check(&legacy.replace("bool", "PersonId"));
    assert_eq!(former["verdict"], "unsupported", "{former}");
    assert!(has_code(&former, "U-SEM-0"), "{former}");
}

#[test]
fn s_contextual_words_forward_declarations_empty_blocks_and_namespace_controls() {
    for word in [
        "observed", "case", "latent", "writes", "finish", "bool", "optional", "true", "false",
    ] {
        let legacy = model("", "", "x = A.run(); end done", "");
        for source in [
            legacy
                .replace("actor A", &format!("actor {word}"))
                .replace("A.run", &format!("{word}.run")),
            legacy.replace("x =", &format!("{word} =")),
            legacy.replace("pass", word),
        ] {
            assert_eq!(
                check(&source)["verdict"],
                "satisfied",
                "context {word}: {source}"
            );
        }
    }
    for source in [
        base(),
        base().replace(';', "\n"),
        base()
            .replace(
                "observed case { approval: optional<T>; flag: bool = false }",
                "",
            )
            .replace(
                "process Flow",
                "observed case { approval: optional<T>; flag: bool = false }; process Flow",
            ),
        model(
            "observed case {}; latent case {}",
            "finish {}",
            "end done",
            "",
        ),
        model(
            "observed case { a,b: T = pass }; latent case { truth: T = fail }",
            "",
            "end done",
            "",
        ),
        model(
            "observed case { A: T = pass; done: T = fail }",
            "",
            "end done",
            "",
        )
        .replace("actor A {", "actor A { latent A: T; latent other: T;")
        .replace("actor B {", "actor B { latent A: T;"),
        base().replace(
            "writes approval",
            "writes [approval,flag]; finish { flag = true }",
        ),
    ] {
        assert_eq!(check(&source)["verdict"], "satisfied", "{source}");
    }
    for name in ["none", "by"] {
        failure(
            &model("", "", "x = A.run(); end done", "").replace("x =", &format!("{name} =")),
            "E-SYN-0",
        );
    }
}

#[test]
fn s_policy_global_dead_suffix_shortcircuit_fixed_and_binding_shadow() {
    let forbidden = "if Invoice.difficulty == difficult {} else {}";
    for flow in [
        format!("if difficult_seen == true {{ {forbidden} }} else {{}}; end done"),
        format!("end done; {forbidden}"),
        "if difficult_seen == false || Invoice.difficulty == difficult {} else {}; end done"
            .to_owned(),
        "if difficult_seen == true && Invoice.difficulty == difficult {} else {}; end done"
            .to_owned(),
    ] {
        let (prefix, suffix) = POLICY.split_once("  process Flow {").unwrap();
        let (_, suffix) = suffix.split_once("  bounds").unwrap();
        failure(
            &format!("{prefix} process Flow {{ {flow} }} bounds{suffix}"),
            "E-POL-LATENT",
        );
    }
    for declaration in [
        "latent case { truth: T }",
        "latent case { truth: T = pass }",
    ] {
        failure(
            &model(
                declaration,
                "",
                "truth = A.run(); if truth == pass {} else {}; end done",
                "",
            ),
            "E-POL-LATENT",
        );
    }
}

fn configured_other(other: &str) -> String {
    model("latent case { truth: T = pass }", "", "x = A.run(); end done", "")
        .replace("bounds {", &format!("process Other {{ {other} }}; cut C {{ include [A,B] }}; configuration Chosen {{ cut C; process Flow }}; configuration Unchosen {{ cut C; process Other }}; bounds {{"))
}

#[test]
fn s_policy_nonselected_scanned_but_ordinary_unknown_binding_not_newly_checked() {
    for condition in ["truth == pass", "A.local == pass"] {
        let source = configured_other(&format!("if {condition} {{}} else {{}}; end done"))
            .replace("actor A {", "actor A { latent local: T = pass;");
        let value = serde_json::to_value(report::check_configuration(&source, "Chosen")).unwrap();
        failure_value(&source, &value, "E-POL-LATENT");
    }
    let source = configured_other("if ordinary_unknown == pass {} else {}; end done");
    assert_eq!(
        serde_json::to_value(report::check_configuration(&source, "Chosen")).unwrap()["verdict"],
        "satisfied"
    );
}

fn metrics(value: &Value, min: u64, max: u64, wmin: u64, wmax: u64, result_at: Option<u64>) {
    assert_eq!(
        value["terminals"],
        json!([{"end":"done","elapsed":{"min":min,"max":max},"work":{"min":wmin,"max":wmax},"result_at":result_at.map(|r|json!({"min":r,"max":r}))}]),
        "{value}"
    );
    assert_eq!(value["bounds"]["truncated"], false);
}

#[test]
fn s_initial_values_optional_none_bool_enum_fixed_latent_and_unused() {
    let declaration = "observed case { a: optional<T>; b: optional<bool>; c: optional<T> = none; d: optional<bool> = none; yes: bool = true; no: bool = false; e: T = fail }; latent case { truth: bool = true }";
    let cond = "a == none && b == none && c == none && d == none && yes == true && no == false && e == fail && truth == true";
    let source = model(
        declaration,
        "",
        "x = A.run(); end done",
        &format!("invariant A.run.start -> {cond}; invariant A.run.finish -> {cond}"),
    );
    let value = check(&source);
    clauses(&value, &["satisfied", "satisfied"]);
    metrics(&value, 1, 1, 1, 1, None);
    assert_eq!(value["verdict"], "satisfied");
    assert!(
        !value["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"].as_str().unwrap().starts_with("latent."))
    );
}

#[test]
fn s_optional_bool_and_none_constants_and_distinct_result_type() {
    // T and Q have identical labels but distinct type identities. Constant
    // writes may cross result types; Result writes may not (matrix above).
    let source = model(
        "observed case { a: optional<bool>; b: optional<Q> = fail; c: optional<T> = pass }",
        "writes [a,b,c]; finish { a = true; b = pass; c = none }",
        "x = A.run(); end done",
        "invariant A.run.start -> a == none && b == fail && c == pass; invariant A.run.finish -> a == true && b == pass && c == none",
    );
    let value = check(&source);
    assert_eq!(value["verdict"], "satisfied");
    clauses(&value, &["satisfied", "satisfied"]);
    metrics(&value, 1, 1, 1, 1, None);
    let fixed = model(
        "latent case { truth: bool = true }; observed case { flag: bool = false }",
        "",
        "x = A.run(); end done",
        "",
    );
    let no_state = model("", "", "x = A.run(); end done", "");
    assert_eq!(
        check(&fixed)["assumptions"],
        check(&no_state)["assumptions"],
        "fixed/default observations are not new nondet assumptions"
    );
}

#[test]
fn s_latent_default_and_comparison_types_checked_even_unused_and_forward() {
    for (items, clause) in [
        ("latent case { truth: T = unknown }", ""),
        ("latent case { truth: bool = none }", ""),
        (
            "latent case { truth: bool = false }",
            "invariant A.run.finish -> truth == pass",
        ),
    ] {
        failure(&model(items, "", "end done", clause), "E-TYPE-0");
    }
    failure(
        &model(
            "",
            "",
            "end done",
            "invariant A.run.finish -> A.local == unknown",
        )
        .replace("actor A {", "actor A { latent local: T = pass;"),
        "E-TYPE-0",
    );
    let forward = model("", "writes approval", "x = A.run(); end done", "").replace(
        "bounds {",
        "observed case { approval: optional<T> }; bounds {",
    );
    assert_eq!(check(&forward)["verdict"], "satisfied");
}

#[test]
fn s_latent_assumptions_exact_fields_values_order_and_no_distribution() {
    let value = check(PRODUCT);
    let assumptions = value["assumptions"].as_array().unwrap();
    let nondet: Vec<_> = assumptions
        .iter()
        .filter(|a| ["latent.truth", "Invoice.difficulty"].contains(&a["field"].as_str().unwrap()))
        .collect();
    assert_eq!(nondet.len(), 2);
    assert_eq!(nondet[0]["field"], "latent.truth");
    assert_eq!(nondet[1]["field"], "Invoice.difficulty");
    for row in nondet {
        assert_eq!(row.as_object().unwrap().len(), 3);
        assert_eq!(row["value"], "nondet(全値)");
        let affects = row["affects"].as_str().unwrap();
        // Exact prose is not specified. Require both nondeterminism and a
        // distribution disclaimer, allowing Japanese or English reporting.
        let lower = affects.to_lowercase();
        assert!(
            affects.contains("非決定") || lower.contains("nondet"),
            "{affects}"
        );
        assert!(
            affects.contains("分布") || lower.contains("distribution"),
            "{affects}"
        );
        assert!(
            affects.contains("ない") || lower.contains("not") || lower.contains("no "),
            "{affects}"
        );
    }
    // An unused nondet variable is not silently cut from assumptions.
    let unused = check(&PRODUCT.replace(
        "latent case { truth: bool }",
        "latent case { truth: bool; unused: T }",
    ));
    assert_eq!(
        unused["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|a| a["field"] == "latent.unused" && a["value"] == "nondet(全値)")
            .count(),
        1
    );
}

macro_rules! exclude_tuple {
    ($name:ident, $opposite_truth:literal, $opposite_difficulty:literal) => {
        #[test] fn $name() {
            let source = PRODUCT.replace(
                "(truth == true || truth == false) && (Invoice.difficulty == routine || Invoice.difficulty == difficult)",
                concat!("truth == ", $opposite_truth, " || Invoice.difficulty == ", $opposite_difficulty));
            let value = check(&source);
            clauses(&value, &["violated"]);
            assert_eq!(value["verdict"], "violated");
            metrics(&value, 1, 1, 1, 1, None);
        }
    };
}
exclude_tuple!(s_product_exclude_true_routine, "false", "difficult");
exclude_tuple!(s_product_exclude_true_difficult, "false", "routine");
exclude_tuple!(s_product_exclude_false_routine, "true", "difficult");
exclude_tuple!(s_product_exclude_false_difficult, "true", "routine");

#[test]
fn s_alias_current_before_call_and_after_another_writer() {
    let source = ALIAS.replace(
        "approval = A.approve()",
        "if approval == none {} else { end rejected }; approval = A.approve()",
    );
    let value = check(&source);
    assert_eq!(value["verdict"], "satisfied");
    metrics(&value, 1, 1, 1, 1, None);
    let source = source.replace("process Flow", "actor B { capacity 1; capability overwrite() -> Verdict { duration fixed 1min; result nondet { fail }; writes approval } }; process Flow")
        .replace("if approval == pass", "later = B.overwrite(); if approval == fail");
    let value = check(&source);
    clauses(&value, &["satisfied"]); // A's historical pass must not be re-read as fail.
    assert_eq!(value["verdict"], "satisfied");
    metrics(&value, 2, 2, 2, 2, None);
    // Arguments still read bindings; state existence cannot supply a future binding.
    failure(
        &source
            .replace(
                "capability overwrite()",
                "capability overwrite(input_value: Verdict)",
            )
            .replace(
                "approval = A.approve()",
                "early = B.overwrite(approval); approval = A.approve()",
            )
            .replace("B.overwrite()", "B.overwrite(approval)"),
        "U-SEM-0",
    );
}

#[test]
fn s_bundle_state_start_prewrite_finish_and_commit_postwrite() {
    let source = INVOICE.replace("contract Controls {", "contract Controls { invariant Approver.approve.start -> approval == none; invariant Ledger.post.start -> registered == false; invariant Ledger.post.commit -> registered == true;");
    let value = check(&source);
    clauses(&value, &["satisfied"; 5]);
    metrics(&value, 2, 2, 2, 2, None);
    let value = check(
        &source
            .replace("commits;", "")
            .replace("invariant Ledger.post.commit -> registered == true;", ""),
    );
    clauses(&value, &["satisfied"; 4]);
}

#[test]
fn s_snapshot_preserves_each_event_value_and_reverse_control() {
    let source = ORDER.replace("p = Probe.check(); ", "").replace(
        "invariant Probe.check.finish -> flag == false",
        "invariant A.set.finish -> flag == true; invariant B.clear.finish -> flag == false",
    );
    for source in [
        source.clone(),
        source.replace("a = A.set(); b = B.clear()", "b = B.clear(); a = A.set()"),
    ] {
        let value = check(&source);
        clauses(&value, &["satisfied", "satisfied"]);
        metrics(&value, 2, 2, 2, 2, None);
    }
}

#[test]
fn s_same_tick_readwrite_all_orders_and_false_true_controls() {
    let source = ORDER
        .replace("b = B.clear(); p = Probe.check()", "p = Probe.check()")
        .replace(
            "a = A.set(); p = Probe.check()",
            "fork f { a = A.set(); p = Probe.check() }; join all f",
        )
        .replace("flag == false", "flag == true");
    for source in [
        source.clone(),
        source.replace("flag == true", "flag == false"),
    ] {
        let value = check(&source);
        clauses(&value, &["violated"]);
        assert_eq!(value["verdict"], "violated");
        metrics(&value, 1, 1, 2, 2, None);
    }
}

#[test]
fn s_cancel_normal_ack_tie_and_nondefault_preservation() {
    let tied = check(&CANCEL.replace("fixed 5min", "fixed 2min"));
    clauses(&tied, &["violated"]); // normal2 writes true, ack2 preserves false.
    metrics(&tied, 3, 3, 4, 4, Some(1));
    let source = CANCEL
        .replace("changed: bool = false", "changed: bool = true")
        .replace("changed = true }", "changed = false }")
        .replace("changed == false", "changed == true");
    let value = check(&source);
    clauses(&value, &["satisfied"]);
    metrics(&value, 3, 3, 4, 4, Some(1));
    // Queued cancellation and the instant of cancellation request require
    // internal observations; aggregate verdicts cannot identify those moments.
}

#[test]
fn s_writer_conflict_is_structural_not_resource_or_duration_dependent() {
    let parallel = ORDER.replace(
        "a = A.set(); b = B.clear()",
        "fork f { a = A.set(); b = B.clear() }; join all f",
    );
    for source in [
        parallel.clone(),
        parallel.replace("fixed 1min", "fixed 0min"),
        parallel.replace("b = B.clear()", "b = A.set()"),
        parallel
            .replace("type T =", "people [p]; type T =")
            .replace("capacity 1", "executors [p]"),
        parallel
            .replace("fork f", "if flag == true { fork f")
            .replace("join all f;", "join all f } else {};"),
    ] {
        failure(&source, "U-SEM-0");
    }
    // Distinct writes, alternative branches and calls after drain remain legal.
    let distinct = parallel
        .replace(
            "flag: bool = false",
            "flag: bool = false; other: bool = true",
        )
        .replace(
            "writes flag; finish { flag = false }",
            "writes other; finish { other = false }",
        )
        .replace("flag == false", "other == false");
    assert_eq!(check(&distinct)["verdict"], "satisfied");
    let alternatives = ORDER
        .replace(
            "a = A.set(); b = B.clear()",
            "if flag == false { a = A.set() } else { b = B.clear() }",
        )
        .replace("flag == false }", "flag == true }");
    assert_eq!(check(&alternatives)["verdict"], "satisfied");
    let after_join = ORDER.replace("a = A.set()", "fork f { a = A.set() }; join all f");
    assert_eq!(check(&after_join)["verdict"], "satisfied");
}

#[test]
fn s_cut_retains_known_state_violation_never_promotes_fulfilled() {
    for (result, expected) in [("pass", "inconclusive"), ("fail", "violated")] {
        let source = INVOICE
            .replace(
                "result nondet { pass }",
                &format!("result nondet {{ {result} }}"),
            )
            .replace(
                "capability post() -> Ack { commits; duration fixed 1min",
                "capability post() -> Ack { commits; duration fixed 5min",
            )
            .replace("clock 20min", "clock 3min");
        let value = check(&source);
        clauses(&value, &[expected, "inconclusive"]);
        assert_eq!(value["verdict"], expected);
        assert_eq!(value["bounds"]["truncated"], true);
        assert_eq!(value["terminals"], json!([]));
    }
}

#[test]
fn s_abort_same_bundle_state_violation_survives_each_clause_order() {
    let response = "response A.run.finish -> one_of(end.done) by 18446744073709551615min after A.run.finish otherwise unresolved";
    let invariant = "invariant A.run.finish -> flag == false";
    for (text, expected) in [
        (
            format!("{response}; {invariant}"),
            ["unsupported", "violated"],
        ),
        (
            format!("{invariant}; {response}"),
            ["violated", "unsupported"],
        ),
    ] {
        let value = check(&model(
            "observed case { flag: bool = false }",
            "writes flag; finish { flag = true }",
            "x = A.run(); end done",
            &text,
        ));
        clauses(&value, &expected);
        assert_eq!(value["verdict"], "violated");
        assert_eq!(value["bounds"]["truncated"], Value::Null);
        assert_eq!(value["terminals"], json!([]));
        assert!(has_code(&value, "U-SEM-0"));
    }
}

#[test]
fn s_agency_scope_keeps_unaffected_binding_effect_and_non_effect_state_guards() {
    let source = AGENCY.replace(
        "end done } else { end rejected }",
        "} else {}; if decision == pass { second = Ledger.post() } else {}; end done",
    );
    let value = check(&source);
    assert_eq!(value["verdict"], "satisfied");
    let agency = &value["agency"];
    assert_eq!(agency["rules"], "agency-p1-state-1");
    let refusal = agency["refusal"].as_array().unwrap();
    assert_eq!(refusal.len(), 1);
    assert_eq!(refusal[0]["call"], "p");
    assert_eq!(
        refusal[0]["judgements"],
        json!([{"call":"j","capability":"Judge.check","stops":"all_paths","bypasses":0}])
    );
    assert_eq!(agency["lineage"].as_array().unwrap().len(), 1);
    assert_eq!(agency["lineage_groups"][0]["judgements"], 1);
    assert!(!has_code(&value, "W-AGY-0") && !has_code(&value, "W-AGY-1"));
    let old = check(AGENCY);
    for source in [
        AGENCY.replace("end done }", "end done; if decision == pass {} else {} }"),
        AGENCY.replace(
            "if j == pass",
            "if decision == none {} else {}; if j == pass",
        ),
    ] {
        let value = check(&source);
        assert_eq!(value["agency"]["rules"], "agency-p1-state-1");
        for key in ["origin", "lineage", "lineage_groups", "refusal"] {
            assert_eq!(value["agency"][key], old["agency"][key], "{key}: {value}");
        }
        assert_eq!(
            value["agency"]["unchecked"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|v| **v == "state_guards")
                .count(),
            1
        );
    }
}

#[test]
fn s_agency_state_guard_does_not_erase_unaffected_conditional_or_none_refusals() {
    for (source, stops, warning) in [
        (
            AGENCY.replace("j == pass", "j == pass || j == fail"),
            "none",
            "W-AGY-0",
        ),
        (
            AGENCY.replace(
                "if j == pass",
                "k = Judge.check(e); if j == pass || k == pass",
            ),
            "conditional",
            "W-AGY-1",
        ),
    ] {
        let old = check(&source);
        let with_state = check(&source.replace(
            "if j == pass",
            "if decision == none {} else {}; if j == pass",
        ));
        assert_eq!(with_state["agency"]["rules"], "agency-p1-state-1");
        for key in ["origin", "lineage", "lineage_groups", "refusal"] {
            assert_eq!(
                with_state["agency"][key], old["agency"][key],
                "{key}: {with_state}"
            );
        }
        let judgements = with_state["agency"]["refusal"][0]["judgements"]
            .as_array()
            .unwrap();
        assert!(!judgements.is_empty());
        assert!(judgements.iter().all(|j| j["stops"] == stops));
        assert!(has_code(&with_state, warning));
    }
}

#[test]
fn s_state_guard_agency_is_projected_by_diff_without_claiming_resource_or_authority() {
    let source = AGENCY.replace("bounds {", "process StateFlow { e2 = Source.read(); j2 = Judge.check(e2); if decision == pass { p2 = Ledger.post(); end done } else { end rejected } }; cut C { include [Invoice,PurchaseOrder,Source,Judge,Ledger] }; configuration From { cut C; process Flow }; configuration To { cut C; process StateFlow }; bounds {");
    let value = serde_json::to_value(diff::diff(&source, "From", "To")).unwrap();
    assert_eq!(value["verdict"], "satisfied");
    for (side, selection, rules) in [
        ("from", "From", "agency-p0-1"),
        ("to", "To", "agency-p1-state-1"),
    ] {
        let report = serde_json::to_value(report::check_configuration(&source, selection)).unwrap();
        assert_eq!(value["agency"][side], report["agency"]);
        assert_eq!(value[side]["report"]["agency"], report["agency"]);
        assert_eq!(report["agency"]["rules"], rules);
    }
    // ADR 0031: diff now computes resource and authority (and the other
    // REMOVE_UNCHECKED words). The pre-ADR lists at 85c5485 are projected, not
    // dropped: the old unchecked minus those words, the old checked plus them.
    const OLD_UNCHECKED: [&str; 9] = [
        "time_intent",
        "resource",
        "meaning",
        "authority",
        "visibility",
        "contest",
        "preserve",
        "override",
        "maintain",
    ];
    const OLD_CHECKED: [&str; 5] = [
        "comparison_mismatch",
        "actor",
        "relation",
        "intent_claims",
        "retire_loses",
    ];
    for word in ["resource", "authority"] {
        assert!(OLD_UNCHECKED.contains(&word));
    }
    assert_eq!(
        value["unchecked"],
        diff_increment::migrated_unchecked(&OLD_UNCHECKED)
    );
    assert_eq!(
        value["checked"],
        diff_increment::migrated_checked(&OLD_CHECKED)
    );
}

#[test]
fn s_fixture_execution_skeleton_controls_are_not_state_acceptance() {
    // Strip the new layer to exercise authored legacy syntax/arithmetic now.
    // This is deliberately not evidence for state initialization or writes.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut count = 0;
    for entry in fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_str().unwrap();
        if !dir.is_dir() || !name.starts_with("state-") {
            continue;
        }
        let original = fs::read_to_string(dir.join("model.relca")).unwrap();
        let prefix = original
            .split_once("  contract Controls {")
            .map_or(original.as_str(), |(s, _)| s);
        let mut source = prefix
            .lines()
            .filter(|line| {
                let trimmed = line.trim_start();
                !trimmed.starts_with("observed case") && !trimmed.starts_with("latent case")
            })
            .collect::<Vec<_>>()
            .join("\n");
        if prefix.len() != original.len() {
            source.push('}');
        }
        for (from, to) in [
            ("latent difficulty: Difficulty = routine", ""),
            ("latent difficulty: Difficulty", ""),
            ("; writes approval", ""),
            ("; writes registered", ""),
            ("; finish { registered = true }", ""),
            ("; writes changed", ""),
            ("; finish { changed = true }", ""),
            ("; writes flag", ""),
            ("; finish { flag = true }", ""),
            ("; finish { flag = false }", ""),
            ("; writes decision", ""),
            ("; writes route", ""),
            ("; finish { route = routine }", ""),
            ("decision == pass", "j == pass"),
            ("route == difficult", "choice == difficult"),
        ] {
            source = source.replace(from, to);
        }
        if name.starts_with("state-policy") {
            let (head, tail) = source.split_once("  process Flow {").unwrap();
            let (_, tail) = tail.split_once("  bounds").unwrap();
            source = format!("{head} process Flow {{ a = AI.extract(); end done }} bounds{tail}");
        }
        let value = check(&source);
        assert_eq!(
            value["verdict"], "satisfied",
            "{name}: skeleton only: {source}\n{value}"
        );
        if name.starts_with("state-agency") {
            assert_eq!(
                rows(&value, "terminals", "end"),
                BTreeMap::from([
                    (
                        "done".to_owned(),
                        json!({"end":"done","elapsed":{"min":3,"max":3},"work":{"min":3,"max":3},"result_at":null})
                    ),
                    (
                        "rejected".to_owned(),
                        json!({"end":"rejected","elapsed":{"min":2,"max":2},"work":{"min":2,"max":2},"result_at":null})
                    ),
                ])
            );
        } else {
            let (min, max, wmin, wmax, r) = if name.starts_with("state-observed-range") {
                (2, 5, 2, 5, None)
            } else if name == "state-cancel" {
                (3, 3, 4, 4, Some(1))
            } else if name.starts_with("state-cancel.") {
                (6, 6, 7, 7, Some(1))
            } else if name == "state-writer-order" {
                (3, 3, 3, 3, None)
            } else if name.starts_with("state-writer-order.") {
                (2, 2, 3, 3, None)
            } else if name.starts_with("state-invoice-write") {
                (2, 2, 2, 2, None)
            } else {
                (1, 1, 1, 1, None)
            };
            metrics(&value, min, max, wmin, wmax, r);
        }
        count += 1;
    }
    assert_eq!(count, 16);
}

#[test]
fn s_wire_diff_ids_vacuity_and_global_policy_failure() {
    let source = model("observed case { flag: bool = false }", "writes flag; finish { flag = true }", "x = A.run(); end done", "invariant A.run.finish -> flag == true")
        .replace("bounds {", "process Empty { end done }; cut C { include [A,B] }; configuration From { cut C; process Flow }; configuration To { cut C; process Empty }; bounds {");
    for (selection, verdict) in [("From", "satisfied"), ("To", "vacuous")] {
        let value = serde_json::to_value(report::check_configuration(&source, selection)).unwrap();
        assert_eq!(ledger_shape::original_field_count(&value), 13);
        assert_eq!(value["verdict"], verdict);
        assert_eq!(
            value["properties"][1]["id"],
            "contract Controls #1 invariant"
        );
        assert_eq!(value["properties"][1]["verdict"], verdict);
        let sha = value["model"]["sha256"].as_str().unwrap();
        assert_eq!(sha.len(), 64);
        assert!(sha.bytes().all(|b| b.is_ascii_hexdigit()));
    }
    let value = serde_json::to_value(diff::diff(&source, "From", "To")).unwrap();
    assert_eq!(value["verdict"], "vacuous");
    assert_eq!(
        value["properties"][1],
        json!({"id":"contract Controls #1 invariant","from":"satisfied","to":"vacuous"})
    );
    let bad = source.replace("process Empty {", "latent case { truth: bool = false }; process Hidden { if truth == true {} else {}; end done }; process Empty {");
    for selection in ["From", "To"] {
        let value = serde_json::to_value(report::check_configuration(&bad, selection)).unwrap();
        failure_value(&bad, &value, "E-POL-LATENT");
    }
    let value = serde_json::to_value(diff::diff(&bad, "From", "To")).unwrap();
    assert_eq!(value["verdict"], "invalid");
    for key in ["checked", "properties", "terminals", "differences"] {
        assert_eq!(value[key], json!([]));
    }
    for side in ["from", "to"] {
        assert_eq!(value[side]["report"]["verdict"], "invalid");
    }
}

#[test]
fn s_pairs_have_exactly_one_declared_source_edit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (base, fault, from, to) in [
        (
            "state-policy",
            "latent-difficulty",
            "difficult_seen == true",
            "Invoice.difficulty == difficult",
        ),
        (
            "state-latent-product",
            "partial-assertion",
            "(Invoice.difficulty == routine || Invoice.difficulty == difficult)",
            "Invoice.difficulty == routine",
        ),
        (
            "state-invoice-write",
            "failed-approval",
            "result nondet { pass }",
            "result nondet { fail }",
        ),
        (
            "state-cancel",
            "ignore-cancel",
            "cancel cooperative ack_within 1min",
            "cancel forbidden_after_start",
        ),
        ("state-name-alias", "missing-write", "; writes approval", ""),
        (
            "state-writer-order",
            "parallel-writers",
            "a = A.set(); b = B.clear()",
            "fork f { a = A.set(); b = B.clear() }; join all f",
        ),
        (
            "state-agency",
            "state-guard",
            "j == pass",
            "decision == pass",
        ),
        (
            "state-observed-range",
            "fixed-route",
            "writes route",
            "writes route; finish { route = routine }",
        ),
    ] {
        let source = fs::read_to_string(root.join(base).join("model.relca")).unwrap();
        assert_eq!(source.matches(from).count(), 1, "one site: {base}");
        let broken = fs::read_to_string(
            root.join(format!("{base}.broken-{fault}"))
                .join("model.relca"),
        )
        .unwrap();
        assert_eq!(
            source.replace(from, to),
            broken,
            "one semantic edit: {base}"
        );
    }
}

// Review (29267c7..5a88cb6) findings. Each case guards one review item.

#[test]
fn review_m1_binding_named_like_case_latent_is_rejected_even_for_invariants() {
    // The binding a is always pass; reading the latent a = fail silently would
    // flip the invariant. The shared name is unsupported, not resolved.
    failure_at(
        &model(
            "latent case { a: T = fail }",
            "",
            "a = A.run(); end done",
            "invariant A.run.finish -> a == pass",
        ),
        "U-SEM-0",
        "a == pass",
    );
}

#[test]
fn review_m2_else_branch_latent_read_is_policy_error_selected_or_not() {
    let selected = model(
        "latent case { truth: T = pass }",
        "",
        "x = A.run(); if x == pass {} else { if truth == pass {} else {} }; end done",
        "",
    );
    failure_at(&selected, "E-POL-LATENT", "truth == pass");
    let source = configured_other(
        "x = A.run(); if x == pass {} else { if truth == pass {} else {} }; end done",
    );
    let value = serde_json::to_value(report::check_configuration(&source, "Chosen")).unwrap();
    failure_value(&source, &value, "E-POL-LATENT");
}

#[test]
fn review_m3_reentry_entry_still_rejects_angle_brackets() {
    let source = model("", "", "x = A.run(); end done", "").replace(
        "bounds {",
        "cut C { include [A, B]; reentry [review when x < 3] }; bounds {",
    );
    failure_at(&source, "E-SYN-0", "<");
}

#[test]
fn review_m4_observed_and_latent_without_case_are_both_unsupported() {
    for item in [
        "observed actor { x: T = pass }",
        "latent actor { x: T = pass }",
    ] {
        let source = model(item, "", "x = A.run(); end done", "");
        failure_at(&source, "U-SYN-0", item.split(' ').next().unwrap());
    }
}

#[test]
fn review_m7_effect_reached_through_any_state_guard_omits_its_refusal() {
    // post is reached via the binding guard j == pass or, failing that, via the
    // state guard decision == fail. One state-guarded path is enough to omit.
    let source = AGENCY.replace(
        "if j == pass { p = Ledger.post(); end done } else { end rejected }",
        "if j == pass {} else { if decision == fail {} else { end rejected } }\n    p = Ledger.post()\n    end done",
    );
    let value = check(&source);
    assert_eq!(value["verdict"], "satisfied", "{value}");
    assert_eq!(value["agency"]["rules"], "agency-p1-state-1");
    assert_eq!(value["agency"]["refusal"], json!([]), "{value}");
    assert!(
        value["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("state_guards"))
    );
    assert!(!has_code(&value, "W-AGY-0") && !has_code(&value, "W-AGY-1"));
}

#[test]
fn review_u8_invariant_reading_excluded_actor_latent_is_a_cut_error() {
    let source = model(
        "",
        "",
        "x = A.run(); end done",
        "invariant A.run.finish -> D.mood == pass",
    )
    .replace("actor A {", "actor D { latent mood: T }\n      actor A {")
    .replace(
        "bounds {",
        "cut C { include [A, B]; exclude D reason \"外部\" loses \"気分\" }; bounds {",
    );
    failure_at(&source, "E-CUT-1", "D.mood");
}
