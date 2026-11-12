//! Migrate the vocabulary of hand expectations, never restore historical bytes.
//! ADR 0043 supplies the rules/unchecked words; ADR 0051 supplies the invoice
//! entry producer pair in five inputs. All numeric hand expectations remain intact.
//! ADR 0054 supplies only the selected-IR responsibility diagnostic inventory.
use serde_json::{Value, json};
#[path = "../../../relca-core/tests/support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;

fn k4_report_pointer(name: &str) -> Option<&'static str> {
    if [
        "explain-deadlock",
        "explain-deadlock.broken-zero-capacity",
        "explain-executors",
        "explain-executors.broken-shared-worker",
        "explain-invariant",
        "explain-invariant.broken-cut-guard",
        "explain-once",
        "explain-once.broken-repeat-result",
        "explain-requires",
        "explain-requires.broken-cut-guard",
        "explain-response",
        "explain-response.broken-late-finish",
        "cf-explain",
        "cf-explain.broken-overlap",
        "rec-explain",
        "rec-explain.broken-bypass",
    ]
    .contains(&name)
    {
        Some("")
    } else if matches!(
        name,
        "mut2-summary-judgement" | "mut2-summary-judgement.broken-no-guard"
    ) {
        Some("/original")
    } else {
        None
    }
}

pub fn assert_k4(name: &str, source: &str, actual: &Value) {
    if let Some(pointer) = k4_report_pointer(name) {
        let model = relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap()).unwrap();
        responsibility_oracle::assert_report(&model, actual.pointer(pointer).unwrap());
    }
}

pub fn diff_state(source: &str, from: &str, to: &str) -> (bool, bool) {
    diff_state_with_scenario(source, from, to, None)
}

pub fn diff_state_with_scenario(
    source: &str,
    from: &str,
    to: &str,
    scenario: Option<&str>,
) -> (bool, bool) {
    let Ok(ast) = relca_core::syntax::parse(source) else {
        return (true, false);
    };
    match (
        relca_core::lower::lower_configuration_with_scenario(&ast, from, scenario),
        relca_core::lower::lower_configuration_with_scenario(&ast, to, scenario),
    ) {
        (Ok(a), Ok(b)) => (
            false,
            a.model.cut.as_ref().map(|c| (&c.name, &c.horizon))
                != b.model.cut.as_ref().map(|c| (&c.name, &c.horizon)),
        ),
        _ => (true, false),
    }
}

pub fn check_diff_marker(
    actual: &Value,
    null_agency: bool,
    cut_mismatch: bool,
) -> Result<(), String> {
    // ADR 0043 Agency differences: either null Agency makes the difference
    // unavailable. ADR 0008 "比較不能(comparison mismatch)" and v0.2 §6.2 stop
    // comparison for different Cuts. Current code also marks that explicit IR
    // mismatch unavailable; see the KC-fix adjudication recorded in ADR 0055.
    let cut_mismatch =
        if cut_mismatch && std::env::var("RELCA_EXPECTATIONS_FAULT").as_deref() == Ok("cut") {
            eprintln!("fault applied expected Cut mismatch");
            false
        } else {
            cut_mismatch
        };
    let required = null_agency || cut_mismatch;
    let count = actual["unchecked"]
        .as_array()
        .ok_or("diff unchecked array")?
        .iter()
        .filter(|w| *w == "agency_differences")
        .count();
    if count == usize::from(required) {
        Ok(())
    } else {
        Err("independent null-Agency/Cut-mismatch marker".into())
    }
}

pub fn migrate(name: &str, source: &str, expected: &mut Value) {
    migrate_with_scenario(name, source, expected, None)
}

