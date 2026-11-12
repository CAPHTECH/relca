//! Independent merge-review acceptance; no changes to the frozen Q6b suite.
#[path = "support/q6b_oracle.rs"]
mod hand;
#[path = "support/q6b_inputs.rs"]
mod input;
#[path = "support/q6b_review_inputs.rs"]
mod review;
use relca_core::{Verdict, agency, engine, ir, lower, reference, report, syntax};
use serde_json::{Value, json};

fn checked(src: &str) -> report::Report {
    let r = report::check(src);
    assert!(
        !matches!(r.verdict, Verdict::Invalid | Verdict::Unsupported),
        "{:?}",
        r.diagnostics
    );
    assert!(r.agency.as_ref().unwrap().degradation.is_some());
    r
}
fn table(src: &str) -> Value {
    json!(checked(src))["agency"]["degradation"].clone()
}
fn unknown(reason: &str) -> Value {
    json!({"status":"not_derived","reason":reason,"ends":null,"observation":null})
}
fn expect_table(src: &str, want: Value) {
    let actual = table(src);
    hand::subset(&want, &actual, "/agency/degradation").unwrap();
    assert_eq!(actual, want);
}
fn empty_side(v: &mut Value, key: &str) {
    let s = &mut v["routes"][0][key];
    s["ends"][0] = json!({"end":"done","reachable":false,"counts":hand::counts(&[]),"may_require":[],"must_require":[]});
    s["observation"] = json!([]);
}

#[test]
fn mj1_arrival_zero_preserves_suffix_start_counts() {
    expect_table(&review::coverage(0, false), hand::expected("suffix"));
}
#[test]
fn mj1_arrival_offset_does_not_certify_outside_calendar_start() {
    let t = table(&review::coverage(9, false));
    if t["status"] == "not_derived" {
        assert_eq!(t["reason"], "calendar");
        assert!(t["routes"].is_null());
    } else {
        let s = &t["routes"][0]["degraded"];
        if s["status"] == "not_derived" {
            assert_eq!(s, &unknown("calendar"));
        } else {
            assert_eq!(
                s["ends"][0]["reachable"], false,
                "Finish earliest start13 is outside [0,11), never derived 1..1 or must: {s}"
            );
            assert_eq!(s["ends"][0]["counts"], hand::counts(&[]));
            assert_eq!(s["ends"][0]["may_require"], json!([]));
            assert_eq!(s["ends"][0]["must_require"], json!([]));
        }
    }
}
#[test]
fn m4_calendar_coverage_long_task_withdraws_not_zero() {
    let mut want = hand::expected("suffix");
    want["routes"][0]["degraded"] = unknown("calendar");
    expect_table(&review::coverage(0, true), want);
    expect_table(&review::coverage(0, false), hand::expected("suffix"));
}

macro_rules! domain {
    ($name:ident, $decl:literal, $absent:literal) => {
        #[test]
        fn $name() {
            let mut want = hand::expected("a7");
            empty_side(&mut want, $absent);
            expect_table(&review::domain($decl), want);
        }
    };
}
domain!(
    u2_nondet_ok_excludes_enum_fail,
    "result nondet { ok }",
    "degraded"
);
domain!(
    u2_nondet_fail_excludes_enum_ok,
    "result nondet { fail }",
    "normal"
);
fn single_ir(value: usize, absent: &str) {
    let mut m = lower::lower(&syntax::parse(&input::base()).unwrap()).unwrap();
    m.capabilities[0].results = vec![ir::ValueId(value)];
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert!(
        e.diagnostics.iter().all(|d| !d.code.starts_with("U-")),
        "{:?}",
        e.diagnostics
    );
    assert_eq!(r.stop, reference::StopReason::Complete);
    let mut want = hand::expected("a7");
    empty_side(&mut want, absent);
    let actual = json!(agency::derive(&m).unwrap().0)["degradation"].clone();
    hand::subset(&want, &actual, "/agency/degradation").unwrap();
    assert_eq!(actual, want);
}
#[test]
fn u2_single_ok_public_ir_excludes_enum_fail() {
    single_ir(0, "degraded");
}
#[test]
fn u2_single_fail_public_ir_excludes_enum_ok() {
    single_ir(1, "normal");
}
#[test]
fn u2_declared_kernel_branches_ignore_zero_mass_and_probability_values() {
    for reverse in [false, true] {
        expect_table(&review::probability(reverse), hand::expected("a7"));
    }
}
#[test]
fn m4_single_case_has_no_fabricated_interference() {
    expect_table(&review::multiple(false), hand::expected("a7"));
}
#[test]
fn m4_m8_multiple_cases_withdraw_sides_and_common_prefix_together() {
    let mut want = hand::expected("a7");
    want["status"] = json!("not_derived");
    want["reason"] = json!("multiple_case_interference");
    want["routes"] = Value::Null;
    expect_table(&review::multiple(true), want);
}
#[test]
fn m4_history_dependent_results_are_shared_state_guard_not_free_enum() {
    let mut want = hand::expected("a7");
    want["routes"][0]["degraded"] = unknown("shared_state_guard");
    expect_table(&review::history(true), want);
    expect_table(&review::history(false), hand::expected("a7"));
}
#[test]
fn m4_resource_impossible_is_unknown_not_lexical_start_one() {
    let mut want = hand::expected("a7");
    want["routes"][0]["degraded"] = unknown("resource_impossible");
    expect_table(&review::resource(true), want);
    expect_table(&review::resource(false), hand::expected("a7"));
}

