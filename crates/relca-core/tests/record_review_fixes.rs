//! ADR 0038 boundary review (m3〜m7) and surviving mutants (F9, O01, O02,
//! M23R2, O06b, O10, O21). Every expected value is derived by hand from the
//! ADR 0038 rules and the legacy scalar twin of the same position, never
//! copied from an implementation's output.
use relca_core::{engine, ir::*, lower, reference, report, syntax};
use serde_json::{Value, json};

fn root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn fixture(name: &str) -> String {
    std::fs::read_to_string(root().join("fixtures").join(name).join("model.relca")).unwrap()
}

fn model(s: &str) -> Model {
    lower::lower(&syntax::parse(s).unwrap_or_else(|e| panic!("{e:?}\n{s}")))
        .unwrap_or_else(|e| panic!("{e:?}\n{s}"))
}

fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}

fn check_in(s: &str, configuration: &str) -> Value {
    serde_json::to_value(report::check_configuration(s, configuration)).unwrap()
}

/// Replaces exactly one occurrence, so an edit never silently misses.
fn changed(s: &str, from: &str, to: &str) -> String {
    assert_eq!(s.matches(from).count(), 1, "{from}");
    s.replace(from, to)
}

/// Error diagnostics as (code, message, span start, span end).
fn errors(v: &Value) -> Vec<(String, String, Option<u64>, Option<u64>)> {
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
                d["span"]["end"].as_u64(),
            )
        })
        .collect()
}

fn err(
    code: &str,
    message: &str,
    start: usize,
    len: usize,
) -> (String, String, Option<u64>, Option<u64>) {
    (
        code.into(),
        message.into(),
        Some(start as u64),
        Some((start + len) as u64),
    )
}

/// Byte offset of the `nth` (0-based) occurrence of `needle`.
fn at(s: &str, needle: &str, nth: usize) -> usize {
    s.match_indices(needle)
        .nth(nth)
        .unwrap_or_else(|| panic!("{needle}"))
        .0
}

fn rejected(v: &Value, verdict: &str) {
    assert_eq!(v["verdict"], verdict, "{v}");
    assert_eq!(v["properties"], json!([]), "{v}");
    assert_eq!(v["terminals"], json!([]), "{v}");
}

// ---------------------------------------------------------------- m3

const OTHER: &str = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = record { f: bool }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { R { f = true } } }
    capability s() -> T { duration fixed 1min; result nondet { ok } }
  }
  process P { e = A.r(); s = A.s(); end done }
  process Q { if READ { end done } else { end no } }
  cut Scope { include [A] }
  configuration One { cut Scope; process P }
  configuration Two { cut Scope; process Q }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;

/// Q reads a binding that only P declares. The legacy scalar twin (`s == ok`)
/// is E-NAME-0 `undefined result binding` at the binding, both with the
/// configuration selected (single process, flow checked) and without it
/// (two processes, flow not checked). The record field read takes the same
/// class at the same position, whatever the flow mode or the spelling.
#[test]
fn m3_other_process_record_binding_is_an_undefined_result_binding() {
    for (read, name) in [("e.f == true", "e"), ("s == ok", "s")] {
        let s = changed(OTHER, "READ", read);
        let pos = at(&s, read, 0);
        let selected = check_in(&s, "Two");
        rejected(&selected, "invalid");
        assert_eq!(
            errors(&selected),
            vec![err("E-NAME-0", "undefined result binding", pos, name.len())],
            "{read}: {selected}"
        );
        // Without configurations both processes lower; flow is not checked.
        let unselected = s
            .replace("  cut Scope { include [A] }\n", "")
            .replace("  configuration One { cut Scope; process P }\n", "")
            .replace("  configuration Two { cut Scope; process Q }\n", "");
        let pos = at(&unselected, read, 0);
        let v = check(&unselected);
        rejected(&v, "invalid");
        let e = errors(&v);
        assert_eq!(e.len(), 2, "{read}: {v}");
        assert_eq!(
            (e[0].0.as_str(), e[0].1.as_str()),
            ("U-SEM-0", "exactly one process is supported"),
            "{read}: {v}"
        );
        assert_eq!(
            e[1],
            err("E-NAME-0", "undefined result binding", pos, name.len()),
            "{read}: {v}"
        );
    }
    // Another spelling of the same binding gives the same class.
    let s = changed(OTHER, "READ", "e.f == true")
        .replace("e = A.r()", "zz_entry = A.r()")
        .replace("e.f == true", "zz_entry.f == true");
    let pos = at(&s, "zz_entry.f", 0);
    assert_eq!(
        errors(&check_in(&s, "Two")),
        vec![err("E-NAME-0", "undefined result binding", pos, 8)]
    );
}