pub fn migrate_with_scenario(
    name: &str,
    source: &str,
    expected: &mut Value,
    scenario: Option<&str>,
) {
    let people_declared = relca_core::syntax::parse(source).is_ok_and(|m| {
        m.items
            .iter()
            .any(|i| matches!(i, relca_core::ast::Item::People(_)))
    });
    fn walk(v: &mut Value, people: bool) {
        match v {
            Value::Object(o) => {
                if o.get("rules") == Some(&json!("agency-viewpoints-p1-1")) {
                    o.insert("rules".into(), json!("agency-viewpoints-p1-2"));
                }
                if matches!(
                    o.get("rules").and_then(Value::as_str),
                    Some("agency-p0-1" | "agency-p1-state-1")
                ) && let Some(words) = o.get_mut("unchecked").and_then(Value::as_array_mut)
                    && words.iter().any(|w| w == "executors")
                {
                    words.retain(|w| w != "executors");
                    if !people {
                        let at = words
                            .iter()
                            .position(|w| w == "obligation_debtor")
                            .unwrap_or(words.len());
                        words.insert(at, json!("responsibility_people_undeclared"));
                    }
                }
                for child in o.values_mut() {
                    walk(child, people);
                }
            }
            Value::Array(a) => a.iter_mut().for_each(|x| walk(x, people)),
            _ => {}
        }
    }
    walk(expected, people_declared);
    // ADR 0054 "範囲" / "意味": these closed generic diagnostic arrays
    // are updated from selected IR, never copied from CLI output or snapshots.
    if let Some(pointer) = k4_report_pointer(name) {
        let model = relca_core::lower::lower(&relca_core::syntax::parse(source).unwrap()).unwrap();
        let report = expected.pointer_mut(pointer).unwrap();
        *report = responsibility_oracle::with_diagnostics(&model, report);
    }
    if expected.get("from").is_some() && expected.get("to").is_some() {
        let (null, mismatch) = diff_state_with_scenario(
            source,
            expected["from"]["configuration"].as_str().unwrap_or(""),
            expected["to"]["configuration"].as_str().unwrap_or(""),
            scenario,
        );
        if let Some(words) = expected.get_mut("unchecked").and_then(Value::as_array_mut) {
            if !null && !mismatch && !words.iter().any(|w| w == "relation_state_producers") {
                words.extend([
                    json!("relation_state_producers"),
                    json!("relation_binding_identity"),
                ]);
            }
            if (null || mismatch) && !words.iter().any(|w| w == "agency_differences") {
                words.push(json!("agency_differences"));
            }
        }
    }
    // Both invoice flows consume entry. The selected input producer changes
    // from human entry to extraction. This pair follows from the DSL, not a
    // saved output or an allowlist of byte positions (ADR 0051).
    if [
        "prob-invoice.broken-ai-transfer",
        "diff2-invoice",
        "diff2-invoice.broken-missing-resource-claim",
        "invoice-phase0-ai",
        "invoice-phase0-ai.broken-unguarded-approval",
    ]
    .contains(&name)
    {
        expected["differences"].as_array_mut().unwrap().push(json!({
            "kind":"relation", "target":"entry", "change":"modified",
            "from":{"producer":"Clerk.enter"}, "to":{"producer":"Extractor.extract"},
            "reason":null, "loses":null
        }));
        if std::env::var("RELCA_EXPECTATIONS_FAULT").as_deref() == Ok("invoice") {
            eprintln!("fault applied expected invoice producer");
            expected["differences"]
                .as_array_mut()
                .unwrap()
                .last_mut()
                .unwrap()["to"]["producer"] = json!("wrong.producer");
        }
    }
}

