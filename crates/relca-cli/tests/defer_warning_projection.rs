//! Hand witnesses for validating additions before legacy diagnostic projection.
#[allow(dead_code)]
#[path = "support/defer_effects_inputs.rs"]
mod hand;
use relca_test_support::fixture_match::legacy_defer_diagnostics;
use serde_json::{Value, json};

fn warning(source: &str, target: &str) -> Value {
    let start = source.find(&format!("defer {target} reason")).unwrap() + "defer ".len();
    json!({"code":"W-CUT-2","severity":"warning","message":hand::warning(target),"span":{"start":start,"end":start+target.len()}})
}

#[test]
fn source_warning_oracle_matches_the_closed_hand_table_before_removing_only_w_cut_2() {
    for k in 0..=7 {
        let source = hand::source(k);
        let targets: &[&str] = match k {
            1 | 7 => &["Ghost"],
            2 => &["A.nosuch"],
            _ => &[],
        };
        let old = json!({"code":"W-UNREACH-0","severity":"warning","message":"old row retained literally","span":null});
        let mut rows = vec![old.clone()];
        rows.extend(targets.iter().map(|t| warning(&source, t)));
        let report = json!({"diagnostics":rows});
        assert_eq!(
            legacy_defer_diagnostics(&report, &source, None).unwrap(),
            vec![old]
        );
    }
    let source = hand::selected_cuts();
    legacy_defer_diagnostics(&json!({"diagnostics":[]}), &source, Some("Clean")).unwrap();
    legacy_defer_diagnostics(
        &json!({"diagnostics":[warning(&source, "Ghost")]}),
        &source,
        Some("Hidden"),
    )
    .unwrap();
    let source = hand::source(1)
        .replace("include [A,R];", "question \"前提\"; include [A,R];")
        .replace("defer Ghost reason \"later\" loses \"load\";", "defer Ghost reason \"later\" loses \"load\"; defer Ghost reason \"later\" loses \"load\";");
    let mut rows = Vec::new();
    for (at, _) in source.match_indices("defer Ghost reason") {
        let mut row = warning(&source, "Ghost");
        row["span"] = json!({"start":at+6,"end":at+11});
        rows.push(row);
    }
    assert_eq!(rows.len(), 2);
    legacy_defer_diagnostics(&json!({"diagnostics":rows}), &source, None).unwrap();
}

#[test]
fn legacy_projection_rejects_extra_missing_renamed_and_wrong_span_warning_rows() {
    let source = hand::source(1);
    let good = json!({"diagnostics":[warning(&source, "Ghost")]});
    legacy_defer_diagnostics(&good, &source, None).unwrap();
    let mut extra = good.clone();
    extra["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(warning(&source, "Ghost"));
    let missing = json!({"diagnostics":[]});
    let mut renamed = good.clone();
    renamed["diagnostics"][0]["message"] = json!(hand::warning("Other"));
    let mut wrong_span = good.clone();
    wrong_span["diagnostics"][0]["span"]["start"] = json!(0);
    for (label, bad) in [
        ("extra", extra),
        ("missing", missing),
        ("renamed", renamed),
        ("span", wrong_span),
    ] {
        let error = legacy_defer_diagnostics(&bad, &source, None).unwrap_err();
        assert!(
            error.contains("source-bound W-CUT-2 mismatch"),
            "{label}: {error}"
        );
        eprintln!("calibration {label}: rejected {error}");
    }
    // Zero expected rows is also a strict obligation, not a blanket filter.
    for source in [hand::source(0), hand::source(6), hand::selected_cuts()] {
        let selection = source.contains("configuration Clean").then_some("Clean");
        assert!(legacy_defer_diagnostics(&good, &source, selection).is_err());
    }
}

#[test]
fn real_source_bound_legacy_comparator_accepts_opaque_and_zero_warning_controls() {
    for name in [
        "agy-invoice",
        "agy-invoice.broken-ai-transfer",
        "agy-o-people",
    ] {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let source =
            std::fs::read_to_string(root.join(format!("fixtures/{name}/model.relca"))).unwrap();
        let selection = name.starts_with("agy-invoice").then_some("AsIs");
        let report = json!(relca_core::report::check_with_selection(
            &source, selection, None
        ));
        let old = legacy_defer_diagnostics(&report, &source, selection).unwrap();
        assert!(!old.iter().any(|d| d["code"] == "W-CUT-2"));
    }
}
