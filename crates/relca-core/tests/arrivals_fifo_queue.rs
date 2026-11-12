//! ADR 0015 public acceptance. Fractions and time extrema precede implementation.
use relca_core::{diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{fs, path::Path};

const COUNT: &str = include_str!("../../../fixtures/arrivals-count/model.relca");
const SIMULTANEOUS: &str = include_str!("../../../fixtures/arrivals-simultaneous/model.relca");
const RELATIVE: &str = include_str!("../../../fixtures/arrivals-relative-within/model.relca");
const ZERO: &str = include_str!("../../../fixtures/arrivals-zero-queue/model.relca");

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn legacy_source(s: &str) -> String {
    s.lines()
        .filter(|l| {
            let l = l.trim_start();
            !l.starts_with("arrivals fixed")
                && !l.starts_with("policy fifo")
                && !l.starts_with("queue capacity")
        })
        .collect::<Vec<_>>()
        .join("\n")
        .replace("cases 2", "cases 1")
        .replace("cases 3", "cases 1")
}
fn legacy_grammar_control(s: &str) {
    let legacy = legacy_source(s);
    relca_core::syntax::parse(&legacy)
        .and_then(|a| relca_core::lower::lower(&a))
        .unwrap_or_else(|d| panic!("existing grammar control, not arrivals RED: {d:?}\n{legacy}"));
}
fn fixture_source(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures")
            .join(name)
            .join("model.relca"),
    )
    .unwrap()
}
fn row<'a>(v: &'a Value, id: &str) -> &'a Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))
}
fn probability(v: &Value, query: usize, case: usize, min: &str, max: &str, verdict: &str) {
    let p = row(v, &format!("probability reached #{query}"));
    assert!(p.get("probability").is_none(), "not a cohort: {p}");
    let c = &p["cases"][case - 1];
    assert_eq!(c["case_id"], case);
    assert_eq!(c["verdict"], verdict, "{v}");
    assert_eq!(c["probability"], json!({"min":min,"max":max}), "{v}");
}
fn metric(v: &Value, case: usize, key: &str, min: u64, max: u64) {
    assert_eq!(
        v["case_outcomes"][case - 1]["terminals"][0][key],
        json!({"min":min,"max":max}),
        "case {case}: {v}"
    );
}
fn new_wire(v: &Value, arrivals: &[u64]) {
    let cut = v["bounds"]["truncated"] == true;
    assert_eq!(
        ledger_shape::original_field_count(v),
        if cut { 16 } else { 15 },
        "{v}"
    );
    let mut keys = vec![
        "actors",
        "admission",
        "agency",
        "assumptions",
        "bounds",
        "case_outcomes",
        "cut",
        "cut_context",
        "diagnostics",
        "ledger",
        "mode",
        "model",
        "properties",
        "terminals",
        "uncertainty",
        "unit",
        "verdict",
    ];
    if cut {
        keys.push("unchecked");
        keys.sort();
    }
    assert_eq!(
        v.as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        keys
    );
    assert_eq!(
        v.get("unchecked"),
        cut.then(|| json!(["unreachable"])).as_ref()
    );
    assert_eq!(v["terminals"], json!([]));
    let outcomes = v["case_outcomes"].as_array().unwrap();
    assert_eq!(outcomes.len(), arrivals.len());
    for (i, (c, arrival)) in outcomes.iter().zip(arrivals).enumerate() {
        assert_eq!(c["case_id"], i + 1);
        assert_eq!(c["arrival"], *arrival);
        for t in c["terminals"].as_array().unwrap() {
            assert!(matches!(t["kind"].as_str(), Some("end" | "rejected")));
            if t["kind"] == "rejected" {
                for k in ["end", "completed_at", "elapsed", "work", "result_at"] {
                    assert_eq!(t.get(k), Some(&Value::Null), "{v}");
                }
            }
        }
    }
    for p in v["properties"].as_array().unwrap() {
        if p["id"] == "safety no_resource_overcommit" {
            assert!(p.get("cases").is_none());
        } else {
            let cs = p["cases"].as_array().unwrap();
            assert_eq!(cs.len(), arrivals.len());
            for (i, c) in cs.iter().enumerate() {
                assert_eq!(c["case_id"], i + 1);
                assert_eq!(c["arrival"], arrivals[i]);
            }
        }
    }
    let unchecked = v["agency"]["unchecked"].as_array().unwrap();
    assert_eq!(
        unchecked
            .iter()
            .filter(|x| *x == "multi_case_interference")
            .count(),
        usize::from(arrivals.len() > 1)
    );
    // ADR 0043 (plan C04) appends responsibility_people_undeclared after the
    // existing markers when the model declares no people.
    let last = unchecked
        .iter()
        .rev()
        .find(|x| *x != "responsibility_people_undeclared");
    if arrivals.len() > 1 {
        assert_eq!(last.unwrap(), "multi_case_interference");
    }
}
fn static_shape(v: &Value) {
    assert_eq!(ledger_shape::original_field_count(v), 13);
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert!(v["agency"].is_null());
    assert!(v.get("case_outcomes").is_none());
    assert!(v.get("admission").is_none());
    for k in ["cases", "live_tasks", "clock", "truncated"] {
        assert_eq!(v["bounds"].get(k), Some(&Value::Null));
    }
}
fn failure(s: &str, code: &str) -> Value {
    let v = check(s);
    assert_eq!(
        v["verdict"],
        if code.starts_with('E') {
            "invalid"
        } else {
            "unsupported"
        },
        "{s}\n{v}"
    );
    static_shape(&v);
    let ds: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(!ds.is_empty());
    for d in ds {
        assert_eq!(d["code"], code, "{s}\n{v}");
        let a = d["span"]["start"].as_u64().unwrap() as usize;
        let b = d["span"]["end"].as_u64().unwrap() as usize;
        assert!(a <= b && b <= s.len() && s.is_char_boundary(a) && s.is_char_boundary(b));
    }
    v
}
/// ADR 0035 U12(意味の注記の条件と性質の行の形): atomic の check-and-insert を
/// 持たない旧いモデルでは、at_most_once の行にだけ notes ["mechanism_unmodeled"] が
/// 足される。他の行・verdict・cases は旧い手計算の期待のまま。at_most_once 以外の
/// 行に notes が付けば、この期待と一致しないので落ちる。
fn with_at_most_once_notes(source: &str, properties: &Value) -> Value {
    assert!(!source.contains("atomic"), "pre-A3 oracle only");
    let mut rows = properties.clone();
    for row in rows.as_array_mut().unwrap() {
        assert!(row.get("notes").is_none(), "hand oracle carries no notes");
        if row["id"].as_str().unwrap().ends_with(" at_most_once") {
            row["notes"] = json!(["mechanism_unmodeled"]);
        }
    }
    rows
}
fn hand(name: &str) {
    let s = fixture_source(name);
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name)
        .join("expected.json");
    let h: Value = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    let v = check(&s);
    for key in ["verdict", "mode", "unit", "cut", "bounds", "terminals"] {
        assert_eq!(v[key], h[key], "{name}: {key}: {v}");
    }
    assert_eq!(
        v["properties"],
        with_at_most_once_notes(&s, &h["properties"]),
        "{name}: properties: {v}"
    );
    if let Some(cases) = h.get("case_outcomes") {
        assert_eq!(v["case_outcomes"], *cases, "{name}");
        assert_eq!(v["admission"], h["admission"]);
        let arrivals: Vec<_> = cases
            .as_array()
            .unwrap()
            .iter()
            .map(|c| c["arrival"].as_u64().unwrap())
            .collect();
        new_wire(&v, &arrivals);
        assumptions(&s, &v, &arrivals);
    } else {
        static_shape(&v);
    }
    for d in h["diagnostics"].as_array().unwrap() {
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["code"] == d["code"] && a["severity"] == d["severity"]),
            "{name}: {v}"
        );
    }
    for code in h["absent_diagnostics"].as_array().unwrap() {
        assert!(
            !v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["code"] == *code)
        );
    }
}
fn assumptions(s: &str, v: &Value, arrivals: &[u64]) {
    let rows = v["assumptions"].as_array().unwrap();
    let mut expected = vec![
        json!({"field":"arrivals","value":format!("fixed[{}]",arrivals.iter().map(|t|format!("{t}min")).collect::<Vec<_>>().join(", ")),"affects":"arrival times are fixed analyst assumptions; case IDs follow list order"}),
    ];
    if s.contains("x: Route ~") {
        expected.push(json!({"field":"initial.case_independence","value":"independent-per-admission","affects":"case initial distributions are drawn independently for each admitted case at receipt; this independence is an analyst assumption"}));
    }
    if s.contains("policy fifo") {
        expected.push(json!({"field":"policy.queue","value":"fifo tie_by case_id","affects":"each start compares only the tasks queued within the Actor at that moment, by queue-entry time then case ID; tasks entering the queue later in the same instant are not compared; ties within one case remain nondeterministic"}));
    }
    if let Some(tail) = s.split("queue capacity ").nth(1) {
        let n = tail.split_whitespace().next().unwrap();
        expected.push(json!({"field":"queue.capacity","value":format!("{n} on_overflow reject"),"affects":"admitted nonterminal cases with no task ever started are counted before same-time starts; rejected cases retain their probability mass"}));
    }
    let names = [
        "arrivals",
        "initial.case_independence",
        "policy.queue",
        "queue.capacity",
    ];
    let actual: Vec<_> = rows
        .iter()
        .filter(|r| names.contains(&r["field"].as_str().unwrap()))
        .cloned()
        .collect();
    assert_eq!(actual, expected);
    let start = rows.iter().position(|r| r["field"] == "arrivals").unwrap();
    assert_eq!(&rows[start..start + expected.len()], expected.as_slice());
    for r in &rows[start + expected.len()..] {
        assert_eq!(r["field"], "probability.scheduler");
    }
    if s.contains("policy fifo") {
        let schedule = rows
            .iter()
            .find(|r| r["field"] == "schedule")
            .expect("schedule assumption");
        assert_eq!(
            schedule["affects"],
            "competing starts unresolved by the declared FIFO rule remain nondeterministic"
        );
    }
}

