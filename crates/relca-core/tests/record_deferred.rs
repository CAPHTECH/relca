//! Records with sets, invariants, limits and Standing.
//! Every expected value is derived by hand from the model
//! text (minutes) and the ADR rules, or from the legacy scalar twin at the same
//! position, before any run; never copied from output. One correction to the
//! deferred list is recorded at r37_antecedent_record_read: an antecedent that
//! never holds is vacuous (ADR 0037 「畳み方」), not satisfied.
use relca_core::{engine, ir::*, lower, reference, report, syntax};
use serde_json::{Value, json};

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn check_in(s: &str, configuration: &str) -> Value {
    serde_json::to_value(report::check_configuration(s, configuration)).unwrap()
}

fn model(s: &str) -> Model {
    lower::lower(&syntax::parse(s).unwrap_or_else(|e| panic!("{e:?}\n{s}")))
        .unwrap_or_else(|e| panic!("{e:?}\n{s}"))
}

fn changed(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "{from}");
    s.replace(from, to)
}

/// Error diagnostics as (code, message, span start).
fn errors(v: &Value) -> Vec<(String, String, Option<u64>)> {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .map(|d| {
            (
                d["code"].as_str().unwrap().to_owned(),
                d["message"].as_str().unwrap().to_owned(),
                d["span"]["start"].as_u64(),
            )
        })
        .collect()
}

fn err(code: &str, message: &str, s: &str, needle: &str) -> (String, String, Option<u64>) {
    let start = s.find(needle).unwrap_or_else(|| panic!("{needle}")) as u64;
    (code.into(), message.into(), Some(start))
}

fn rejected(v: &Value, verdict: &str) {
    assert_eq!(v["verdict"], verdict, "{v}");
    assert_eq!(v["properties"], json!([]), "{v}");
    assert_eq!(v["terminals"], json!([]), "{v}");
}

fn verdict(v: &Value, id: &str) -> Value {
    v["properties"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == id)
        .unwrap_or_else(|| panic!("missing {id}: {v}"))["verdict"]
        .clone()
}

fn r(n: u64) -> Value {
    json!({"min": n, "max": n})
}

/// Engine and the independent reference agree on the whole runtime Outcome,
/// on the legacy graph and on the one-arrival engine.
fn both(src: &str) -> Value {
    for arrivals in [false, true] {
        let mut m = model(src);
        if arrivals {
            m.arrivals = Some(vec![0]);
        }
        let e = engine::explore(&m);
        let mut r = reference::explore(&m);
        assert_eq!(
            r.stop,
            reference::StopReason::Complete,
            "no silent reference exclusion"
        );
        assert!(r.outcome.diagnostics.starts_with(&m.diagnostics));
        r.outcome.diagnostics.drain(..m.diagnostics.len());
        assert_eq!(e, r.outcome, "engine vs reference, arrivals={arrivals}");
    }
    check(src)
}

const INV: &str = "contract C #1 invariant";

// ---------------------------------------------------------------- ADR 0037

/// rec-state-invariant: Entry has confidence high/low; the observed entry
/// defaults to DEFAULT; Work.run (1min) writes the whole Entry(high) on finish.
/// The case ends at 1 (elapsed 1, work 1).
const STATE: &str = r#"model Records version "0.2" {
type Verdict = enum { pass, fail }
type Ack = enum { committed }
type Entry = record { confidence: enum { high, low } }
observed case { entry: Entry = Entry { confidence = DEFAULT }; approval: optional<Verdict>; registered: bool = false }
actor Work { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { committed }; writes entry; finish { entry = Entry { confidence = high } } } }
process Flow { r = Work.run(); end done }
contract C { INVARIANT }
bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
}
"#;

fn state(default: &str, invariant: &str) -> String {
    changed(&changed(STATE, "DEFAULT", default), "INVARIANT", invariant)
}

fn done_at_one(v: &Value) {
    assert_eq!(
        v["terminals"],
        json!([{"end": "done", "elapsed": r(1), "work": r(1), "result_at": null}]),
        "{v}"
    );
}

/// `invariant entry.confidence == high`: default high is satisfied; default
/// low is violated at the receipt (time 0) and the later whole write of high
/// at 1 does not erase it (sticky, ADR 0037). done 1 / work 1 in both.
#[test]
fn r37_unconditional_record_field_invariant() {
    let v = both(&state("high", "invariant entry.confidence == high"));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(verdict(&v, INV), "satisfied");
    done_at_one(&v);
    let v = both(&state("low", "invariant entry.confidence == high"));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(verdict(&v, INV), "violated");
    done_at_one(&v);
}

