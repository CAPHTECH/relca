//! Parent and child ledgers, selection order, mutation provenance (ADR 0058).
#[path = "support/ledger_harness.rs"]
mod h;
use h::{Spec, known, oracle};
use serde_json::{Value, json};

#[test]
fn green_legacy_diff_order_values_reasons_and_exit_are_hand_fixed() {
    let v = h::invoke("diff", h::PARALLEL, &["--from", "AsIs", "--to", "ToBeP"]);
    assert_eq!(v.exit, 0);
    assert_eq!(v.value["verdict"], "satisfied");
    assert_eq!(v.value["from"]["configuration"], "AsIs");
    assert_eq!(v.value["to"]["configuration"], "ToBeP");
    h::leaf_numbers(&v.value["from"]["report"], 8, 8);
    h::leaf_numbers(&v.value["to"]["report"], 5, 8);
    assert_eq!(
        v.value["differences"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["target"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["Office.long", "Office.short"]
    );
    assert_eq!(
        v.value["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["W-DIFF-1", "W-DIFF-1", "W-DIFF-3", "W-DIFF-3"]
    );
    assert!(v.stderr.starts_with("diff: W-DIFF-1:"));
}
#[test]
fn diff_root_ledger_connects_both_sides_and_has_no_first_side_seed() {
    let v = h::invoke("diff", h::PARALLEL, &["--from", "AsIs", "--to", "ToBeP"]);
    h::ledger(&v.value, &Spec::aggregate("diff"));
    assert_eq!(
        v.value["ledger"]["execution"]["seed"],
        oracle::na("aggregate_per_source")
    );
}
#[test]
fn diff_children_keep_owner_relative_context_and_distinct_processes() {
    let v = h::invoke("diff", h::PARALLEL, &["--from", "AsIs", "--to", "ToBeP"]);
    h::ledger(&v.value["from"]["report"], &h::selected("Serial", "AsIs"));
    h::ledger(&v.value["to"]["report"], &h::selected("Parallel", "ToBeP"));
    let single = h::invoke("check", h::PARALLEL, &["--configuration", "AsIs"]);
    assert_eq!(single.value["ledger"], v.value["from"]["report"]["ledger"]);
    let typed: relca_core::diff::DiffReport = serde_json::from_value(v.value.clone()).unwrap();
    assert_eq!(serde_json::to_value(typed).unwrap(), v.value);
}
#[test]
fn compare_exhaustive_root_table_cells_and_requested_plan_order() {
    let v = h::invoke(
        "compare",
        h::PARALLEL,
        &["--plans", "ToBeP,AsIs", "--mode", "exhaustive"],
    );
    assert_eq!(v.exit, 0);
    assert_eq!(v.value["plans"][0]["configuration"], "ToBeP");
    assert_eq!(v.value["plans"][1]["configuration"], "AsIs");
    h::ledger(&v.value, &Spec::aggregate("compare"));
    h::ledger(
        &v.value["plans"][0]["report"],
        &h::selected("Parallel", "ToBeP"),
    );
    h::ledger(
        &v.value["plans"][1]["report"],
        &h::selected("Serial", "AsIs"),
    );
    h::leaf_numbers(&v.value["plans"][0]["report"], 5, 8);
    h::leaf_numbers(&v.value["plans"][1]["report"], 8, 8);
}
#[test]
fn compare_simulation_children_keep_selected_seed_confidence_bounds_and_local_quality() {
    let source = include_str!("../../../fixtures/qual-compare-sim/model.relca");
    let v = h::invoke(
        "compare",
        source,
        &["--plans", "Check,Fast", "--mode", "simulation"],
    );
    assert_eq!(v.exit, 0); // reached has a witnessed path; this is not sampled safety
    for (n, config, process) in [(0, "Check", "Flow"), (1, "Fast", "Quick")] {
        let mut s = Spec::simulation(process, &["Checker", "Ledger"], 1, 4, "1/2");
        s.selection = oracle::selection(Some(config), None, process);
        let child = &v.value["plans"][n]["report"];
        assert_eq!(child["bounds"]["cases"], 2);
        assert_eq!(child["bounds"]["clock"], 30);
        h::ledger(child, &s);
    }
    h::ledger(&v.value, &Spec::aggregate("compare"));
}
#[test]
fn compare_argument_failure_empty_plans_and_diff_cut_mismatch_still_have_ledgers() {
    let v = h::cli(&["compare"]);
    assert_eq!(v.exit, 3);
    assert_eq!(v.value["plans"], json!([]));
    h::ledger(&v.value, &Spec::aggregate("compare"));
    let source = h::PARALLEL.replace(
        "configuration ToBeP { cut C;",
        "cut Other { include [Office] } configuration ToBeP { cut Other;",
    );
    let v = h::invoke("diff", &source, &["--from", "AsIs", "--to", "ToBeP"]);
    assert_eq!(v.exit, 3);
    assert!(!v.value["mismatch"].is_null());
    h::ledger(&v.value, &Spec::aggregate("diff"));
}
#[test]
fn mutation_root_original_and_every_generated_child_have_own_ledgers() {
    let v = h::invoke("mutate", h::MUTATE, &[]);
    assert_eq!(v.exit, 4);
    h::ledger(&v.value, &Spec::aggregate("mutation"));
    let original = &v.value["original"];
    h::ledger(original, &h::mutation_spec());
    let mut count = 0;
    for row in v.value["mutations"].as_array().unwrap() {
        for mutant in row["mutants"].as_array().unwrap() {
            if let Some(report) = mutant.get("report").filter(|r| !r.is_null()) {
                count += 1;
                h::ledger(report, &h::mutation_spec());
                assert_eq!(report["model"]["sha256"], original["model"]["sha256"]);
            }
            for field in ["assigned_changes", "other_changes"] {
                for delta in mutant[field].as_array().unwrap() {
                    assert!(!delta["field"].as_str().unwrap().contains("ledger"));
                }
            }
        }
    }
    assert!(count > 0, "must actually exercise generated reports");
}
#[test]
fn mutation_child_ledger_is_required_even_if_parent_is_not_checked_yet() {
    let v = h::invoke("mutate", h::MUTATE, &[]);
    let mut count = 0;
    for row in v.value["mutations"].as_array().unwrap() {
        for mutant in row["mutants"].as_array().unwrap() {
            if let Some(report) = mutant.get("report").filter(|r| !r.is_null()) {
                count += 1;
                h::ledger(report, &h::mutation_spec());
            }
        }
    }
    assert!(count > 0);
}
#[test]
fn mutation_blocked_missing_original_and_no_candidate_run_do_not_fabricate_children() {
    let v = h::cli(&["mutate"]);
    assert_eq!(v.exit, 3);
    assert!(v.value["original"].is_null());
    h::ledger(&v.value, &Spec::aggregate("mutation"));
    let source = include_str!("../../../fixtures/mut-empty/model.relca");
    let v = h::invoke("mutate", source, &[]);
    assert_eq!(v.exit, 4);
    h::ledger(&v.value["original"], &Spec::finite("Flow", &[]));
    h::ledger(&v.value, &Spec::aggregate("mutation"));
}
#[test]
fn core_aggregate_apis_include_ledgers_without_cli_postprocessing() {
    let diff = serde_json::to_value(relca_core::diff::diff(h::PARALLEL, "AsIs", "ToBeP")).unwrap();
    h::ledger(&diff, &Spec::aggregate("diff"));
    let compare = serde_json::to_value(relca_core::compare::compare(
        h::PARALLEL,
        &["AsIs", "ToBeP"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    h::ledger(&compare, &Spec::aggregate("compare"));
    let mutation = serde_json::to_value(relca_core::mutation::mutate(h::MUTATE)).unwrap();
    h::ledger(&mutation, &Spec::aggregate("mutation"));
}
#[test]
fn rule_diff_children_do_not_merge_two_effective_versions_of_same_actor() {
    let v = h::invoke("diff", h::RULE, &["--from", "AsIs", "--to", "ToBeB"]);
    assert_eq!(v.exit, 1);
    h::ledger(&v.value["from"]["report"], &h::rule_spec("AsIs", "r1"));
    h::ledger(&v.value["to"]["report"], &h::rule_spec("ToBeB", "r2"));
    h::ledger(&v.value, &Spec::aggregate("diff"));
    assert_eq!(v.value["ledger"]["actor_versions"]["entries"], json!([]));
}
#[test]
fn production_decoder_rejects_existing_local_rule_version_disagreeing_with_ledger() {
    let mut v = serde_json::to_value(relca_core::report::check(h::SERIAL)).unwrap();
    let s = h::serial_spec();
    v["ledger"] = oracle::expected(&v, &s).unwrap();
    serde_json::from_value::<relca_core::report::Report>(v.clone())
        .expect("valid hand ledger control");
    assert!(
        !v["ledger"]["derivation"]["components"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    v["ledger"]["derivation"]["components"][0]["version"] = json!("agency-p1-state-1");
    assert!(
        serde_json::from_value::<relca_core::report::Report>(v).is_err(),
        "LOCAL_VERSION_MISMATCH accepted by production"
    );
}
#[test]
fn green_aggregate_wire_zeros_do_not_borrow_seed_or_actor_versions() {
    let report = json!({"mode":"compare","verdict":"unsupported","plans":[],"tables":{}});
    let e = oracle::expected(&report, &Spec::aggregate("compare")).unwrap();
    assert_eq!(e["execution"]["seed"], oracle::na("aggregate_per_source"));
    assert_eq!(e["actor_versions"]["entries"], json!([]));
    assert_ne!(e["execution"]["seed"], known(0));
    assert_ne!(e["execution"]["bounds"], Value::Null);
}