macro_rules! fixtures { ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test(){hand($name);})+}; }
fixtures! {
    f1_simultaneous => "arrivals-simultaneous", f1_no_policy => "arrivals-simultaneous.broken-no-policy",
    f2_staggered => "arrivals-staggered", f2_capacity => "arrivals-staggered.broken-extra-capacity",
    f3_reject_mass => "arrivals-reject-mass", f3_queue => "arrivals-reject-mass.broken-larger-queue",
    f4_same_tick => "arrivals-same-time-slot", f4_later => "arrivals-same-time-slot.broken-later-arrival",
    f5_initial => "arrivals-initial-product", f5_fixed => "arrivals-initial-product.broken-fixed-initial",
    f6_relative => "arrivals-relative-within", f6_deadline => "arrivals-relative-within.broken-tight-deadline",
    f7_count => "arrivals-count", f7_mismatch => "arrivals-count.broken-case-count",
    f8_order => "arrivals-order", f8_descending => "arrivals-order.broken-descending",
    f9_required => "arrivals-required", f9_missing => "arrivals-required.broken-missing-arrivals",
    f10_shared => "arrivals-shared-fixed", f10_unset => "arrivals-shared-fixed.broken-unset-shared",
    f11_contracts => "arrivals-contracts", f11_response => "arrivals-contracts.broken-short-response",
    f12_zero => "arrivals-zero-queue", f12_positive => "arrivals-zero-queue.broken-positive-queue",
}