/// Antecedent record read: `entry.confidence == low -> approval == pass`
/// (approval stays none). Default high: the antecedent never holds. The
/// deferred list wrote "satisfied"; ADR 0037 「畳み方」 folds an invariant
/// that is never active to vacuous (exit 5), and the pre-A4 binary gives
/// vacuous for the scalar twin (`conf == low -> approval == pass`). Default
/// low: the antecedent holds at 0 with approval none, violated.
#[test]
fn r37_antecedent_record_read() {
    let inv = "invariant entry.confidence == low -> approval == pass";
    let v = both(&state("high", inv));
    assert_eq!(v["verdict"], "vacuous", "{v}");
    assert_eq!(verdict(&v, INV), "vacuous");
    done_at_one(&v);
    let v = both(&state("low", inv));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(verdict(&v, INV), "violated");
    done_at_one(&v);
}

/// Consequent record read, checked separately: `registered == false ->
/// entry.confidence == high` (registered stays false, so always active).
/// Default high: satisfied. Default low: violated at 0.
#[test]
fn r37_consequent_record_read() {
    let inv = "invariant registered == false -> entry.confidence == high";
    let v = both(&state("high", inv));
    assert_eq!(v["verdict"], "satisfied", "{v}");
    done_at_one(&v);
    let v = both(&state("low", inv));
    assert_eq!(v["verdict"], "violated", "{v}");
    assert_eq!(verdict(&v, INV), "violated");
    done_at_one(&v);
}

/// Replace only the case variable by an available binding `e` of type Entry:
/// the scalar twin is U-SEM-0 `state invariant reads a process binding` at the
/// name (lower, before any backend), not an E-NAME fallback. The record read
/// takes the same class at the same position, in both clause positions.
#[test]
fn r37_binding_record_read_is_the_scalar_binding_refusal() {
    let with_binding = changed(
        &changed(
            STATE,
            "process Flow { r = Work.run(); end done }",
            "actor Ex { capacity 1; capability extract() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } } }\nprocess Flow { e = Ex.extract(); r = Work.run(); end done }",
        ),
        "DEFAULT",
        "high",
    );
    for inv in [
        "invariant e.confidence == high",
        "invariant registered == false -> e.confidence == high",
        "invariant e.confidence == low -> approval == pass",
    ] {
        let s = changed(&with_binding, "INVARIANT", inv);
        let v = check(&s);
        rejected(&v, "unsupported");
        assert_eq!(
            errors(&v),
            vec![err(
                "U-SEM-0",
                "state invariant reads a process binding",
                &s,
                "e.confidence"
            )],
            "{inv}: {v}"
        );
    }
}

/// A latent and a record binding with the same name, read by a state
/// invariant: the scalar twin (state_invariant_name) is U-SEM-0 `invariant
/// name is both a binding and a latent variable` at the name.
#[test]
fn r37_latent_named_like_a_record_binding_is_the_scalar_guard() {
    let s = changed(
        &changed(
            &changed(
                STATE,
                "process Flow { r = Work.run(); end done }",
                "latent case { e: bool = true }\nactor Ex { capacity 1; capability extract() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } } }\nprocess Flow { e = Ex.extract(); r = Work.run(); end done }",
            ),
            "DEFAULT",
            "high",
        ),
        "INVARIANT",
        "invariant e.confidence == high",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![err(
            "U-SEM-0",
            "invariant name is both a binding and a latent variable",
            &s,
            "e.confidence"
        )],
        "{v}"
    );
}

/// The same-name alias `entry = A.x()` in an unselected process must still
/// be a result writer when a state invariant reads entry (ADR 0037: state
/// clauses extend alias validation to nonselected processes; sinv-alias is the
/// scalar pair). Good (selected by X) writes the result into entry and ends at
/// 1 (done 1 / work 1, Other is never called). Removing only Other's `writes
/// entry` rejects even X, for a record read in the antecedent and in the
/// consequent, and whether Other's call sits under a guard or not.
const ALIAS: &str = r#"model Records version "0.2" {
type Entry = record { confidence: enum { high, low } }
observed case { entry: Entry = Entry { confidence = high }; registered: bool = false }
actor A { capacity 1
  capability good() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } }; writes entry }
  capability other() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } }; writes entry }
}
process Good { entry = A.good(); end done }
process Other { OTHER }
cut K { include [A] }
configuration X { cut K; process Good }
configuration Y { cut K; process Other }
bounds { cases 1; live_tasks 1; clock 20min }
contract C { INVARIANT }
}
"#;

