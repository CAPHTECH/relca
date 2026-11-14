//! Boundary review of ADR 0043 (c5, 20a2812): acceptance written before the
//! fixes. Expectations are hand-derived from the models below (comments), not
//! copied from the binary's output.
//!
//! - M1 (U13): a refusal / lineage_groups row that one side did not derive
//!   (the effect was skipped for state guards) is `not_comparable`, never a
//!   `changed` row to/from null. `null` stays only when both sides derived and
//!   one of them genuinely lacks the row.
//! - m1: keep fallback is still called on a dead arm / after an End (E-DIFF-0).
//! - m2: `reads [Doc PO]` keeps the pre-C5 E-SYN-0 bytes (`expected ]`).
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn temp(source: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let d = root().join("target/agency-second-half-boundary-inputs");
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join(format!(
        "{}-{}.relca",
        std::process::id(),
        N.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::write(&p, source).unwrap();
    p
}
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap()
}
fn json_of(o: &Output) -> Value {
    serde_json::from_slice(&o.stdout).unwrap()
}

/// The agy2-static-still-called model (Doc → enter/extract → approve → post)
/// with an observed case flag, and the To process supplied by each test.
fn model(to_process: &str) -> String {
    format!(
        r#"model AgencySecondHalf version "0.2" {{
  type Entry = enum {{ entry }}
  type V = enum {{ pass, fail }}
  type Ack = enum {{ committed }}
  people [c1, a1]
  observed case {{ flag: bool = false }}
  actor Doc {{ source document }}
  actor Clerk {{ executors [c1]; capability enter() -> Entry {{ reads [Doc]; duration fixed 1min; result nondet {{ entry }} }} }}
  actor Approver {{ executors [a1]; capability approve(e: Entry) -> V {{ duration fixed 1min; result nondet {{ pass, fail }} }} }}
  actor Ledger {{ capacity 1; capability post(v: V) -> Ack {{ commits; duration fixed 1min; result nondet {{ committed }} }} }}
  actor Extractor {{ capacity 1; capability extract() -> Entry {{ reads [Doc]; duration fixed 1min; result nondet {{ entry }} }} }}
  process Flow {{ e = Clerk.enter(); v = Approver.approve(e); if v == pass {{}} else {{ end returned }}; k = Ledger.post(v); end posted }}
  {to_process}
  configuration From {{ cut Scope; process Flow }}
  configuration To {{ cut Scope; process ToFlow; intent {{ retire Clerk.enter from normal_path keep fallback; add Extractor.extract reason "replace entry" }} }}
  cut Scope {{ include [Doc, Clerk, Extractor, Approver, Ledger] }}
  bounds {{ cases 1; live_tasks 3; clock 20min }}
  verify safety no_resource_overcommit
}}
"#
    )
}

/// agy2-diff-refusal-guard: the review's guard2 input. The observed case flag
/// guards the path to the effect k, so To skips k in `refusal` (state_guards)
/// and has no judgement in all_j, hence `lineage_groups: []`.
const GUARDED: &str = "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; if flag == true {} else { end returned }; k = Ledger.post(v); end posted }";
/// .broken-no-effect (control): To genuinely has no effect, both sides are
/// derived, so the From rows are `changed` to null.
const NO_EFFECT: &str = "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; end posted }";
/// Partially derived lineage: k is unguarded (same refusal row as From), a
/// second effect k2 with its own judgement w (subject Doc) is state-guarded.
const PARTIAL: &str = "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); w = Approver.approve(e); if flag == true { k2 = Ledger.post(w) } else {}; end posted }";

fn legacy_rows(from: &str, to: &str, process: &str) -> Value {
    let p = temp(&model(process));
    let o = run(&["diff", p.to_str().unwrap(), "--from", from, "--to", to]);
    let v = json_of(&o);
    let d = &v["agency"]["differences"];
    assert_eq!(d["status"], "derived", "{d}");
    Value::Array(
        d["rows"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|r| matches!(r["viewpoint"].as_str(), Some("refusal" | "lineage_groups")))
            .cloned()
            .collect(),
    )
}
fn derived() -> Value {
    json!({"status":"derived","reason":null})
}
fn guards() -> Value {
    json!({"status":"not_derived","reason":"state_guards"})
}
fn effect(viewpoint: &str, call: &str, from: Value, to: Value) -> Value {
    json!({"viewpoint":viewpoint,"item":"judgements","effect":"Ledger.post","call":call,
           "status":"not_comparable","from":from,"to":to})
}
fn doc(item: &str, from: Value, to: Value) -> Value {
    json!({"viewpoint":"lineage_groups","item":item,"subject_sources":["Doc"],
           "status":"not_comparable","from":from,"to":to})
}

#[test]
fn m1_refusal_and_lineage_of_a_state_guarded_effect_are_not_comparable() {
    // From: refusal k = [v all_paths 0]; lineage_groups [Doc: 1, 1, []].
    // To: k skipped (state_guards), lineage_groups incomplete.
    let want = json!([
        effect("refusal", "k", derived(), guards()),
        doc("judgements", derived(), guards()),
        doc("actors", derived(), guards()),
        doc("independent_sources", derived(), guards()),
    ]);
    assert_eq!(legacy_rows("From", "To", GUARDED), want);
}