// ---------------------------------------------------------------- m4

/// var_decl rejects `x: list<bool>` as U-SYN-0 `nested or unknown type
/// constructor`; a record field type is the same type position.
#[test]
fn m4_unknown_record_field_type_constructor_matches_var_decl() {
    let base = fixture("rec-size");
    for field in ["f0: list<bool>", "f0: R<bool>", "f0: bool<bool>"] {
        let s = changed(&base, "f0: bool", field);
        let v = check(&s);
        rejected(&v, "unsupported");
        let e = errors(&v);
        assert_eq!(e.len(), 1, "{field}: {v}");
        assert_eq!(
            (e[0].0.as_str(), e[0].1.as_str()),
            (
                "U-SYN-0",
                "unsupported syntax: nested or unknown type constructor"
            ),
            "{field}: {v}"
        );
    }
    // The legacy twin, for the shared wording.
    let legacy = r#"model M version "0.2" {
  observed case { x: list<bool> }
  process P { end done }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
    let e = errors(&check(legacy));
    assert_eq!(
        (e[0].0.as_str(), e[0].1.as_str()),
        (
            "U-SYN-0",
            "unsupported syntax: nested or unknown type constructor"
        )
    );
}

// ---------------------------------------------------------------- m5

const LATENT_TWIN: &str = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = record { f: bool }
  latent case { e: bool = true }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { R { f = true } } }
    capability s() -> T { duration fixed 1min; result nondet { ok } }
    capability c() -> T { commits; duration fixed 1min; result nondet { ok } }
  }
  process P { e = BIND; a = A.c(); end done }
  contract C { invariant A.c.commit -> READ }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;

/// A latent named like a binding: the scalar contract read is U-SEM-0
/// `invariant name is both a binding and a latent variable` at the name. The
/// record field read in the same position takes the same guard.
#[test]
fn m5_contract_latent_named_like_record_binding_is_guarded() {
    for (bind, read) in [("A.s()", "e == ok"), ("A.r()", "e.f == true")] {
        let s = changed(&changed(LATENT_TWIN, "BIND", bind), "READ", read);
        let pos = at(&s, read, 0);
        let v = check(&s);
        rejected(&v, "unsupported");
        assert_eq!(
            errors(&v),
            vec![err(
                "U-SEM-0",
                "invariant name is both a binding and a latent variable",
                pos,
                1
            )],
            "{read}: {v}"
        );
    }
    // Control: without the latent the record contract read is accepted.
    let s = changed(
        &changed(
            &changed(LATENT_TWIN, "BIND", "A.r()"),
            "READ",
            "e.f == true",
        ),
        "  latent case { e: bool = true }\n",
        "",
    );
    assert_eq!(check(&s)["verdict"], "satisfied", "{}", check(&s));
}

// ---------------------------------------------------------------- m6

const BASE: &str = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = record { f: bool }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { R { f = true } } }
    capability s() -> T { duration fixed 1min; result nondet { ok } }
    capability use(x: R) -> T { duration fixed 1min; result nondet { ok } }
  }
  process P { BODY }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;

