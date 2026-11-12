//! Records with sets, invariants, limits and Standing; obligation
//! R2: in a process condition, a latent case variable and a record binding with
//! the same name. The legacy scalar twin (`e == x`) reads the latent and is
//! E-POL-LATENT `policy cannot read latent state` at the name, in every process
//! (policy conditions are checked before configuration selection), exit 3. The
//! record field read `e.f` takes the same side. Expected values are derived from
//! that scalar rule before the fix, never copied from output.
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn scratch(s: &str) -> PathBuf {
    static N: AtomicUsize = AtomicUsize::new(0);
    let dir = root()
        .join("target")
        .join(format!("record-integration-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(format!("{}.relca", N.fetch_add(1, Ordering::Relaxed)));
    std::fs::write(&p, s).unwrap();
    p
}

fn check(src: &str, configuration: Option<&str>) -> (i32, Value) {
    let path = scratch(src);
    let mut args = vec!["check".to_string(), path.to_str().unwrap().to_string()];
    if let Some(c) = configuration {
        args.push("--configuration".into());
        args.push(c.into());
    }
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(&args)
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout)
            .unwrap_or_else(|e| panic!("{e}: {}", String::from_utf8_lossy(&o.stdout))),
    )
}

/// Error diagnostics as (code, message, span start, span end).
fn errors(v: &Value) -> Vec<(String, String, u64, u64)> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
                d["span"]["start"].as_u64().unwrap(),
                d["span"]["end"].as_u64().unwrap(),
            )
        })
        .collect()
}

fn latent_at(s: &str, needle: &str, nth: usize) -> (String, String, u64, u64) {
    let start = s
        .match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("{needle}"))
        .0 as u64;
    (
        "E-POL-LATENT".into(),
        "policy cannot read latent state".into(),
        start,
        start + 1,
    )
}

fn changed(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "{from}");
    s.replace(from, to)
}

/// Record boundary probe (plat.relca).
const PLAT: &str = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = record { f: bool }
  latent case { e: bool = true }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { R { f = true } } }
  }
  process P { e = A.r(); if e.f == true { end done } else { end no } }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}
"#;

/// The scalar twin (plat-s.relca): R is an enum and the read is `e == x`.
const PLAT_SCALAR: &str = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = enum { x }
  latent case { e: bool = true }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { x } }
  }
  process P { e = A.r(); if e == x { end done } else { end no } }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}
"#;

fn rejected(code: i32, v: &Value) {
    assert_eq!(code, 3, "{v}");
    assert_eq!(v["verdict"], "invalid", "{v}");
    assert_eq!(v["properties"], json!([]), "{v}");
    assert_eq!(v["terminals"], json!([]), "{v}");
}

/// Control: the scalar twin already reads the latent (E-POL-LATENT at `e`, then
/// the member check of `x` against bool).
#[test]
fn r2_scalar_twin_reads_the_latent() {
    let (code, v) = check(PLAT_SCALAR, None);
    rejected(code, &v);
    let e = errors(&v);
    assert_eq!(e.len(), 2, "{v}");
    assert_eq!(e[0], latent_at(PLAT_SCALAR, "e == x", 0), "{v}");
    assert_eq!(
        (e[1].0.as_str(), e[1].1.as_str()),
        ("E-TYPE-0", "x is not a member of bool")
    );
}

/// The record field read takes the latent side: E-POL-LATENT at `e`, exit 3.
/// No other error: the read is refused before any field of the latent is
/// looked up (bool has no field f, and that is not a second finding).
#[test]
fn r2_record_field_read_of_a_latent_named_like_the_binding_is_e_pol_latent() {
    let (code, v) = check(PLAT, None);
    rejected(code, &v);
    assert_eq!(errors(&v), vec![latent_at(PLAT, "e.f == true", 0)], "{v}");
}

/// Each atom of a chain, and an atom inside a group, is checked (as the
/// scalar policy check recurses into And/Or/Group).
#[test]
fn r2_every_atom_of_a_chain_and_a_group_is_checked() {
    let chain = changed(PLAT, "if e.f == true {", "if e.f == true || e.f == false {");
    let (code, v) = check(&chain, None);
    rejected(code, &v);
    assert_eq!(
        errors(&v),
        vec![
            latent_at(&chain, "e.f == true", 0),
            latent_at(&chain, "e.f == false", 0)
        ],
        "{v}"
    );
    let group = changed(PLAT, "if e.f == true {", "if (e.f == true) {");
    let (code, v) = check(&group, None);
    rejected(code, &v);
    assert_eq!(errors(&v), vec![latent_at(&group, "e.f == true", 0)], "{v}");
}