fn decode(v: Value) -> Result<report::Report, serde_json::Error> {
    serde_json::from_value(v)
}
fn cut_wire() -> Value {
    let good = checked(&input::source("clock"));
    assert_eq!(good.verdict, Verdict::Inconclusive);
    let v = json!(good);
    assert_eq!(v["bounds"]["truncated"], true);
    decode(v.clone()).expect("positive truncated/inconclusive wire");
    v
}
#[test]
fn u3_truncated_inconclusive_wire_is_valid() {
    cut_wire();
}
macro_rules! truncated {
    ($test:ident, $root:literal) => {
        #[test]
        fn $test() {
            let mut v = cut_wire();
            v["verdict"] = json!($root);
            assert!(
                decode(v).is_err(),
                "degrade plus truncated cannot decode root {}",
                $root
            );
        }
    };
}
truncated!(u3_truncated_satisfied_wire_is_rejected, "satisfied");
truncated!(u3_truncated_vacuous_wire_is_rejected, "vacuous");

// seal refreshes derived uncertainty/model pointers and ledger component links;
// it does not validate the degradation host contract. Thus no dangling pointer
// becomes an accidental oracle for the missing table/assumption tests.
fn reseal(r: report::Report) -> Value {
    let r = report::seal(r);
    assert!(
        !r.diagnostics.iter().any(|d| d.code == "U-LEDGER-0"),
        "{:?}",
        r.diagnostics
    );
    json!(r)
}
#[test]
fn m7_complete_declaration_and_scope_wire_is_valid() {
    decode(reseal(checked(&input::base()))).unwrap();
}
#[test]
fn m7_declaration_without_table_is_rejected() {
    let mut r = checked(&input::base());
    r.agency.as_mut().unwrap().degradation = None;
    assert!(
        decode(reseal(r)).is_err(),
        "declared degradation cannot disappear from wire"
    );
}
#[test]
fn m7_missing_whole_process_retention_scope_is_rejected() {
    let mut r = checked(&input::base());
    let before = r.assumptions.len();
    r.assumptions
        .retain(|a| a.field != "agency.retention.normal.scope");
    assert_eq!(r.assumptions.len() + 1, before);
    assert!(
        decode(reseal(r)).is_err(),
        "new side table must not silently redefine old retention.normal"
    );
}
#[test]
fn m7_table_without_declaration_is_rejected() {
    let mut r = checked(&input::base());
    let before = r.assumptions.len();
    r.assumptions
        .retain(|a| a.field != "agency.degradation.declarations");
    assert_eq!(r.assumptions.len() + 1, before);
    assert!(
        decode(reseal(r)).is_err(),
        "existing direction remains checked"
    );
}

fn empty_ir(capability: usize) {
    // U2 supplementary ruling: no-result DSL is rejected by lower; do not open
    // new grammar. Empty IR must fail validation, not later clock progression.
    // Exercise public entrances and do not feed refused IR to Agency.
    let mut m = lower::lower(&syntax::parse(&input::base()).unwrap()).unwrap();
    m.capabilities[capability].results.clear();
    let e = engine::explore(&m);
    let r = reference::explore(&m);
    assert_eq!(r.stop, reference::StopReason::SemanticAbort);
    assert!(r.outcome.terminals.is_empty());
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0" && d.message.contains("empty result")),
        "empty result validation must precede runtime, got {:?}",
        e.diagnostics
    );
    assert!(e.terminals.is_empty());
    assert!(
        e.case_outcomes
            .iter()
            .flatten()
            .all(|c| c.terminals.is_empty())
    );
}
#[test]
fn u2_empty_public_ir_is_rejected_without_deriving_a_dependency_table() {
    empty_ir(0);
}
#[test]
fn u2_empty_unused_capability_is_rejected_before_execution_not_on_encounter() {
    // Other.enter is never called. A runtime-only empty-result error cannot
    // satisfy this test: declarations must be validated before any execution.
    empty_ir(3);
}