#[test]
fn pairs_are_one_source_replacement_not_two_faults() {
    for (base, fault, from, to) in [
        (
            "simultaneous",
            "no-policy",
            "  policy fifo tie_by case_id\n",
            "",
        ),
        ("staggered", "extra-capacity", "capacity 1", "capacity 2"),
        (
            "reject-mass",
            "larger-queue",
            "queue capacity 1",
            "queue capacity 2",
        ),
        (
            "same-time-slot",
            "later-arrival",
            "[0min, 1min, 2min]",
            "[0min, 1min, 3min]",
        ),
        (
            "initial-product",
            "fixed-initial",
            "x: Route ~ { short:0.5, long:0.5 }",
            "x: Route = short",
        ),
        (
            "relative-within",
            "tight-deadline",
            "within 2min",
            "within 1min",
        ),
        ("count", "case-count", "cases 2", "cases 3"),
        ("order", "descending", "[0min, 1min]", "[1min, 0min]"),
        (
            "required",
            "missing-arrivals",
            "  arrivals fixed [0min, 1min]\n",
            "",
        ),
        (
            "shared-fixed",
            "unset-shared",
            "truth: bool = true",
            "truth: bool",
        ),
        ("contracts", "short-response", "by 4min", "by 3min"),
        (
            "zero-queue",
            "positive-queue",
            "queue capacity 0",
            "queue capacity 1",
        ),
    ] {
        let s = fixture_source(&format!("arrivals-{base}"));
        assert_eq!(s.matches(from).count(), 1);
        assert_eq!(
            s.replacen(from, to, 1),
            fixture_source(&format!("arrivals-{base}.broken-{fault}"))
        );
    }
}

#[test]
fn legacy_grammar_controls_isolate_new_feature_red_from_fixture_typos() {
    for name in [
        "simultaneous",
        "staggered",
        "reject-mass",
        "same-time-slot",
        "initial-product",
        "relative-within",
        "count",
        "order",
        "required",
        "shared-fixed",
        "contracts",
        "zero-queue",
    ] {
        legacy_grammar_control(&fixture_source(&format!("arrivals-{name}")));
    }
    legacy_grammar_control(CANCEL_READ);
}

#[test]
fn a_diagnostic_matrix_syntax_semantics_and_fail_closed_boundaries() {
    for (from, to, code) in [
        (
            "arrivals fixed [0min, 1min]",
            "arrivals fixed []",
            "E-SYN-0",
        ),
        ("[0min, 1min]", "0min, 1min]", "E-SYN-0"),
        ("[0min, 1min]", "[0min, 1min", "E-SYN-0"),
        ("[0min, 1min]", "[0min 1min]", "E-SYN-0"),
        ("[0min, 1min]", "[0min, 1min,]", "E-SYN-0"),
        ("[0min, 1min]", "[0min,]", "E-SYN-0"),
        ("[0min, 1min]", "[0min, 1]", "E-SYN-0"),
        ("[0min, 1min]", "[0min, 1sec]", "U-SYN-0"),
        ("[0min, 1min]", "[0min, 1.5min]", "U-SYN-0"),
        ("[0min, 1min]", "[0min, delay]", "U-SYN-0"),
        ("[0min, 1min]", "[0min, 1min+1min]", "U-SYN-0"),
        ("[0min, 1min]", "[0min, 18446744073709551616min]", "U-SYN-0"),
        (
            "arrivals fixed [0min, 1min]",
            "arrivals renewal_every 1min",
            "U-SYN-0",
        ),
        (
            "arrivals fixed [0min, 1min]",
            "arrivals random [0min, 1min]",
            "U-SYN-0",
        ),
        (
            "arrivals fixed [0min, 1min]",
            "arrivals business_hours [0min, 1min]",
            "U-SYN-0",
        ),
        (
            "policy fifo tie_by case_id",
            "policy priority tie_by case_id",
            "U-SYN-0",
        ),
        ("policy fifo tie_by case_id", "policy random", "U-SYN-0"),
        (
            "policy fifo tie_by case_id",
            "policy partial_observation fifo tie_by case_id",
            "U-SYN-0",
        ),
        (
            "policy fifo tie_by case_id",
            "policy fifo case_id",
            "E-SYN-0",
        ),
        (
            "policy fifo tie_by case_id",
            "policy fifo tie_by person_id",
            "U-SYN-0",
        ),
        (
            "policy fifo tie_by case_id",
            "policy fifo tie_by case_id\n policy fifo tie_by case_id",
            "U-SEM-0",
        ),
        (
            "arrivals fixed [0min, 1min]",
            "arrivals fixed [0min, 1min]\n arrivals fixed [0min, 1min]",
            "U-SEM-0",
        ),
        ("cases 2", "cases 0", "U-SEM-0"),
        ("capacity 1;", "", "E-CAP-0"),
        (
            "verify reachable end.done",
            "verify reachable end.absent",
            "E-NAME-0",
        ),
    ] {
        assert_eq!(COUNT.matches(from).count(), 1);
        failure(&COUNT.replacen(from, to, 1), code);
    }
    for (from, to, code) in [
        (
            "queue capacity 0 on_overflow reject",
            "queue 0 on_overflow reject",
            "E-SYN-0",
        ),
        (
            "queue capacity 0 on_overflow reject",
            "queue capacity on_overflow reject",
            "E-SYN-0",
        ),
        (
            "queue capacity 0 on_overflow reject",
            "queue capacity 0 reject",
            "E-SYN-0",
        ),
        ("on_overflow reject", "on_overflow wait", "U-SYN-0"),
        ("on_overflow reject", "on_overflow overflow", "U-SYN-0"),
        (
            "on_overflow reject",
            "on_overflow reject_and_count",
            "U-SYN-0",
        ),
        (
            "queue capacity 0",
            "queue capacity 18446744073709551616",
            "U-SYN-0",
        ),
        (
            "queue capacity 0 on_overflow reject",
            "queue capacity 0 on_overflow reject\n queue capacity 0 on_overflow reject",
            "U-SEM-0",
        ),
    ] {
        failure(&ZERO.replacen(from, to, 1), code);
    }
    // Missing words at EOF distinguish missing syntax from unknown choices.
    for suffix in ["policy fifo tie_by", "queue capacity 1 on_overflow"] {
        failure(&format!("model M version \"0.2\" {{ {suffix}"), "E-SYN-0");
    }
    let shared = fixture_source("arrivals-shared-fixed");
    failure(
        &shared.replace(
            "truth: bool = true",
            "truth: bool ~ { true:0.5, false:0.5 }",
        ),
        "U-SEM-0",
    );
    let invalid_shared = check(&shared.replace(
        "truth: bool = true",
        "truth: bool ~ { true:0.5, false:0.4 }",
    ));
    assert_eq!(invalid_shared["verdict"], "invalid");
    static_shape(&invalid_shared);
    assert!(
        invalid_shared["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-PROB-0" && d["severity"] == "error")
    );
    failure(
        &RELATIVE.replace(
            "r = A.run(); end done",
            "r = A.run(); if missing == true { end other } else { end done }",
        ),
        "E-NAME-0",
    );
}