/// The scalar policy check covers every process, selected or not: an
/// unselected process that reads `e.f` with the latent `e` and its own record
/// binding `e` is refused as well, and so is its scalar twin.
#[test]
fn r2_unselected_process_is_checked_like_the_scalar_twin() {
    for (src, read) in [(PLAT, "e.f == true"), (PLAT_SCALAR, "e == x")] {
        let configured = changed(
            src,
            "  bounds {",
            "  process Q { g = A.r(); end done }\n  cut Scope { include [A] }\n  configuration One { cut Scope; process P }\n  configuration Two { cut Scope; process Q }\n  bounds {",
        );
        let (code, v) = check(&configured, Some("Two"));
        rejected(code, &v);
        let e = errors(&v);
        assert_eq!(e[0], latent_at(&configured, read, 0), "{read}: {v}");
        if read == "e.f == true" {
            assert_eq!(e.len(), 1, "{v}");
        }
    }
}

/// Control: without the latent, the binding is read (one result R { f = true },
/// so the process ends done) and the model is satisfied.
#[test]
fn r2_control_without_the_latent_reads_the_binding() {
    let src = changed(PLAT, "  latent case { e: bool = true }\n", "");
    let (code, v) = check(&src, None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(errors(&v), vec![], "{v}");
}

// ------------------------------------------------- deferred ADR 0037 explain

fn explain(src: &str) -> (i32, Value) {
    let path = scratch(src);
    let o = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(["explain", path.to_str().unwrap()])
        .output()
        .unwrap();
    (
        o.status.code().unwrap(),
        serde_json::from_slice(&o.stdout).unwrap(),
    )
}

const STATE: &str = r#"model Records version "0.2" {
type Ack = enum { committed }
type Entry = record { confidence: enum { high, low } }
observed case { entry: Entry = Entry { confidence = DEFAULT } }
actor Work { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { committed }; writes entry; finish { entry = Entry { confidence = high } } } }
process Flow { r = Work.run(); end done }
contract C { invariant entry.confidence == high }
bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
}
"#;

/// Configuration-free explain of the record state invariant. Default low is
/// violated at the receipt: the shortest witness is the initial stage alone
/// (time 0, no transition), the state-invariant detail names the canonical
/// Entry value `Entry { confidence = low }` (ADR 0038 canonical member), and
/// the independent replay verifies it. Default high has no counterexample.
#[test]
fn deferred_explain_initial_state_witness_with_canonical_record_value() {
    let (code, v) = explain(&changed(STATE, "DEFAULT", "low"));
    assert_eq!(code, 1, "{v}");
    let c = &v["explanation"]["counterexamples"][0];
    assert_eq!(c["id"], "contract C #1 invariant", "{v}");
    assert_eq!(c["status"], "verified", "{v}");
    assert_eq!(c["transition_count"], 0, "{v}");
    let step = &c["steps"][0];
    assert_eq!(
        (step["kind"].clone(), step["time"].clone()),
        (json!("initial"), json!(0))
    );
    let detail = &step["violation"]["state_invariant"];
    assert_eq!(detail["antecedent"], true, "{v}");
    assert_eq!(detail["condition"], false, "{v}");
    let values: Vec<_> = detail["values"]
        .as_array()
        .unwrap()
        .iter()
        .map(|x| {
            (
                x["name"].clone(),
                x["value"].clone(),
                x["visibility"].clone(),
            )
        })
        .collect();
    assert_eq!(
        values,
        vec![(
            json!("entry"),
            json!("Entry { confidence = low }"),
            json!("observed")
        )],
        "{v}"
    );
    let (code, v) = explain(&changed(STATE, "DEFAULT", "high"));
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["explanation"]["counterexamples"], json!([]), "{v}");
}

/// The record kernel result keeps the explain source mapping of a state
/// invariant violated by a kernel assignment. The scalar twin (Entry as an
/// enum { high, low }) maps the violation at the finish (time 1) to the
/// invariant and the `flagged = true` assignment and verifies it; the record
/// form must do the same, not fall back to source_mapping_failed.
#[test]
fn deferred_explain_maps_a_record_kernel_result_like_the_scalar_twin() {
    let record = r#"model Records version "0.2" {
type Difficulty = enum { routine, difficult }
type Entry = record { confidence: enum { high, low } }
observed case { entry: optional<Entry>; flagged: bool = false }
actor Invoice { source document; latent difficulty: Difficulty ~ { routine: 0.5, difficult: 0.5 } }
actor Extractor { capacity 1; capability extract() -> Entry { duration fixed 1min; outcomes kernel ai_entry; kernel_reads latent [Invoice.difficulty]; writes [entry, flagged] } }
kernel ai_entry given [Invoice.difficulty] { routine -> { 1: Entry { confidence = high } with flagged = false }; difficult -> { 1: Entry { confidence = low } with flagged = true } }
process Flow { e = Extractor.extract(); end done }
contract C { invariant flagged == false }
bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
}
"#;
    let scalar = record
        .replace(
            "type Entry = record { confidence: enum { high, low } }",
            "type Entry = enum { high, low }",
        )
        .replace("Entry { confidence = high }", "high")
        .replace("Entry { confidence = low }", "low");
    for src in [scalar.as_str(), record] {
        let (code, v) = explain(src);
        assert_eq!(code, 1, "{v}");
        let c = &v["explanation"]["counterexamples"][0];
        assert_eq!(c["status"], "verified", "{v}");
        assert_eq!(c["reason"], Value::Null, "{v}");
        let texts: Vec<_> = c["related_locations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                let a = l["span"]["start"].as_u64().unwrap() as usize;
                let b = l["span"]["end"].as_u64().unwrap() as usize;
                src[a..b].to_string()
            })
            .collect();
        assert_eq!(texts, vec!["invariant flagged == false", "flagged = true"]);
        let last = c["steps"].as_array().unwrap().last().unwrap();
        assert_eq!(
            (last["kind"].clone(), last["time"].clone()),
            (json!("finish"), json!(1))
        );
    }
}