#[test]
fn diff_migration_uses_null_agency_and_explicit_ir_cut_mismatch() {
    let source = r#"model D version "0.2" {
      type R=enum{ok}
      actor A {capacity 1;capability run()->R{duration fixed 1min;result nondet{ok}}}
      process P {a=A.run();end done}
      cut K{include[A]} cut L{include[A]}
      configuration From{cut K;process P} configuration To{cut L;process P}
      bounds{cases 1;live_tasks 1;clock 5min} verify reachable end.done
    }"#;
    let make = || json!({"from":{"configuration":"From"},"to":{"configuration":"To"},"unchecked":[],"terminals":[{"elapsed":{"min":1,"max":1}}]});
    let mut expected = make();
    migrate("hand-diff", source, &mut expected);
    let raw = json!(relca_core::diff::diff(source, "From", "To"));
    assert_eq!(diff_state(source, "From", "To"), (false, true));
    check_diff_marker(&raw, false, true).unwrap();
    assert!(
        check_diff_marker(&raw, false, false).is_err(),
        "falsely matching IR Cuts must fail the actual marker comparator"
    );
    assert_eq!(expected["unchecked"], json!(["agency_differences"]));
    let same = source.replace("configuration To{cut L", "configuration To{cut K");
    let raw = json!(relca_core::diff::diff(&same, "From", "To"));
    assert_eq!(diff_state(&same, "From", "To"), (false, false));
    check_diff_marker(&raw, false, false).unwrap();
    let mut expected = make();
    migrate("hand-diff", &same, &mut expected);
    assert_eq!(
        expected["unchecked"],
        json!(["relation_state_producers", "relation_binding_identity"])
    );
    assert_eq!(
        expected["terminals"],
        json!([{"elapsed":{"min":1,"max":1}}])
    );
    let raw = json!(relca_core::diff::diff(source, "From", "Missing"));
    assert!(raw["agency"]["to"].is_null());
    check_diff_marker(&raw, true, false).unwrap();
}

#[test]
fn invoice_migration_adds_the_hand_producer_row_and_preserves_existing_rows() {
    let source = include_str!("../../../../fixtures/invoice-phase0-ai/model.relca");
    let mut expected = json!({"differences":[{"kind":"time","target":"Clerk.match_po","from":["sequential"],"to":["fork"]}],"terminals":[{"work":{"min":15,"max":15}}]});
    let old = expected.clone();
    migrate("invoice-phase0-ai", source, &mut expected);
    assert_eq!(expected["differences"][0], old["differences"][0]);
    assert_eq!(expected["terminals"], old["terminals"]);
    assert_eq!(
        expected["differences"][1],
        json!({"kind":"relation","target":"entry","change":"modified","from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"},"reason":null,"loses":null})
    );
}

#[test]
fn k4_inventory_migration_rejects_extra_and_missing_carriers() {
    let effect = r#"model M version "0.2" {
type R=enum {ok}
actor Poster {capacity 1;capability post()->R {commits;duration fixed 1min;result nondet {ok}}}
process Flow {x=Poster.post();end done}
bounds {cases 1;live_tasks 1;clock 2min}
verify safety no_resource_overcommit
}"#;
    let plain = effect.replace("commits;", "");
    let good = json!(relca_core::report::check(effect));
    assert_k4("explain-invariant", effect, &good);
    let mut hand = json!({"diagnostics":[],"bounds":{"cases":1}});
    migrate("explain-invariant", effect, &mut hand);
    assert_eq!(hand["bounds"], json!({"cases":1}));
    assert_eq!(
        hand["diagnostics"][0]["message"],
        "Poster has no declared responsibility"
    );
    let mut missing = good.clone();
    missing["diagnostics"]
        .as_array_mut()
        .unwrap()
        .retain(|d| d["code"] != "W-RESP-0");
    assert!(std::panic::catch_unwind(|| assert_k4("explain-invariant", effect, &missing)).is_err());
    let mut spurious = json!(relca_core::report::check(&plain));
    assert_k4("explain-invariant", &plain, &spurious);
    spurious["diagnostics"]
        .as_array_mut()
        .unwrap()
        .push(hand["diagnostics"][0].clone());
    assert!(
        std::panic::catch_unwind(|| assert_k4("explain-invariant", &plain, &spurious)).is_err()
    );
}

#[test]
fn vocabulary_migration_preserves_hand_numbers_and_does_not_read_actual_output() {
    let mut hand = json!({"terminals":[{"elapsed":{"min":3,"max":5}}],
        "agency":{"rules":"agency-p0-1","unchecked":["via_model","executors"]}});
    migrate("hand", "model M version \"0.2\" {}", &mut hand);
    assert_eq!(hand["terminals"], json!([{"elapsed":{"min":3,"max":5}}]));
    assert_eq!(
        hand["agency"]["unchecked"],
        json!(["via_model", "responsibility_people_undeclared"])
    );
}