/// A duplicate binding is U-SEM-0 at both declarations (the legacy rule);
/// the record field read of the ambiguous name adds U-SEM-0 `ambiguous
/// record binding` at the read, never a first-wins resolution.
#[test]
fn m6_ambiguous_record_binding() {
    let s = changed(
        BASE,
        "BODY",
        "e = A.r(); e = A.r(); if e.f == true { end done } else { end no }",
    );
    let v = check(&s);
    rejected(&v, "unsupported");
    let mut e = errors(&v);
    e.sort_by_key(|d| d.2);
    assert_eq!(
        e,
        vec![
            err(
                "U-SEM-0",
                "duplicate or shadowed binding",
                at(&s, "e = A.r()", 0),
                1
            ),
            err(
                "U-SEM-0",
                "duplicate or shadowed binding",
                at(&s, "e = A.r()", 1),
                1
            ),
            err("U-SEM-0", "ambiguous record binding", at(&s, "e.f", 0), 1),
        ],
        "{v}"
    );
}

/// An active contract reads a binding of a process outside the selected
/// configuration: the scalar contract read is U-SEM-0 `contract binding
/// belongs to another process`; so is the record field read.
#[test]
fn m6_record_contract_binding_of_another_process() {
    let src = r#"model M version "0.2" {
  type T = enum { ok, ng }
  type R = record { f: bool }
  actor A { capacity 1
    capability r() -> R { duration fixed 1min; result nondet { R { f = true } } }
    capability s() -> T { duration fixed 1min; result nondet { ok } }
    capability c() -> T { commits; duration fixed 1min; result nondet { ok } }
  }
  process P { e = A.r(); s = A.s(); end done }
  process Q { a = A.c(); end done }
  cut Scope { include [A] }
  configuration One { cut Scope; process P }
  configuration Two { cut Scope; process Q }
  contract C { invariant A.c.commit -> READ }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
    for (read, name) in [("s == ok", "s"), ("e.f == true", "e")] {
        let s = changed(src, "READ", read);
        let pos = at(&s, read, 0);
        let v = check_in(&s, "Two");
        rejected(&v, "unsupported");
        assert_eq!(
            errors(&v),
            vec![err(
                "U-SEM-0",
                "contract binding belongs to another process",
                pos,
                name.len()
            )],
            "{read}: {v}"
        );
    }
}

/// A record field value is one value name; a nested record, a qualified name
/// or arithmetic is U-SYN-0 `record field value is unsupported`. A record
/// literal argument is U-SYN-0 `record argument literal is unsupported` once a
/// record is declared.
#[test]
fn m6_record_field_value_and_argument_literal_are_unsupported() {
    for value in [
        "R { f = R { f = true } }",
        "R { f = A.f }",
        "R { f = true + 1 }",
    ] {
        let s = changed(
            &changed(
                BASE,
                "result nondet { R { f = true } }",
                &format!("result nondet {{ {value} }}"),
            ),
            "BODY",
            "e = A.r(); end done",
        );
        let v = check(&s);
        rejected(&v, "unsupported");
        let e = errors(&v);
        assert_eq!(e.len(), 1, "{value}: {v}");
        assert_eq!(
            (e[0].0.as_str(), e[0].1.as_str()),
            (
                "U-SYN-0",
                "unsupported syntax: record field value is unsupported"
            ),
            "{value}: {v}"
        );
    }
    let s = changed(BASE, "BODY", "x = A.r(); a = A.use(x {}); end done");
    let v = check(&s);
    rejected(&v, "unsupported");
    let e = errors(&v);
    assert_eq!(e.len(), 1, "{v}");
    assert_eq!(
        (e[0].0.as_str(), e[0].1.as_str()),
        (
            "U-SYN-0",
            "unsupported syntax: record argument literal is unsupported"
        )
    );
    // Control: the plain argument is accepted.
    let s = changed(BASE, "BODY", "x = A.r(); a = A.use(x); end done");
    assert_eq!(check(&s)["verdict"], "satisfied", "{}", check(&s));
}