#[test]
fn a_arrival_errors_have_exact_byte_spans_and_static_nulls() {
    for (s, span_text, occurrence) in [
        (
            COUNT.replace("cases 2", "cases 3"),
            "arrivals fixed [0min, 1min]",
            0,
        ),
        (COUNT.replace("[0min, 1min]", "[1min, 0min]"), "0min", 0),
        (COUNT.replace("  arrivals fixed [0min, 1min]\n", ""), "2", 1),
    ] {
        let v = failure(&s, "E-ARR-0");
        let d = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .find(|d| d["code"] == "E-ARR-0")
            .unwrap();
        let start = d["span"]["start"].as_u64().unwrap() as usize;
        let end = d["span"]["end"].as_u64().unwrap() as usize;
        assert_eq!(&s[start..end], span_text);
        if occurrence == 1 {
            assert_eq!(start, s.find("cases 2").unwrap() + 6);
        }
    }
}

#[test]
fn a_configuration_declaration_selection_and_diff_are_case_local() {
    let with = COUNT.replace(
        "  bounds",
        "  cut K { include [A] }\n  configuration C { cut K; process Flow }\n  bounds",
    );
    failure(&with, "E-CONF-0");
    let selected = serde_json::to_value(report::check_configuration(&with, "C")).unwrap();
    assert_eq!(selected["verdict"], "satisfied");
    assert_eq!(selected["case_outcomes"].as_array().unwrap().len(), 2);
    let d = serde_json::to_value(diff::diff(&with, "C", "C")).unwrap();
    assert_eq!(d["verdict"], "satisfied");
    assert_eq!(d["case_outcomes"].as_array().unwrap().len(), 2);
    for source in [COUNT, with.as_str()] {
        let code = if source == COUNT {
            "E-CONF-0"
        } else {
            "E-NAME-0"
        };
        let v = serde_json::to_value(report::check_configuration(source, "Unknown")).unwrap();
        assert_eq!(v["verdict"], "invalid");
        static_shape(&v);
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code)
        );
        let v = serde_json::to_value(diff::diff(source, "Unknown", "D")).unwrap();
        assert_eq!(v["verdict"], "invalid");
        for side in ["from", "to"] {
            assert!(
                v[side]["report"]["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == code)
            );
        }
    }
}

#[test]
fn a_single_new_and_legacy_keys_assumptions_and_agency_do_not_mix() {
    let old = COUNT
        .replace("  arrivals fixed [0min, 1min]\n", "")
        .replace("  policy fifo tie_by case_id\n", "")
        .replace("cases 2", "cases 1");
    let v = check(&old);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(ledger_shape::original_field_count(&v), 13);
    for k in ["case_outcomes", "admission"] {
        assert!(v.get(k).is_none());
    }
    for r in v["assumptions"].as_array().unwrap() {
        assert!(
            ![
                "arrivals",
                "initial.case_independence",
                "policy.queue",
                "queue.capacity"
            ]
            .contains(&r["field"].as_str().unwrap())
        );
    }
    assert!(
        v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p.get("cases").is_none())
    );
    for clause in [
        "arrivals fixed [5min]",
        "policy fifo tie_by case_id",
        "queue capacity 0 on_overflow reject",
    ] {
        let source = old.replacen("  bounds", &format!("  {clause}\n  bounds"), 1);
        let n = check(&source);
        let arrival = if clause.starts_with("arrivals") { 5 } else { 0 };
        new_wire(&n, &[arrival]);
        assert_eq!(
            n["agency"], v["agency"],
            "new one-case still derives old Agency"
        );
        if clause.starts_with("queue") {
            assert_eq!(n["case_outcomes"][0]["terminals"][0]["kind"], "rejected");
        } else {
            metric(&n, 1, "completed_at", arrival + 2, arrival + 2);
            metric(&n, 1, "elapsed", 2, 2);
        }
    }
}

#[test]
fn a_no_task_end_is_not_a_reject_or_an_admission_slot_freeing_shortcut() {
    let s = COUNT
        .replace("r = A.run(); end done", "end done")
        .replace("[0min, 1min]", "[0min, 0min]")
        .replace(
            "  bounds",
            "  queue capacity 1 on_overflow reject\n  bounds",
        );
    let v = check(&s);
    new_wire(&v, &[0, 0]);
    metric(&v, 1, "completed_at", 0, 0);
    metric(&v, 1, "elapsed", 0, 0);
    metric(&v, 1, "work", 0, 0);
    assert_eq!(v["case_outcomes"][0]["terminals"][0]["kind"], "end");
    assert_eq!(v["case_outcomes"][1]["terminals"][0]["kind"], "rejected");
    assert_eq!(
        row(&v, "reachable end.done")["cases"][1]["verdict"],
        "violated"
    );
    assert_eq!(v["admission"]["rejected_count"], json!({"min":1,"max":1}));
    assert_eq!(v["bounds"]["truncated"], false);
}