#[test]
fn r37_record_alias_in_an_unselected_process_must_be_a_result_writer() {
    for inv in [
        "invariant entry.confidence == high -> registered == false",
        "invariant registered == false -> entry.confidence == high",
    ] {
        for other in [
            "entry = A.other(); end done",
            "if registered == false { entry = A.other(); end done } else { end done }",
        ] {
            let s = changed(&changed(ALIAS, "INVARIANT", inv), "OTHER", other);
            let v = check_in(&s, "X");
            assert_eq!(v["verdict"], "satisfied", "{inv} / {other}: {v}");
            assert_eq!(
                v["terminals"],
                json!([{"end": "done", "elapsed": r(1), "work": r(1), "result_at": null}])
            );
            let broken = changed(
                &s,
                "capability other() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } }; writes entry }",
                "capability other() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } }",
            );
            let at = broken.find("entry = A.other()").unwrap() as u64;
            for configuration in ["X", "Y"] {
                let v = check_in(&broken, configuration);
                rejected(&v, "unsupported");
                assert_eq!(
                    errors(&v),
                    vec![(
                        "U-SEM-0".to_string(),
                        "observed binding alias requires a result writer".to_string(),
                        Some(at)
                    )],
                    "{inv} / {other} / {configuration}: {v}"
                );
            }
        }
    }
}

// ---------------------------------------------------------------- ADR 0035

const ATOMIC: &str = r#"model Records version "0.2" {
type Ack = enum { committed, refused }
type Entry = record { confidence: enum { high, low } }
observed case { allowed: bool = true; registered: bool = false }
actor Ex { capacity 1; capability extract() -> Entry { duration fixed 1min; ITEM } }
process Flow { e = Ex.extract(); end done }
bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
}
"#;

