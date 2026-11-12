#[path = "support/read_write_oracle.rs"]
mod h;
use relca_core::{engine, reference, report};
use serde_json::{Value, json};

#[test]
fn hand_relation_matches_existing_ir_substrate() {
    for (name, src, expected) in h::cases() {
        let rows = h::rows(&h::model(&src));
        let actual: Vec<_> = rows.iter().map(|r| r["value"].as_str().unwrap()).collect();
        assert_eq!(actual, expected, "{name}");
    }
    let ast = relca_core::syntax::parse(&h::configuration_source()).unwrap();
    for name in ["Left", "Right"] {
        let m = relca_core::lower::lower_with_selection(&ast, Some(name), None).unwrap();
        assert_eq!(h::rows(&m), h::rows(&h::model(h::BASE)));
        assert!(m.simulation.is_some());
    }
}

fn accept(name: &str) {
    let (_, src, expected) = h::cases().into_iter().find(|(n, _, _)| *n == name).unwrap();
    let v = json!(report::check(&src));
    let expected: Vec<_> = expected.iter().map(String::as_str).collect();
    h::assert_rows(&src, &v, &expected);
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|d| d["code"] != "U-SEM-0"),
        "{name}: candidates are not refusals"
    );
}

macro_rules! cases { ($($test:ident => $name:literal),+ $(,)?) => { $(
    #[test] fn $test() { accept($name); }
)+ }; }
cases! {
    requires => "requires", broken_unwritten => "broken-unwritten",
    repeated_read => "repeated-read", atomic => "atomic",
    atomic_broken_unwritten => "atomic-broken-unwritten",
    both_capability_conditions => "both-capability-conditions",
    process_if => "process-if", process_if_broken_unwritten => "process-if-broken-unwritten",
    utf8_byte_position => "utf8-byte-position", unused_reader => "unused-reader",
    self_pair => "self-pair", start_writer => "start-writer",
    variable_order => "variable-order", single_case => "single-case",
    case_local => "case-local", per_case => "per-case", evaluation_only => "evaluation-only",
    integer_comparison => "integer-comparison", serialized_actor => "serialized-actor",
    serialized_resource => "serialized-resource",
    two_if_sites => "two-if-sites", writer_declaration_order => "writer-declaration-order",
}

#[test]
fn candidate_rows_reach_explain_simulate_diff_and_compare_reports() {
    let src = h::surface_source();
    let configured = h::configuration_source();
    let vs = [
        json!(relca_core::explain::explain(&src)),
        json!(report::simulate(&src)),
        json!(relca_core::diff::diff(&configured, "Left", "Right")),
        json!(relca_core::compare::compare(
            &configured,
            &["Right", "Left"],
            relca_core::compare::Analysis::Exhaustive
        )),
    ];
    let wanted = h::rows(&h::model(&src));
    for (i, v) in vs.iter().enumerate() {
        let pointers: &[&str] = match i {
            0 | 1 => &[""],
            2 => &["/from/report", "/to/report"],
            _ => &["/plans/0/report", "/plans/1/report"],
        };
        for p in pointers {
            h::assert_candidate_tail(v.pointer(p).unwrap(), &wanted);
        }
    }
}

#[test]
fn unwritten_pair_is_zero_candidates_and_existing_semantics_stay_fixed() {
    for src in [
        h::BASE.to_owned(),
        h::BASE.replacen("requires x", "requires y", 1),
    ] {
        let m = h::model(&src);
        let e = engine::explore(&m);
        let r = reference::explore(&m);
        assert_eq!(r.stop, reference::StopReason::Complete);
        assert_eq!(e.properties, r.outcome.properties);
        assert_eq!(e.case_outcomes, r.outcome.case_outcomes);
        let v = json!(report::check(&src));
        assert_eq!(v["verdict"], "violated");
        assert_eq!(v["bounds"]["truncated"], false);
        for case in v["case_outcomes"].as_array().unwrap() {
            let ends = case["terminals"].as_array().unwrap();
            assert_eq!(ends.len(), 1);
            assert_eq!(ends[0]["end"], "done");
            assert_eq!(ends[0]["elapsed"], json!({"min":2,"max":2}));
            assert_eq!(ends[0]["work"], json!({"min":2,"max":2}));
        }
        if src.contains("requires y") {
            assert!(h::candidate_rows(&v).is_empty());
        }
    }
}

#[test]
fn latent_and_unsupported_reads_remain_fail_closed() {
    let hidden = h::BASE
        .replace("state y: bool = false", "latent y: bool = false")
        .replacen("requires x", "requires A.y", 1);
    let v: Value = json!(report::check(&hidden));
    assert_eq!(v["verdict"], "invalid");
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-POL-LATENT")
    );
    assert!(h::candidate_rows(&v).is_empty());
    let explicit = h::BASE.replace("requires x == false else rejected", "reads A.x");
    let v = json!(report::check(&explicit));
    assert_eq!(v["verdict"], "unsupported");
    assert!(h::candidate_rows(&v).is_empty());
}