#[test]
fn a_rejection_retains_zero_hit_even_with_initial_true_and_contract_vacuity() {
    let s = ZERO.replace("hit: bool = false","hit: bool = true").replace("  bounds","  contract C { response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved }\n  bounds");
    let v = check(&s);
    new_wire(&v, &[0, 1]);
    for c in 1..=2 {
        probability(&v, 1, c, "0", "0", "satisfied");
    }
    assert_eq!(row(&v, "contract C #1 response")["verdict"], "vacuous");
    assert_eq!(v["verdict"], "vacuous");
    let mixed = s
        .replace("queue capacity 0", "queue capacity 1")
        .replace("[0min, 1min]", "[0min, 0min]");
    let v = check(&mixed);
    assert_eq!(v["verdict"], "vacuous");
    let p = row(&v, "contract C #1 response");
    assert_eq!(p["cases"][0]["verdict"], "satisfied");
    assert_eq!(p["cases"][1]["verdict"], "vacuous");
}

#[test]
fn a_liveness_future_arrivals_and_zero_capacity_deadlock() {
    let s = COUNT.replace("[0min, 1min]", "[5min, 6min]");
    let v = check(&s);
    assert_eq!(v["verdict"], "satisfied");
    metric(&v, 1, "completed_at", 7, 7);
    metric(&v, 2, "completed_at", 9, 9);
    let v = check(&s.replace("capacity 1", "capacity 0"));
    assert_eq!(v["verdict"], "violated");
    assert_eq!(
        row(&v, "safety no_resource_overcommit")["verdict"],
        "satisfied"
    );
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "W-DEADLOCK-0")
    );
    assert_eq!(v["case_outcomes"][0]["terminals"], json!([]));
    assert_eq!(v["case_outcomes"][1]["terminals"], json!([]));
}

#[test]
fn a_live_per_case_not_global_and_strict_overflow() {
    let s = COUNT
        .replace("[0min, 1min]", "[0min, 0min]")
        .replace("capacity 1", "capacity 2")
        .replace("fixed 2min", "fixed 4min");
    let v = check(&s);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["bounds"]["truncated"], false);
    for c in 1..=2 {
        metric(&v, c, "completed_at", 4, 4);
        metric(&v, c, "work", 4, 4);
    }
    let s = s.replace(
        "r = A.run();",
        "fork f { r = A.run(); q = A.run() }; join all f;",
    );
    let v = check(&s);
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(v["verdict"], "inconclusive");
}

#[test]
fn a_shifted_cuts_clock_exemption_uses_relative_deadline() {
    let s = RELATIVE
        .replace("duration fixed 2min", "duration {1min:0.5,3min:0.5}")
        .replace("clock 20min", "clock 7min")
        .replace("within 2min >= 1", "within 2min >= 0.5");
    let v = check(&s);
    new_wire(&v, &[5, 6]);
    assert_eq!(v["bounds"]["truncated"], true);
    assert_eq!(v["verdict"], "inconclusive");
    probability(&v, 1, 1, "1/2", "1/2", "satisfied");
    probability(&v, 1, 2, "1/2", "1", "satisfied");
    let v = check(&s.replace("within 2min", "within 1min"));
    for c in 1..=2 {
        probability(&v, 1, c, "1/2", "1/2", "satisfied");
    }
}

#[test]
fn a_unarrived_cut_is_unknown_not_a_placeholder_hit_or_deadline_failure() {
    let s = RELATIVE
        .replace("[5min, 6min]", "[0min, 8min]")
        .replace("clock 20min", "clock 7min")
        .replace("hit: bool = false", "hit: bool = true")
        .replace("within 2min", "within 0min");
    let v = check(&s);
    new_wire(&v, &[0, 8]);
    assert_eq!(v["bounds"]["truncated"], true);
    probability(&v, 1, 1, "1", "1", "satisfied");
    probability(&v, 1, 2, "0", "1", "inconclusive");
    assert_eq!(v["case_outcomes"][1]["terminals"], json!([]));
    assert_eq!(v["admission"]["rejected_count"], json!({"min":0,"max":1}));
}

#[test]
fn a_completed_before_cut_and_known_reject_are_absorbing_per_case() {
    let s = RELATIVE
        .replace("[5min, 6min]", "[0min, 3min]")
        .replace("capacity 2", "capacity 1")
        .replace("clock 20min", "clock 4min");
    let v = check(&s);
    new_wire(&v, &[0, 3]);
    assert_eq!(v["bounds"]["truncated"], true);
    probability(&v, 1, 1, "1", "1", "satisfied");
    probability(&v, 1, 2, "0", "1", "inconclusive");
    for k in ["completed_at", "elapsed", "work"] {
        metric(&v, 1, k, 2, 2);
    }
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["probability"],
        json!({"min":"1","max":"1"})
    );
    assert_eq!(v["case_outcomes"][1]["terminals"], json!([]));
    let s = ZERO
        .replace("[0min, 1min]", "[0min, 5min]")
        .replace("clock 20min", "clock 4min");
    let v = check(&s);
    new_wire(&v, &[0, 5]);
    probability(&v, 1, 1, "0", "0", "satisfied");
    probability(&v, 1, 2, "0", "1", "satisfied");
    assert_eq!(
        v["case_outcomes"][0]["terminals"][0]["probability"],
        json!({"min":"1","max":"1"})
    );
    assert_eq!(v["admission"]["rejected_count"], json!({"min":1,"max":2}));
}

#[test]
fn a_ever_started_survives_task_replacement_and_intermediate_queueing() {
    // A: [0,1),[1,2),[3,4); B: [1,4),[4,7),[7,10).
    // At t3 case2 waits for B but is no longer an admission waiter.
    let s = r#"model EverStarted version "0.2" {
      type T = enum { ok }
      actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      actor B { capacity 1; capability run() -> T { duration fixed 3min; result nondet { ok } } }
      process Flow { a = A.run(); b = B.run(); end done }
      arrivals fixed [0min, 1min, 3min]
      policy fifo tie_by case_id
      queue capacity 1 on_overflow reject
      bounds { cases 3; live_tasks 1; clock 12min }
      verify safety no_resource_overcommit
      verify reachable end.done
    }"#;
    legacy_grammar_control(s);
    let v = check(s);
    new_wire(&v, &[0, 1, 3]);
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(v["admission"]["rejected_count"], json!({"min":0,"max":0}));
    for (case, t, e) in [(1, 4, 4), (2, 7, 6), (3, 10, 7)] {
        metric(&v, case, "completed_at", t, t);
        metric(&v, case, "elapsed", e, e);
        metric(&v, case, "work", 4, 4);
    }
}