// --------------------------------------- same class as R2 (ADR 0035 sets)

/// A record literal written into an ADR 0035 case set must not be read
/// through its head as a CaseId parameter that happens to share the record
/// type's name. The control inserts the parameter itself (prime 1min, post
/// 1min: done 2 / work 2). The literal takes the record-value type check of
/// any other non-record target: E-TYPE-0 `record value has different type` at
/// the head, exit 3.
#[test]
fn same_class_record_literal_into_a_case_set_is_a_type_error() {
    let src = r#"model Records version "0.2" {
type Ack = enum { committed, rejected }
type K = record { f: bool }
actor Ledger { capacity 1; state keys: set<CaseId>; capability prime(K: CaseId) -> Ack { writes keys; finish { keys += VALUE }; duration fixed 1min; result nondet { committed } } }
actor Poster { capacity 1; capability post(key: CaseId) -> Ack { requires key in Ledger.keys else rejected; duration fixed 1min; result nondet { committed } } }
process Flow { p = Ledger.prime(case.id); a = Poster.post(case.id); end done }
cut C { include [Ledger, Poster] }
bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
}
"#;
    let (code, v) = check(&changed(src, "VALUE", "K"), None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let s = changed(src, "VALUE", "K { f = true }");
    let (code, v) = check(&s, None);
    rejected(code, &v);
    let at = s.find("K { f = true }").unwrap() as u64;
    assert_eq!(
        errors(&v),
        vec![(
            "E-TYPE-0".to_string(),
            "record value has different type".to_string(),
            at,
            at + 1
        )],
        "{v}"
    );
}

/// Boundary review after the integration, m1: `none` written into an optional
/// record state inside atomic. ADR 0038 keeps record state writes out of
/// atomic (U-SEM-0 `atomic record write is unsupported`); the public IR check
/// (engine and reference) refuses it without a span. Lower must refuse it
/// first, at the assignment target, so the diagnostic carries a position:
/// exit 4, no property rows. The control writes the same `none` with `finish`
/// outside atomic and is accepted (g: 1min, done).
const ATOMIC_NONE: &str = r#"model Records version "0.2" {
type Ack = enum { committed, refused }
type Entry = record { confidence: enum { high, low } }
observed case { entry: optional<Entry>; allowed: bool = true }
actor W { capacity 1
  capability g() -> Ack { duration fixed 1min; writes entry; ITEM }
}
process P { a = W.g(); end done }
bounds { cases 1; live_tasks 1; clock 5min }
verify safety no_resource_overcommit
}
"#;

#[test]
fn m1_atomic_none_into_an_optional_record_is_refused_by_lower_with_a_span() {
    let s = changed(
        ATOMIC_NONE,
        "ITEM",
        "atomic { if allowed == true { entry = none; result committed } else { result refused } }",
    );
    let (code, v) = check(&s, None);
    assert_eq!(code, 4, "{v}");
    assert_eq!(v["verdict"], "unsupported", "{v}");
    assert_eq!(v["properties"], json!([]), "{v}");
    assert_eq!(v["terminals"], json!([]), "{v}");
    let at = s.find("entry = none").unwrap() as u64;
    assert_eq!(
        errors(&v),
        vec![(
            "U-SEM-0".to_string(),
            "atomic record write is unsupported".to_string(),
            at,
            at + "entry".len() as u64
        )],
        "{v}"
    );
}

#[test]
fn m1_control_none_into_an_optional_record_by_finish_is_accepted() {
    let s = changed(
        ATOMIC_NONE,
        "ITEM",
        "result nondet { committed }; finish { entry = none }",
    );
    let (code, v) = check(&s, None);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(errors(&v), vec![], "{v}");
}