// ---------------------------------------------------------------- m7

/// E-TYPE-0 names the type the author wrote. An inline enum has no name, so
/// the message names its position (the record field or the Capability result)
/// and never shows an internal type name.
#[test]
fn m7_inline_enum_type_errors_name_the_written_position() {
    let field = r#"model M version "0.2" {
  type Entry = record { confidence: enum { high, low } }
  actor A { capacity 1
    capability r() -> Entry { duration fixed 1min; result nondet { Entry { confidence = medium } } }
  }
  process P { e = A.r(); end done }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
    let v = check(field);
    rejected(&v, "invalid");
    assert_eq!(
        errors(&v),
        vec![err(
            "E-TYPE-0",
            "medium is not a member of inline enum of field Entry.confidence",
            at(field, "medium", 0),
            6
        )],
        "{v}"
    );
    let read = changed(
        field,
        "Entry { confidence = medium }",
        "Entry { confidence = high }",
    );
    let read = changed(
        &read,
        "e = A.r(); end done",
        "e = A.r(); if e.confidence == medium { end done } else { end no }",
    );
    let v = check(&read);
    rejected(&v, "invalid");
    assert_eq!(
        errors(&v),
        vec![err(
            "E-TYPE-0",
            "medium is not a member of inline enum of field Entry.confidence",
            at(&read, "medium", 0),
            6
        )],
        "{v}"
    );
    let result = r#"model M version "0.2" {
  actor A { capacity 1
    capability c() -> enum { yes, no } { duration fixed 1min; result nondet { maybe } }
  }
  process P { r = A.c(); end done }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
    let v = check(result);
    rejected(&v, "invalid");
    assert_eq!(
        errors(&v),
        vec![err(
            "E-TYPE-0",
            "maybe is not a member of inline enum of result A.c",
            at(result, "maybe", 0),
            5
        )],
        "{v}"
    );
    for v in [check(field), check(&read), check(result)] {
        assert!(!v.to_string().contains("@inline"), "{v}");
    }
}

// ---------------------------------------------------------------- P-B (M23R2)

/// rec-s18-given forged so that the only remaining defect is a record in the
/// kernel `given`: entry is non-optional with a fixed initial member, and the
/// latent read list is emptied (the existing forgery kept it, so the
/// reference rejected it for another reason). Rows: K has 2 rows and entry's
/// domain is 2 (high, low). The same forgery on the observed bool
/// `registered` is accepted by the reference (control).
#[test]
fn pb_record_kernel_given_is_rejected_by_each_backend_alone() {
    let forge = |name: &str| {
        let mut m = model(&fixture("rec-s18-given"));
        let id = m
            .state_variables
            .iter()
            .position(|v| v.name == name)
            .unwrap();
        if name == "entry" {
            m.state_variables[id].optional = false;
            m.state_variables[id].initial = InitialValue::Fixed(StateValue::Value(ValueId(0)));
        }
        m.kernels[0].given = vec![StateVarId(id)];
        m.capabilities[0].kernel.as_mut().unwrap().given = vec![StateVarId(id)];
        m.capabilities[0].kernel_reads_latent.clear();
        m
    };
    let control = forge("registered");
    assert_eq!(
        reference::explore(&control).stop,
        reference::StopReason::Complete
    );
    assert!(
        engine::explore(&control)
            .diagnostics
            .iter()
            .all(|d| d.code != "U-SEM-0")
    );
    let m = forge("entry");
    let e = engine::explore(&m);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.code == "U-SEM-0"
                && d.message.contains("record in kernel given is unsupported")),
        "{:?}",
        e.diagnostics
    );
    assert!(e.properties.is_empty() && e.terminals.is_empty());
    assert_eq!(
        reference::explore(&m).stop,
        reference::StopReason::SemanticAbort
    );
}

// ---------------------------------------------------------------- P-C (F9)