#[test]
fn a_same_case_tie_retains_nondeterminism_even_with_fifo() {
    let s = r#"model SameCaseTie version "0.2" {
      type T = enum { ok }
      observed case { leftDone: bool = false }
      actor A { capacity 1
        capability short() -> T { duration fixed 1min; result nondet { ok }; writes leftDone; finish { leftDone = true } }
        capability long() -> T { duration fixed 3min; result nondet { ok } }
      }
      process Flow { fork f { l = A.short(); r = A.long() }; join all f; end done }
      policy fifo tie_by case_id
      bounds { cases 1; live_tasks 2; clock 10min }
      verify probability reached leftDone == true within 1min <= 1
    }"#;
    legacy_grammar_control(s);
    let v = check(s);
    new_wire(&v, &[0]);
    probability(&v, 1, 1, "0", "1", "satisfied");
    metric(&v, 1, "completed_at", 4, 4);
    metric(&v, 1, "work", 4, 4);
    assert_eq!(
        check(SIMULTANEOUS)["properties"][1]["cases"][0]["probability"],
        json!({"min":"1/2","max":"1/2"})
    );
}

#[test]
fn a_person_global_is_not_cloned_per_case_or_per_actor() {
    let s = r#"model PersonGlobal version "0.2" {
      type T = enum { ok }
      people [p]
      actor A { executors [p]; capability run() -> T { commits; duration fixed 1min; result nondet { ok } } }
      actor B { executors [p]; capability run() -> T { commits; duration fixed 1min; result nondet { ok } } }
      authority Grant { basis "test"; grants p to A.run; grants p to B.run }
      process Flow { fork f { a = A.run(); b = B.run() }; join all f; end done }
      arrivals fixed [0min, 0min]
      bounds { cases 2; live_tasks 2; clock 10min }
      verify safety no_resource_overcommit
      contract C { permitted A.run; permitted B.run; authority_basis A.run Grant; authority_basis B.run Grant }
    }"#;
    legacy_grammar_control(s);
    let v = check(s);
    new_wire(&v, &[0, 0]);
    assert_eq!(v["verdict"], "satisfied");
    for c in 1..=2 {
        metric(&v, c, "completed_at", 2, 4);
        metric(&v, c, "work", 2, 2);
    }
    for p in v["properties"].as_array().unwrap().iter().skip(1) {
        assert_eq!(p["verdict"], "satisfied");
        for c in p["cases"].as_array().unwrap() {
            assert_eq!(c["verdict"], "satisfied");
        }
    }
    // Permission is a checked contract, never a dispatch filter.
    let denied = s.replace("people [p]", "people [p,q]").replace(
        "      authority Grant",
        "      permit q to A.run\n      authority Grant",
    );
    legacy_grammar_control(&denied);
    let v = check(&denied);
    assert_eq!(row(&v, "contract C #1 permitted")["verdict"], "violated");
    for c in 1..=2 {
        metric(&v, c, "work", 2, 2);
    }
}

#[test]
fn a_separation_case_public_positive_and_local_violation_rows() {
    let s = r#"model SeparationCase version "0.2" {
      type T = enum { ok }
      people [p,q]
      actor A { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      actor B { executors [q]; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      process Flow { a = A.run(); b = B.run(); end done }
      arrivals fixed [0min, 0min]
      policy fifo tie_by case_id
      bounds { cases 2; live_tasks 1; clock 10min }
      contract C { separation A.run.finish, B.run.finish }
    }"#;
    legacy_grammar_control(s);
    for (source, expected) in [
        (s.to_owned(), "satisfied"),
        (s.replace("executors [q]", "executors [p]"), "violated"),
    ] {
        let v = check(&source);
        new_wire(&v, &[0, 0]);
        let p = row(&v, "contract C #1 separation");
        assert_eq!(p["verdict"], expected);
        for c in p["cases"].as_array().unwrap() {
            assert_eq!(c["verdict"], expected);
        }
    }
    // The crossed p→q / q→p direct-trace oracle still belongs to internal tests.
}

#[test]
fn a_requires_case_never_uses_another_cases_predecessor() {
    // Each independent receipt chooses A-only or B-only. A B-only case has no
    // local predecessor even on paths where the other case finished A first.
    let s = r#"model RequiresCase version "0.2" {
      type Route = enum { left, right }
      type T = enum { ok }
      actor Choose { capacity 2; capability route() -> Route { duration fixed 1min; result nondet { left, right } } }
      actor A { capacity 2; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      actor B { capacity 2; capability run() -> T { duration fixed 1min; result nondet { ok } } }
      process Flow { c = Choose.route(); if c == left { a = A.run(); end done } else { b = B.run(); end done } }
      arrivals fixed [0min, 3min]
      bounds { cases 2; live_tasks 1; clock 10min }
      contract C { requires_event B.run.start after A.run.finish }
    }"#;
    legacy_grammar_control(s);
    let v = check(s);
    new_wire(&v, &[0, 3]);
    let p = row(&v, "contract C #1 requires_event");
    assert_eq!(p["verdict"], "violated");
    for c in p["cases"].as_array().unwrap() {
        assert_eq!(c["verdict"], "violated");
    }
    let v = check(&s.replace("result nondet { left, right }", "result nondet { left }"));
    assert_eq!(
        row(&v, "contract C #1 requires_event")["verdict"],
        "vacuous"
    );
}

const CANCEL_READ: &str = r#"model CancelRead version "0.2" {
  type Result = enum { pass, ok }
  actor F { capacity 2; capability run() -> Result { duration fixed 1min; result nondet { pass }; cancel forbidden_after_start } }
  actor S { capacity 1; capability run() -> Result { duration fixed 3min; result nondet { ok }; cancel forbidden_after_start } }
  actor H { capacity 2; capability use(x: Result) -> Result { duration fixed 1min; result nondet { ok } } }
  process Flow {
    fork f { fast = F.run(); slow = S.run() }
    join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected
    if slow == ok { h = H.use(slow); end done } else { end rejected }
  }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
}"#;