/// S29: the ordinary Entry result is the positive (done 1 / work 1). Adding
/// only an atomic block (with its declared write) to the record-return
/// Capability is U-SEM-0 (ADR 0038 「record と atomic の結果の併存」), reported
/// at the atomic item before the bare members high/low could be E-TYPE-0.
#[test]
fn s29_record_result_with_atomic_is_unsupported_before_member_errors() {
    let positive = changed(
        ATOMIC,
        "ITEM",
        "result nondet { Entry { confidence = high } }",
    );
    let v = both(&positive);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    done_at_one(&v);
    let s = changed(
        ATOMIC,
        "ITEM",
        "writes registered; atomic { if allowed == true { registered = true; result high } else { result low } }",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![err(
            "U-SEM-0",
            "atomic with record result is unsupported",
            &s,
            "atomic {"
        )],
        "{v}"
    );
    // Record literals as the atomic results stay outside the parser's atomic
    // form (fail closed, U-SYN-0), never a silent scalar reading.
    let s = changed(
        ATOMIC,
        "ITEM",
        "writes registered; atomic { if allowed == true { registered = true; result Entry { confidence = high } } else { result Entry { confidence = low } } }",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    assert!(errors(&v).iter().all(|e| e.0 == "U-SYN-0"), "{v}");
}

/// A distinct forbidden shape: a record constant written inside atomic with a
/// scalar result. The atomic assignment reads one scalar value and refuses a
/// following `{` (ADR 0035 parser: `atomic expression`, U-SYN-0).
#[test]
fn s29_record_constant_atomic_write_with_scalar_result_is_refused() {
    let s = changed(
        &changed(ATOMIC, "-> Entry {", "-> Ack {"),
        "ITEM",
        "writes registered; atomic { if allowed == true { registered = Entry { confidence = high }; result committed } else { result refused } }",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![err(
            "U-SYN-0",
            "unsupported syntax: atomic expression",
            &s,
            "{ confidence = high }; result committed"
        )],
        "{v}"
    );
}

/// Appending an unused record declaration (a forward declaration for the
/// record lookahead: it follows every use site) to ADR 0035 models keeps the
/// Contains / CaseId / executor.id / atomic / none classifications: the whole
/// check report is the same except the source hash, once a byte span that
/// reaches past the insertion point (the model's own span) is shifted back by
/// the inserted length.
#[test]
fn s29_unused_record_declaration_keeps_adr0035_classifications() {
    fn shift(v: &mut Value, at: u64, by: u64) {
        match v {
            Value::Object(o) => {
                if let Some(Value::Object(span)) = o.get_mut("span") {
                    for key in ["start", "end"] {
                        if let Some(n) = span.get(key).and_then(Value::as_u64)
                            && n > at
                        {
                            span.insert(key.into(), json!(n - by));
                        }
                    }
                }
                for (_, x) in o.iter_mut() {
                    shift(x, at, by);
                }
            }
            Value::Array(xs) => {
                for x in xs {
                    shift(x, at, by);
                }
            }
            _ => {}
        }
    }
    let mut names: Vec<_> = std::fs::read_dir(root().join("fixtures"))
        .unwrap()
        .map(|d| d.unwrap().file_name().into_string().unwrap())
        .filter(|n| n.starts_with("aset-") || n.starts_with("aobs-optional"))
        .collect();
    names.sort();
    assert!(names.len() > 100, "{}", names.len());
    let inserted = "type Zz = record { zz: bool }\n";
    for name in names {
        let path = root().join("fixtures").join(&name).join("model.relca");
        let s = std::fs::read_to_string(path).unwrap();
        let end = s.rfind('}').unwrap();
        let t = format!("{}{inserted}{}", &s[..end], &s[end..]);
        let mut a = check(&s);
        let mut b = check(&t);
        shift(&mut b, end as u64, inserted.len() as u64);
        for v in [&mut a, &mut b] {
            v["model"]["sha256"] = Value::Null;
        }
        assert_eq!(a, b, "{name}");
    }
}

/// A whole-record comparison is U-SYN-0 `record comparison is unsupported`
/// even with a forward record declaration, newlines inside the literal and a
/// nested scope; an ordinary enum member comparison followed by a block is
/// parsed as before in the same model (one result ok, so done 1 / work 1).
#[test]
fn s29_whole_record_comparison_after_the_adr0035_atom() {
    let base = r#"model Records version "0.2" {
type T = enum { ok, no }
actor A { capacity 1
  capability r() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } }
  capability s() -> T { duration fixed 1min; result nondet { ok } }
}
process P { x = A.s(); if x == ok { e = A.r(); BODY } else { end no } }
bounds { cases 1; live_tasks 1; clock 5min }
verify safety no_resource_overcommit
type Entry = record { confidence: enum { high, low } }
}
"#;
    let s = changed(
        base,
        "BODY",
        "if e ==\n Entry {\n  confidence = high\n } { end a } else { end b }",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![(
            "U-SYN-0".to_string(),
            "unsupported syntax: record comparison is unsupported".to_string(),
            Some(s.find("e ==\n").unwrap() as u64)
        )],
        "{v}"
    );
    let s = changed(
        base,
        "BODY",
        "if e.confidence == high { end a } else { end b }",
    );
    let v = both(&s);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(
        v["terminals"],
        json!([{"end": "a", "elapsed": r(2), "work": r(2), "result_at": null}]),
        "{v}"
    );
}

/// The none comparison of an optional record stays accepted: entry is never
/// written, so `entry == none` holds and the case ends a at 0 (reachable).
#[test]
fn s29_none_comparison_of_an_optional_record_is_accepted() {
    let s = r#"model Records version "0.2" {
type Entry = record { confidence: enum { high, low } }
observed case { entry: optional<Entry> }
process P { if entry == none { end a } else { end b } }
bounds { cases 1; live_tasks 1; clock 5min }
verify reachable end.a
}
"#;
    let v = both(s);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(
        v["terminals"],
        json!([{"end": "a", "elapsed": r(0), "work": r(0), "result_at": null}]),
        "{v}"
    );
}

fn refused_everywhere(m: &Model, fault: &str) {
    for arrivals in [false, true] {
        let mut m = m.clone();
        if arrivals {
            m.arrivals = Some(vec![0]);
        }
        let e = engine::explore(&m);
        assert!(
            e.diagnostics.iter().any(|d| d.code == "U-SEM-0"),
            "{fault}, arrivals={arrivals}: {:?}",
            e.diagnostics
        );
        assert!(e.properties.is_empty() && e.terminals.is_empty(), "{fault}");
        assert_eq!(
            reference::explore(&m).stop,
            reference::StopReason::SemanticAbort,
            "{fault}, arrivals={arrivals}"
        );
    }
}