const FLAG_FIRST: &str = r#"model Records version "0.2" {
type Pair = record { flag: bool; color: enum { red, blue, green } }
observed case { pair: optional<Pair> }
actor Extractor { capacity 1; capability extract() -> Pair { duration fixed 1min; result nondet { Pair { flag = false, color = red } }; writes pair } }
process Flow { p = Extractor.extract(); if p.color == red { end red } else { end other } }

bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
verify reachable end.red
}"#;

/// Declaration order differs from spelling order. The first declared field is
/// the most significant: bool is [true, false], color is [red, blue, green],
/// so `Pair { flag = false, color = red }` is 1 * 3 + 0 = ValueId(3). With
/// spelling order it would be 0 * 2 + 1 = 1 (flag = true, color = blue) and
/// end.red would be unreachable.
#[test]
fn pc_record_value_index_uses_declaration_order() {
    let m = model(FLAG_FIRST);
    let pair = m.types.iter().find(|t| t.name == "Pair").unwrap();
    assert_eq!(pair.members.len(), 6);
    assert_eq!(pair.members[3], "Pair { flag = false, color = red }");
    let cap = m.capabilities.iter().find(|c| c.name == "extract").unwrap();
    assert_eq!(cap.results, vec![ValueId(3)]);
    let v = check(FLAG_FIRST);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let t = v["terminals"].as_array().unwrap();
    assert_eq!(t.len(), 1, "{v}");
    assert_eq!(t[0]["end"], "red");
    assert_eq!(t[0]["elapsed"], json!({"min": 1, "max": 1}));
    assert_eq!(t[0]["work"], json!({"min": 1, "max": 1}));
}

// ---------------------------------------------------------------- P-D (O01, O02)

/// rec-size (10 bools, 1024 members) forged with an 11th bool: 2048 canonical
/// members in declaration order (member i has field k = true iff bit 10-k of
/// i is 0). Lower rejects it from the DSL, so only this forgery reaches the
/// engine and reference bounds; each must reject it on its own.
#[test]
fn pd_forged_2048_member_record_is_rejected_by_each_backend() {
    let mut m = model(&fixture("rec-size"));
    let r = m.types.iter().position(|t| t.name == "R").unwrap();
    let b = m.types[r].record.as_ref().unwrap().fields[0].ty;
    m.types[r]
        .record
        .as_mut()
        .unwrap()
        .fields
        .push(RecordField {
            name: "f10".into(),
            ty: b,
        });
    m.types[r].members = (0..2048usize)
        .map(|i| {
            let parts: Vec<_> = (0..11)
                .map(|k| {
                    format!(
                        "f{k} = {}",
                        if (i >> (10 - k)) & 1 == 0 {
                            "true"
                        } else {
                            "false"
                        }
                    )
                })
                .collect();
            format!("R {{ {} }}", parts.join(", "))
        })
        .collect();
    let e = engine::explore(&m);
    assert!(
        e.diagnostics.iter().any(|d| d.code == "U-SEM-0"
            && d.message
                .contains("record domain exceeds the supported size")),
        "{:?}",
        e.diagnostics
    );
    assert!(e.properties.is_empty() && e.terminals.is_empty());
    assert_eq!(
        reference::explore(&m).stop,
        reference::StopReason::SemanticAbort
    );
}

// ---------------------------------------------------------------- P-E (O06b)

/// A whole-record comparison of a case variable: `entry == none` is the
/// optional exception (initial none, so end.empty at elapsed 0, work 0);
/// `entry == high` is U-SYN-0 `record comparison is unsupported` at the value.
#[test]
fn pe_case_variable_whole_record_comparison_is_unsupported() {
    let base = r#"model Records version "0.2" {
type Entry = record { confidence: enum { high, low } }
observed case { entry: optional<Entry> }
process Flow { if entry == none { end empty } else { end filled } }

bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
verify reachable end.empty
}"#;
    let v = check(base);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let t = v["terminals"].as_array().unwrap();
    assert_eq!(t.len(), 1, "{v}");
    assert_eq!(t[0]["end"], "empty");
    assert_eq!(t[0]["elapsed"], json!({"min": 0, "max": 0}));
    assert_eq!(t[0]["work"], json!({"min": 0, "max": 0}));
    let broken = changed(base, "if entry == none", "if entry == high");
    let v = check(&broken);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![err(
            "U-SYN-0",
            "record comparison is unsupported",
            at(&broken, "high {", 0),
            4
        )],
        "{v}"
    );
}