#[test]
fn a_static_cancel_read_one_case_proof_does_not_generalize_to_multiple_cases() {
    for clause in ["", "arrivals fixed [0min]"] {
        let s = CANCEL_READ.replace("  bounds", &format!("  {clause}\n  bounds"));
        assert_eq!(check(&s)["verdict"], "satisfied", "{s}");
    }
    let multi = CANCEL_READ
        .replace("cases 1", "cases 2")
        .replace("  bounds", "  arrivals fixed [0min, 0min]\n  bounds");
    failure(&multi, "U-SEM-0");
    // Isolate argument and invariant readers rather than allowing the first if
    // to hide them. All are the same U9 uncertainty, not a blanket cancel ban.
    let argument = multi.replace(
        "if slow == ok { h = H.use(slow); end done } else { end rejected }",
        "h = H.use(slow); end done; end rejected",
    );
    failure(&argument, "U-SEM-0");
    let invariant = argument
        .replace("H.use(slow)", "H.use()")
        .replace("use(x: Result)", "use()")
        .replace(
            "  bounds",
            "  contract C { invariant H.use.finish -> slow == ok }\n  bounds",
        );
    failure(&invariant, "U-SEM-0");
    let observed = multi
        .replace(
            "  actor F",
            "  observed case { slow: Result = ok }\n  actor F",
        )
        .replace("duration fixed 3min;", "writes slow; duration fixed 3min;");
    assert_eq!(
        check(&observed)["verdict"],
        "satisfied",
        "observed current value is always defined"
    );
}

#[test]
fn a_single_new_duration_does_not_gain_future_duration_lookahead() {
    let s = include_str!("../../../fixtures/duration-no-lookahead/model.relca")
        .replace("  bounds", "  arrivals fixed [0min]\n  bounds");
    let v = check(&s);
    new_wire(&v, &[0]);
    probability(&v, 1, 1, "1/2", "1/2", "satisfied");
}

#[test]
fn a_unused_shared_mutability_is_rejected_but_single_new_case_is_supported() {
    for declaration in [
        "actor Shared { latent truth: bool }",
        "actor Shared { latent truth: bool ~ { true:0.5, false:0.5 } }",
        "kernel K { * -> { 1: ok with truth = false } }\n actor Shared { latent truth: bool = true; capacity 1; capability mutate() -> T { duration fixed 1min; writes truth; outcomes kernel K } }",
    ] {
        let s = COUNT.replace("  actor A", &format!("  {declaration}\n  actor A"));
        failure(&s, "U-SEM-0");
        let one = s
            .replace("cases 2", "cases 1")
            .replace("[0min, 1min]", "[0min]");
        assert_eq!(
            check(&one)["verdict"],
            "satisfied",
            "U2 is multiple-case-only"
        );
    }
}

#[test]
fn a_existing_latent_cut_type_probability_and_writer_errors_remain_invalid() {
    let latent = RELATIVE
        .replace(
            "  observed case",
            "  latent case { secret: bool = true }\n  observed case",
        )
        .replace(
            "r = A.run(); end done",
            "r = A.run(); if secret == true { end hidden } else { end done }",
        );
    failure(&latent, "E-POL-LATENT");
    failure(
        &RELATIVE.replace("finish { hit = true }", "finish { hit = ok }"),
        "E-TYPE-0",
    );
    failure(&SIMULTANEOUS.replace("3min:0.5", "3min:0.4"), "E-PROB-0");
    failure(&SIMULTANEOUS.replace("1min:0.5", "0min:0.5"), "U-SEM-0");
    let writers = RELATIVE
        .replace(
            "r = A.run();",
            "fork f { r = A.run(); q = A.run() }; join all f;",
        )
        .replace("live_tasks 1", "live_tasks 2");
    failure(&writers, "U-SEM-0");
    let cut = include_str!("../../../fixtures/cut-declared.broken-excluded-call/model.relca")
        .replace("cases 1", "cases 2")
        .replace("  bounds", "  arrivals fixed [0min, 1min]\n  bounds");
    failure(&cut, "E-CUT-1");
    let missing_cancel =
        include_str!("../../../fixtures/any-drain.broken-missing-cancel/model.relca")
            .replace("cases 1", "cases 2")
            .replace("  bounds", "  arrivals fixed [0min, 1min]\n  bounds")
            .replace(
                "if a == pass { end done } else { end no_match }",
                "end done; end no_match",
            );
    failure(&missing_cancel, "E-CANCEL-0");
    let bad_contract = COUNT.replace(
        "  bounds",
        "  contract C { at_most_once A.run.start per case.id }\n  bounds",
    );
    failure(&bad_contract, "E-CON-0");
}