/// Public IR: a record field whose domain is the reserved CaseId TypeId or the
/// builtin PersonId type, and an atomic block on a record-return Capability or
/// writing a record state, are refused by each backend alone (engine U-SEM-0,
/// reference SemanticAbort), on both entries, without an index panic. The
/// simulation entry does not sample a malformed record field.
#[test]
fn s29_public_ir_case_id_person_and_atomic_record_fail_closed() {
    let good = model(
        r#"model Records version "0.2" {
type Ack = enum { committed, refused }
type Entry = record { confidence: enum { high, low } }
people [p1]
observed case { entry: optional<Entry>; who: optional<PersonId>; allowed: bool = true; registered: bool = false }
actor W { capacity 1
  capability r() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } }; writes entry }
  capability g() -> Ack { duration fixed 1min; writes registered; atomic { if allowed == true { registered = true; result committed } else { result refused } } }
}
process P { entry = W.r(); a = W.g(); end done }
bounds { cases 1; live_tasks 1; clock 5min }
verify safety no_resource_overcommit
}
"#,
    );
    // Positive control on both entries.
    for arrivals in [false, true] {
        let mut m = good.clone();
        if arrivals {
            m.arrivals = Some(vec![0]);
        }
        assert_eq!(reference::explore(&m).stop, reference::StopReason::Complete);
        assert!(
            engine::explore(&m)
                .diagnostics
                .iter()
                .all(|d| d.code != "U-SEM-0")
        );
    }
    let entry = good.types.iter().position(|t| t.name == "Entry").unwrap();
    let person = good
        .types
        .iter()
        .position(|t| t.name == "PersonId")
        .unwrap();
    let mut m = good.clone();
    m.types[entry].record.as_mut().unwrap().fields[0].ty = CASE_ID_TYPE;
    refused_everywhere(&m, "CaseId field");
    let mut m = good.clone();
    m.types[entry].record.as_mut().unwrap().fields[0].ty = TypeId(person);
    refused_everywhere(&m, "PersonId field");
    let r = good
        .capabilities
        .iter()
        .position(|c| c.name == "r")
        .unwrap();
    let g = good
        .capabilities
        .iter()
        .position(|c| c.name == "g")
        .unwrap();
    // atomic on the record-return Capability (results and values in range).
    let mut m = good.clone();
    let mut atomic = good.capabilities[g].atomic.clone().unwrap();
    atomic.commit = ValueId(0);
    atomic.reject = ValueId(1);
    m.capabilities[r].atomic = Some(atomic);
    m.capabilities[r].results = vec![ValueId(0), ValueId(1)];
    m.capabilities[r].writes.clear();
    refused_everywhere(&m, "atomic with record result");
    // atomic writing a whole record constant into the record state.
    let mut m = good.clone();
    let state = good
        .state_variables
        .iter()
        .position(|v| v.name == "entry")
        .unwrap();
    m.capabilities[g].atomic.as_mut().unwrap().writes = vec![StateWrite {
        variable: StateVarId(state),
        source: WriteSource::Constant(StateValue::Value(ValueId(0))),
    }];
    refused_everywhere(&m, "atomic record write");
    // Simulation entry: the probability fixture samples 3 replications; the
    // same model with a CaseId or PersonId-free malformed field samples none.
    let mut sim = model(&changed(
        &std::fs::read_to_string(root().join("fixtures/rec-prob-field/model.relca")).unwrap(),
        "verify safety",
        "simulation replications 3 seed 19 confidence 0.95\nverify safety",
    ));
    for arrivals in [false, true] {
        if arrivals {
            sim.arrivals = Some(vec![0]);
        }
        assert_eq!(engine::simulate(&sim).completed_replications, 3);
        let pair = sim.types.iter().position(|t| t.record.is_some()).unwrap();
        let mut m = sim.clone();
        m.types[pair].record.as_mut().unwrap().fields[0].ty = CASE_ID_TYPE;
        let bad = engine::simulate(&m);
        assert_eq!(bad.completed_replications, 0, "arrivals={arrivals}");
        assert!(bad.diagnostics.iter().any(|d| d.code == "U-SEM-0"));
    }
}