#[test]
fn m1_reverse_direction_is_symmetric() {
    let want = json!([
        effect("refusal", "k", guards(), derived()),
        doc("judgements", guards(), derived()),
        doc("actors", guards(), derived()),
        doc("independent_sources", guards(), derived()),
    ]);
    assert_eq!(legacy_rows("To", "From", GUARDED), want);
}

#[test]
fn m1_control_genuinely_absent_effect_stays_changed_to_null() {
    let changed = |viewpoint: &str, item: &str, key: (&str, Value), from: Value| {
        let mut r =
            json!({"viewpoint":viewpoint,"item":item,"status":"changed","from":from,"to":null});
        match key.0 {
            "effect" => {
                r["effect"] = json!("Ledger.post");
                r["call"] = key.1;
            }
            _ => r["subject_sources"] = key.1,
        }
        r
    };
    let want = json!([
        changed(
            "refusal",
            "judgements",
            ("effect", json!("k")),
            json!([{"call":"v","capability":"Approver.approve","stops":"all_paths","bypasses":0}])
        ),
        changed(
            "lineage_groups",
            "judgements",
            ("subject", json!(["Doc"])),
            json!(1)
        ),
        changed(
            "lineage_groups",
            "actors",
            ("subject", json!(["Doc"])),
            json!(1)
        ),
        changed(
            "lineage_groups",
            "independent_sources",
            ("subject", json!(["Doc"])),
            json!([])
        ),
    ]);
    assert_eq!(legacy_rows("From", "To", NO_EFFECT), want);
}

#[test]
fn m1_partially_derived_lineage_is_not_reported_as_no_difference() {
    // To: refusal k equals From's row (no row). k2 is skipped: From has no k2
    // (derived absence), To did not derive it. Lineage on To counts only v
    // (w serves only the skipped k2), so its equal-looking Doc group is not
    // a derived value.
    let want = json!([
        effect("refusal", "k2", derived(), guards()),
        doc("judgements", derived(), guards()),
        doc("actors", derived(), guards()),
        doc("independent_sources", derived(), guards()),
    ]);
    assert_eq!(legacy_rows("From", "To", PARTIAL), want);
}

/// m1: the lexical still-called check, beyond the direct call form.
fn still_called(process: &str) {
    let p = temp(&model(process));
    let p = p.to_str().unwrap();
    for args in [
        vec!["check", p, "--configuration", "To"],
        vec!["diff", p, "--from", "From", "--to", "To"],
    ] {
        let o = run(&args);
        assert_eq!(o.status.code(), Some(3), "{args:?}");
        let stderr = String::from_utf8(o.stderr).unwrap();
        assert!(
            stderr.contains("E-DIFF-0: kept fallback is still called on the normal path"),
            "{args:?}: {stderr}"
        );
        assert!(!stderr.contains("W-AGY-3"), "{args:?}: {stderr}");
    }
}

#[test]
fn m1_still_called_on_a_dead_arm_is_rejected() {
    // Extractor.extract has the single result entry: the else arm is dead.
    still_called(
        "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if e == entry {} else { x = Clerk.enter(); end returned }; if v == pass {} else { end returned }; k = Ledger.post(v); end posted }",
    );
}

#[test]
fn m1_still_called_after_an_end_is_rejected() {
    still_called(
        "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); end posted; x = Clerk.enter() }",
    );
    still_called(
        "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass { k = Ledger.post(v); end posted } else { end returned }; x = Clerk.enter() }",
    );
}

#[test]
fn m1_control_not_called_keep_fallback_is_accepted() {
    let p = temp(&model(
        "process ToFlow { e = Extractor.extract(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); end posted }",
    ));
    let o = run(&["check", p.to_str().unwrap(), "--configuration", "To"]);
    assert_eq!(o.status.code(), Some(0));
}

#[test]
fn m2_unseparated_reads_keep_the_pre_c5_syntax_diagnostic_bytes() {
    // Pre-C5 parser: ident, then `,` or `]` (names("]")); at PO it says
    // "expected ]". The diagnostic span is the PO token.
    for reads in ["reads [Doc PO]", "reads [Doc", "reads [Doc, PO X]"] {
        let source = model(NO_EFFECT).replacen("reads [Doc]", reads, 1);
        let p = temp(&source);
        let o = run(&["check", p.to_str().unwrap()]);
        assert_eq!(o.status.code(), Some(3), "{reads}");
        assert_eq!(
            String::from_utf8(o.stderr.clone()).unwrap(),
            "E-SYN-0: expected ]\n",
            "{reads}"
        );
        let v = json_of(&o);
        // The offending token: PO, the `;` after "[Doc", and X.
        let (start, len) = match reads {
            "reads [Doc PO]" => (source.find("Doc PO").unwrap() + 4, 2),
            "reads [Doc" => (source.find(reads).unwrap() + reads.len(), 1),
            _ => (source.find("PO X").unwrap() + 3, 1),
        };
        let token = (start, start + len);
        assert_eq!(
            v["diagnostics"][0],
            json!({"code":"E-SYN-0","severity":"error","message":"expected ]",
                   "span":{"start":token.0,"end":token.1}}),
            "{reads}"
        );
    }
}
