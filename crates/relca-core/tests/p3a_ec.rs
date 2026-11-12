//! One-order integration, not a whole-output oracle or a performance gate.
#[path = "support/p3a_hand.rs"]
mod h;
use relca_core::{
    compare::{self, Analysis},
    diff, explain, report,
    sensitivity::{self, input::Mode},
};
use serde_json::json;
const EC: &str = include_str!("support/p3a_ec.relca");
#[test]
fn r1_ec_exhaustive_compare_preserves_violations_unknown_and_terminal_extrema() {
    let v = json!(compare::compare(
        EC,
        &["Keyed", "Plain"],
        Analysis::Exhaustive
    ));
    h::verdict(&v, "violated");
    for c in h::children(&v, true) {
        h::verdict(c, "violated");
        assert_eq!(
            h::row(&c["properties"], "id", "receipt_resolution")["verdict"],
            "inconclusive"
        );
    }
    for (end, ka, kb, plain) in [
        ("out_of_stock", 2, 2, 2),
        ("shipped", 7, 8, 7),
        ("stock_refunded", 5, 6, 5),
        ("refund_rejected", 46, 47, 46),
        ("cancel_refunded", 47, 48, 47),
    ] {
        let row = h::row(&v["tables"]["terminals"], "end", end);
        assert_eq!(row["values"][0]["value"]["elapsed"], h::range(ka, kb));
        assert_eq!(row["values"][1]["value"]["elapsed"], h::range(plain, plain));
    }
    let u = h::row(&v["tables"]["terminals"], "kind", "receipt_unknown");
    for cell in h::rows(&u["values"]) {
        h::observed(&cell["value"], 4, 4);
    }
    let q = h::row(&v["tables"]["properties"], "id", "probability reached #1");
    assert_eq!(q["values"][0]["probability"], json!({"min":"0","max":"0"}));
    assert_eq!(q["values"][1]["probability"], json!({"min":"0","max":"1"}));
    assert!(
        v["tables"].get("agency").is_none(),
        "no implicit objectives or Agency adoption decision"
    );
}
#[test]
fn r1_ec_diff_keeps_original_capability_replacement_and_real_counterexample() {
    let v = json!(diff::diff(EC, "Keyed", "Plain"));
    h::verdict(&v, "violated");
    let rs = h::rows(&v["differences"]);
    for (target, change) in [
        ("Psp.charge_keyed", "removed"),
        ("Psp.charge_plain", "added"),
    ] {
        assert!(
            rs.iter()
                .any(|r| r["kind"] == "relation" && r["target"] == target && r["change"] == change)
        );
    }
    assert!(!rs.iter().any(|r| r["kind"] == "actor"));
    let s = EC
        .replace(
            "configuration Keyed { cut OrderCut; process KeyedFlow }",
            "",
        )
        .replace(
            "configuration Plain { cut OrderCut; process PlainFlow }",
            "",
        );
    // Explain requires an unambiguous process, not a new two-file CLI.
    let at = s.find("  process KeyedFlow").unwrap();
    let end = s.find("  process PlainFlow").unwrap();
    let s = format!("{}{}", &s[..at], &s[end..]);
    let e = json!(explain::explain(&s));
    let ce = h::row(
        &e["explanation"]["counterexamples"],
        "id",
        "contract PayControls #2 at_most_once",
    );
    assert_eq!(ce["status"], "verified");
    for attempt in [1, 2] {
        assert!(h::rows(&ce["steps"]).iter().any(|s| {
            s["command"] == "pay"
                && s["attempt"] == attempt
                && (s["kind"] == "commit"
                    || s["events"]
                        .as_array()
                        .is_some_and(|es| es.iter().any(|e| e["kind"] == "commit")))
        }));
    }
}
#[test]
fn r1_ec_simulation_still_refuses_unresolved_nondeterminism() {
    let v = json!(compare::compare(
        EC,
        &["Keyed", "Plain"],
        Analysis::Simulation
    ));
    h::verdict(&v, "unsupported");
    for c in h::children(&v, true) {
        h::verdict(c, "unsupported");
        assert!(
            h::rows(&c["diagnostics"])
                .iter()
                .any(|d| d["code"] == "U-SEM-0")
        );
    }
}
#[test]
fn r4_ec_responsibility_is_not_deleted_to_pass_the_sensitivity_gate() {
    // Use the real EC point twice, keeping business nondeterminism, responsibility,
    // Authority and every opaque defer. The Q7 target remains charges == 2.
    let d = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../.tmp")
        .join(format!("p3a-ec-suite-{}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    std::fs::write(d.join("ec.relca"), EC).unwrap();
    let manifest = d.join("manifest.json");
    std::fs::write(&manifest,json!({"schema":"relca-sensitivity/v1","scope":{"kind":"enumerated_models","rationale":"two explicit identical EC assumptions"},"points":[{"id":"a","model":"ec.relca","configuration":"Keyed","scenario":null},{"id":"b","model":"ec.relca","configuration":"Keyed","scenario":null}],"targets":[{"kind":"probability_query","id":"probability reached #1"}],"confidence":null}).to_string()).unwrap();
    let v = json!(sensitivity::run::run(&manifest, Mode::Exhaustive));
    h::verdict(&v, "violated");
    for p in h::rows(&v["points"]) {
        assert_eq!(p["status"], "evaluated");
        assert!(!h::rows(&p["report"]["responsibilities"]).is_empty());
    }
    // A normal selected check still sees the declared duty.
    assert!(
        !report::check_configuration(EC, "Keyed")
            .responsibilities
            .unwrap()
            .is_empty()
    );
}