#[test]
fn a_diagnostic_matrix_new_clauses_stay_in_model_scope() {
    for (from, to) in [
        ("arrivals fixed [0min, 1min]", "arrivals renewal_every 1min"),
        ("policy fifo tie_by case_id", "policy random"),
        (
            "actor A {",
            "actor A { queue capacity 1 on_overflow reject;",
        ),
        (
            "process Flow {",
            "process Flow { queue capacity 1 on_overflow reject;",
        ),
        ("bounds {", "bounds { arrivals fixed [0min, 1min];"),
        (
            "verify reachable end.done",
            "verify probability cohort any hit >= 0.5",
        ),
        (
            "verify reachable end.done",
            "verify expected leadtime <= 2min",
        ),
        ("verify reachable end.done", "verify mean leadtime <= 2min"),
        ("verify reachable end.done", "simulate 100"),
        ("verify reachable end.done", "symmetry cases"),
    ] {
        failure(&COUNT.replacen(from, to, 1), "U-SYN-0");
    }
    for number in ["-1", "+1", "1.5"] {
        failure(
            &ZERO.replace("queue capacity 0", &format!("queue capacity {number}")),
            "E-SYN-0",
        );
    }
    for suffix in [
        "policy fifo",
        "queue capacity",
        "queue capacity 1",
        "arrivals fixed [0min,",
    ] {
        failure(&format!("model M version \"0.2\" {{ {suffix}"), "E-SYN-0");
    }
    for duplicate in ["cases 2;", "live_tasks 1;", "clock 20min;"] {
        let from = duplicate.trim_end_matches(';');
        failure(
            &COUNT.replacen(from, &format!("{duplicate} {from}"), 1),
            "U-SEM-0",
        );
    }
    failure(&COUNT.replace("live_tasks 1; ", ""), "U-SEM-0");
}

#[test]
fn a_new_words_remain_contextual_identifiers_and_newline_separators_work() {
    for word in [
        "arrivals",
        "policy",
        "fifo",
        "tie_by",
        "case_id",
        "queue",
        "on_overflow",
        "reject",
    ] {
        let source = COUNT
            .replace("actor A", &format!("actor {word}"))
            .replace("A.run()", &format!("{word}.run()"));
        assert_eq!(check(&source)["verdict"], "satisfied", "Actor {word}");
        let source = COUNT.replace("run", word);
        assert_eq!(check(&source)["verdict"], "satisfied", "Capability {word}");
        let source = COUNT.replace("r = A.run()", &format!("{word} = A.run()"));
        assert_eq!(check(&source)["verdict"], "satisfied", "binding {word}");
        let source = COUNT.replace("ok", word);
        assert_eq!(check(&source)["verdict"], "satisfied", "enum {word}");
    }
    let s = ZERO
        .replace("arrivals fixed", "arrivals\n fixed")
        .replace(
            "policy fifo tie_by case_id",
            "policy\n fifo\n tie_by\n case_id",
        )
        .replace(
            "queue capacity 0 on_overflow reject",
            "queue\n capacity\n 0\n on_overflow\n reject",
        );
    let v = check(&s);
    new_wire(&v, &[0, 1]);
    assert_eq!(v["verdict"], "satisfied");
}

#[test]
fn a_equal_arrivals_clock_boundary_and_relative_u64_overflow_are_not_static_errors() {
    assert_eq!(
        check(&COUNT.replace("[0min, 1min]", "[0min, 0min]"))["verdict"],
        "satisfied"
    );
    let at_clock = COUNT
        .replace("[0min, 1min]", "[18min, 18min]")
        .replace("capacity 1", "capacity 2");
    let v = check(&at_clock);
    assert_eq!(v["verdict"], "satisfied");
    for c in 1..=2 {
        metric(&v, c, "completed_at", 20, 20);
    }
    // No arrival+within addition is needed for the query. Process has no Task,
    // so the test does not create a legitimate duration arithmetic overflow.
    let s = RELATIVE
        .replace("r = A.run(); end done", "end done")
        .replace(
            "[5min, 6min]",
            "[18446744073709551614min, 18446744073709551615min]",
        )
        .replace("clock 20min", "clock 18446744073709551615min")
        .replace("within 2min >= 1", "within 5min <= 1");
    let v = check(&s);
    new_wire(&v, &[u64::MAX - 1, u64::MAX]);
    assert_eq!(v["verdict"], "satisfied");
    for c in 1..=2 {
        probability(&v, 1, c, "0", "0", "satisfied");
        metric(&v, c, "elapsed", 0, 0);
    }
}

#[test]
fn a_wire_new_runtime_has_all_ids_and_case_probabilities_but_no_cohort() {
    let v = check(SIMULTANEOUS);
    new_wire(&v, &[0, 0]);
    assumptions(SIMULTANEOUS, &v, &[0, 0]);
    for p in v["properties"].as_array().unwrap().iter().skip(1) {
        assert!(p.get("probability").is_none());
        for c in p["cases"].as_array().unwrap() {
            assert!(c.get("probability").is_some());
        }
    }
    let plain = COUNT
        .replace("  arrivals fixed [0min, 1min]\n", "")
        .replace("  policy fifo tie_by case_id\n", "")
        .replace("cases 2", "cases 1");
    let v = check(&plain);
    assert_eq!(ledger_shape::original_field_count(&v), 13);
    assert_eq!(v["terminals"].as_array().unwrap().len(), 1);
    assert!(v.get("case_outcomes").is_none());
    assert!(v.get("admission").is_none());
}

#[test]
fn a_agency_is_static_template_derivation_plus_exactly_one_unchecked_marker() {
    for base in [
        "simultaneous",
        "staggered",
        "reject-mass",
        "same-time-slot",
        "initial-product",
        "relative-within",
        "count",
        "order",
        "required",
        "shared-fixed",
        "contracts",
        "zero-queue",
    ] {
        let source = fixture_source(&format!("arrivals-{base}"));
        let legacy = check(&legacy_source(&source));
        let multi = check(&source);
        let mut agency = multi["agency"].clone();
        let unchecked = agency["unchecked"].as_array_mut().unwrap();
        // ADR 0043 (plan C04): the no-people marker follows the arrivals marker
        // in both reports; remove only the arrivals marker in front of it.
        let people = (unchecked.last().unwrap() == "responsibility_people_undeclared")
            .then(|| unchecked.pop().unwrap());
        assert_eq!(unchecked.pop().unwrap(), "multi_case_interference");
        unchecked.extend(people);
        assert!(!unchecked.iter().any(|x| x == "multi_case_interference"));
        assert_eq!(
            agency, legacy["agency"],
            "{base}: no new Agency guarantee from arrivals"
        );
    }
}