// ---------------------------------------------------------------- P-F (O10)

const UNAVAILABLE: &str = r#"model Records version "0.2" {
type Entry = record { confidence: enum { high, low } }
type T = enum { ok, ng }
observed case { entry: optional<Entry>; registered: bool = false }
actor Extractor { capacity 1
  capability extract() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } }; writes entry }
  capability scalar() -> T { duration fixed 1min; result nondet { ok } }
}
process Flow { if registered == true { e = Extractor.CAP(); end x } else { ELSE } }

bounds { cases 1; live_tasks 1; clock 20min }
verify safety no_resource_overcommit
verify reachable end.y
}"#;

/// The binding is declared only on the then-branch; the else-branch reads it.
/// The scalar read is U-SEM-0 `result is not definitely available (not yet
/// defined or possibly cancelled)` at the binding, from lower; the record
/// field read is the same rule at the same span (not a later engine check).
#[test]
fn pf_unavailable_record_binding_read_is_lower_s_availability_error() {
    let base = changed(&changed(UNAVAILABLE, "CAP", "extract"), "ELSE", "end y");
    assert_eq!(check(&base)["verdict"], "satisfied", "{}", check(&base));
    for (cap, read) in [("scalar", "e == ok"), ("extract", "e.confidence == high")] {
        let s = changed(
            &changed(UNAVAILABLE, "CAP", cap),
            "ELSE",
            &format!("if {read} {{ end a }} else {{ end y }}"),
        );
        let v = check(&s);
        // The message and span first: the engine's own IR check would also
        // reject the read, with another message and no span.
        assert_eq!(
            errors(&v),
            vec![err(
                "U-SEM-0",
                "result is not definitely available (not yet defined or possibly cancelled)",
                at(&s, read, 0),
                1
            )],
            "{read}: {v}"
        );
        rejected(&v, "unsupported");
    }
}

// ---------------------------------------------------------------- P-G (O21)

/// rec-context-scope with the bindings inside an if. entry is fixed to
/// `Entry { confidence = high }`, so done: seed 1min then run 1min, elapsed 2,
/// work 2. The broken side reads the nested record binding in the authority
/// scope, a state-only position: U-SEM-0 `record binding is unsupported in
/// this condition` at the binding and no other error.
#[test]
fn pg_record_binding_inside_if_is_still_a_record_binding() {
    let base = changed(
        &fixture("rec-context-scope"),
        "process Flow { e = Seed.seed(); r = Work.run(); end done }",
        "process Flow { if entry.confidence == high { e = Seed.seed(); r = Work.run(); end done } else { end skipped } }",
    );
    let v = check(&base);
    assert_eq!(v["verdict"], "satisfied", "{v}");
    let t = v["terminals"].as_array().unwrap();
    assert_eq!(t.len(), 1, "{v}");
    assert_eq!(t[0]["end"], "done");
    assert_eq!(t[0]["elapsed"], json!({"min": 2, "max": 2}));
    assert_eq!(t[0]["work"], json!({"min": 2, "max": 2}));
    let broken = changed(
        &base,
        "scope entry.confidence == high",
        "scope e.confidence == high",
    );
    let v = check(&broken);
    rejected(&v, "unsupported");
    assert_eq!(
        errors(&v),
        vec![err(
            "U-SEM-0",
            "record binding is unsupported in this condition",
            at(&broken, "e.confidence == high }", 0),
            1
        )],
        "{v}"
    );
}
