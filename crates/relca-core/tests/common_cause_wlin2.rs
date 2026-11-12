//! Independent, hand-written ADR 0024 structural oracles (not new CLI captures).
use relca_core::{lower, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;

#[path = "support/rule_quality_inputs.rs"]
mod deferred_control;
#[path = "support/common_cause_population.rs"]
mod population;
pub use population::NAMES;
#[test]
fn wlin2_is_registered_with_warning_severity() {
    assert_eq!(
        relca_core::diag::severity_of("W-LIN-2"),
        Some(relca_core::diag::Severity::Warning)
    );
}
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    std::fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn expected(name: &str) -> Value {
    serde_json::from_slice(&std::fs::read(root().join(name).join("expected.json")).unwrap())
        .unwrap()
}
#[test]
fn twelve_fixture_pairs_have_only_the_declared_single_transformation() {
    type Transformation = (&'static str, &'static str, fn(String) -> String);
    let transformations: [Transformation; 12] = [
        ("lin-invoice", "po-evidence", |s| {
            s.replace("reads [PurchaseOrder]; ", "")
        }),
        ("lin-five-kernels", "given-only-read", |mut s| {
            let lo = s
                .find("  kernel human_entry given [Invoice.difficulty]")
                .unwrap();
            let hi = lo + s[lo..].find("  kernel ai_entry").unwrap();
            s.replace_range(lo..hi,"  kernel human_entry { * -> { 0.98: high with extraction_correct = true; 0.02: high with extraction_correct = false } }\n");
            s
        }),
        ("lin-basis", "no-basis", |s| {
            s.replace(" basis \"sample X measured independent detection\"", "")
        }),
        ("lin-tacit", "unused-tacit", |s| {
            s.replace("reads [Experience]; ", "")
        }),
        ("lin-correlation", "unrelated-given", |s| {
            s.replace("KB given [bad,a]", "KB given [bad,c]")
        }),
        ("lin-single", "second-call", |s| {
            s.replace("    end done", "    bV2 = B.check(e)\n    end done")
        }),
        ("lin-case-latent", "actor-local", |s| {
            s.replace("  latent case { difficulty: bool = false }\n", "")
                .replace(
                    "source document }",
                    "source document; latent difficulty: bool = false }",
                )
                .replace("[difficulty]", "[Invoice.difficulty]")
        }),
        ("lin-numeric", "zero-detection", |s| {
            s.replace(
                "true -> { 0.5: pass; 0.5: fail }",
                "true -> { 1: pass; 0: fail }",
            )
        }),
        ("lin-no-error-writer", "connect-error-writer", |s| {
            s.replace("e = P.make()", "e = P.make_bad()")
        }),
        ("lin-same-actor", "read-removed", |s| {
            s.replace(
                "reads [R]; duration fixed 1min; outcomes kernel KB",
                "duration fixed 1min; outcomes kernel KB",
            )
        }),
        ("lin-shared-kernel", "drop-call", |s| {
            s.replace(" c2 = A.check();", "")
        }),
        ("lin-same-source-different-target", "use-same-target", |s| {
            s.replace("B.check(e2)", "B.check(e1)")
        }),
    ];
    for (base, fault, edit) in transformations {
        let s = source(base);
        let changed = edit(s.clone());
        assert_ne!(changed, s, "nonempty declared change {base}");
        assert_eq!(
            source(&format!("{base}.broken-{fault}")),
            changed,
            "no second fault in {base}"
        );
    }
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array {a}"));
            assert_eq!(es.len(), actual.len(), "exact inventory {a}");
            for (x, y) in es.iter().zip(actual) {
                subset(x, y);
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn warning_calls(name: &str) -> Vec<&'static str> {
    match name {
        "lin-invoice.broken-po-evidence" => vec!["poV", "approvalV"],
        "lin-invoice" | "lin-five-kernels" | "lin-five-kernels.broken-given-only-read" => {
            vec!["approvalV"]
        }
        "lin-single.broken-second-call" => vec!["bV", "bV2"],
        "lin-same-actor" => vec!["aV", "bV"],
        "lin-basis.broken-no-basis"
        | "lin-tacit.broken-unused-tacit"
        | "lin-correlation.broken-unrelated-given"
        | "lin-numeric"
        | "lin-numeric.broken-zero-detection"
        | "lin-no-error-writer.broken-connect-error-writer"
        | "lin-same-actor.broken-read-removed"
        | "lin-same-source-different-target.broken-use-same-target" => vec!["bV"],
        _ => vec![],
    }
}
pub fn assert_report(name: &str, src: &str, actual: &Value) {
    subset(&expected(name), actual);
    let agency = &actual["agency"];
    assert!(agency.is_object(), "complete static derivation {name}");
    assert!(
        !agency["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("W-LIN-2")),
        "checked in every Report {name}"
    );
    for key in [
        "common_causes",
        "model_uncertainties",
        "source_descriptions",
    ] {
        if expected(name)["agency"].get(key).is_none() {
            assert!(
                agency.get(key).is_none(),
                "no empty/null/unjustified {key} in {name}"
            );
        } else {
            assert_eq!(
                agency[key],
                expected(name)["agency"][key],
                "full table {key} in {name}"
            );
        }
    }
    let diagnostics = actual["diagnostics"].as_array().unwrap();
    assert_eq!(
        diagnostics.len(),
        expected(name)["diagnostics"].as_array().unwrap().len(),
        "no unexpected warnings {name}"
    );
    for (d, call) in diagnostics
        .iter()
        .filter(|d| d["code"] == "W-LIN-2")
        .zip(warning_calls(name))
    {
        assert_eq!(d["code"], "W-LIN-2");
        assert_eq!(d["severity"], "warning");
        let start = d["span"]["start"].as_u64().unwrap() as usize;
        let end = d["span"]["end"].as_u64().unwrap() as usize;
        let from = src.find(&format!("{call} =")).unwrap();
        let to = from + src[from..].find(')').unwrap() + 1;
        assert_eq!(
            (start, end),
            (from, to),
            "exact UTF8 byte call span {name}/{call}"
        );
        assert_eq!(&src[start..end], &src[from..to]);
        let message = d["message"].as_str().unwrap();
        assert!(
            message.contains(call),
            "call is identified {name}: {message}"
        );
    }
    let hand = expected(name);
    let new: Vec<_> = diagnostics
        .iter()
        .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
        .collect();
    let expected_new: Vec<_> = hand["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"].as_str().unwrap().starts_with("W-UNREACH-"))
        .collect();
    assert_eq!(
        new.len(),
        expected_new.len(),
        "fixed ADR0025 warning inventory"
    );
    for (a, h) in new.iter().zip(expected_new) {
        for key in ["code", "severity", "span"] {
            assert_eq!(a[key], h[key], "fixed ADR0025 {name}");
        }
    }
}

/// Derived in lin-invoice/calc.md and lin-correlation/calc.md before execution.
/// No producer output participates in this oracle.
pub struct BoundaryCase {
    pub label: &'static str,
    pub src: String,
    pub calls: Vec<&'static str>,
    pub uncertainties: Option<Value>,
    pub elapsed: u64,
    pub work: u64,
}
pub fn boundary_cases() -> Vec<BoundaryCase> {
    let correlation = source("lin-correlation");
    let hand = expected("lin-correlation")["agency"]["model_uncertainties"].clone();
    let row = "false, none -> { 1: pass }; false, pass -> { 1: pass }; false, fail -> { 1: pass }; true, none -> { 1: pass }; true, pass -> { 1: pass }; true, fail -> { 1: pass }";
    let mutual = correlation.replace("reads [R]; ", "").replace(
        "KA given [bad] { false -> { 1: pass }; true -> { 1: pass } }",
        &format!("KA given [bad,b] {{ {row} }}"),
    );
    let foreign = correlation
        .replace("KB given [bad,a]", "KB given [bad,c]")
        .replace("  process Flow {", "  actor C { capacity 1; capability check(e:Entry) -> Verdict { reads [R]; duration fixed 1min; outcomes kernel KA; kernel_reads latent [bad]; writes c } }\n  process Flow {")
        .replace("    aV = A.check(e)", "    e2 = P.make()\n    cV = C.check(e2)\n    aV = A.check(e)")
        .replace("include [S,R,P,A,B]", "include [S,R,P,A,B,C]");
    let mut foreign_hand = hand.clone();
    foreign_hand["items"][0]["targets"] = json!(["e2"]);
    foreign_hand["items"][0]["judgements"] = json!(["cV"]);
    let reread = source("lin-invoice")
        .replace("approve(e:Entry,p:Verdict,r:Verdict)", "approve(e:Entry)")
        .replace(
            "approve(e:Entry) -> Verdict {",
            "approve(e:Entry) -> Verdict { reads [PurchaseOrder];",
        )
        .replace(
            "Approver.approve(entry,poV,receiptV)",
            "Approver.approve(entry)",
        );
    assert!(reread.contains("approve(e:Entry) -> Verdict { reads [PurchaseOrder];"));
    vec![
        BoundaryCase {
            label: "reread",
            src: reread,
            calls: vec!["poV", "approvalV"],
            uncertainties: None,
            elapsed: 17,
            work: 17,
        },
        BoundaryCase {
            label: "serial",
            src: correlation.clone(),
            calls: vec![],
            uncertainties: Some(hand.clone()),
            elapsed: 3,
            work: 3,
        },
        BoundaryCase {
            label: "e1-future-result",
            src: correlation.replace(
                "    aV = A.check(e)\n    bV = B.check(e)",
                "    bV = B.check(e)\n    aV = A.check(e)",
            ),
            calls: vec!["bV"],
            uncertainties: None,
            elapsed: 3,
            work: 3,
        },
        BoundaryCase {
            label: "e4-mutual",
            src: mutual,
            calls: vec!["aV", "bV"],
            uncertainties: None,
            elapsed: 3,
            work: 3,
        },
        BoundaryCase {
            label: "fork-unordered",
            src: correlation.replace(
                "    aV = A.check(e)\n    bV = B.check(e)",
                "    fork checks { aV = A.check(e); bV = B.check(e) }\n    join all checks",
            ),
            calls: vec!["bV"],
            uncertainties: None,
            elapsed: 2,
            work: 3,
        },
        BoundaryCase {
            label: "argument-ancestor",
            src: correlation
                .replace(
                    "actor B { capacity 1\n    capability check(e:Entry)",
                    "actor B { capacity 1\n    capability check(e:Entry,prior:Verdict)",
                )
                .replace("B.check(e)", "B.check(e,aV)"),
            calls: vec![],
            uncertainties: Some(hand),
            elapsed: 3,
            work: 3,
        },
        BoundaryCase {
            label: "foreign-group",
            src: foreign.clone(),
            calls: vec!["bV"],
            uncertainties: None,
            elapsed: 5,
            work: 5,
        },
        BoundaryCase {
            label: "foreign-group-positive-control",
            src: foreign.replace("B.check(e)", "B.check(e2)"),
            calls: vec![],
            uncertainties: Some(foreign_hand),
            elapsed: 5,
            work: 5,
        },
    ]
}
pub fn assert_boundary(case: &BoundaryCase, v: &Value) {
    assert_eq!(v["verdict"], "satisfied", "{}: {v}", case.label);
    let terminal = v["terminals"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| {
            t["end"]
                == if case.label == "reread" {
                    "posted"
                } else {
                    "done"
                }
        })
        .unwrap();
    assert_eq!(
        terminal["elapsed"],
        json!({"min":case.elapsed,"max":case.elapsed})
    );
    assert_eq!(terminal["work"], json!({"min":case.work,"max":case.work}));
    let ds: Vec<_> = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == "W-LIN-2")
        .collect();
    assert_eq!(
        ds.len(),
        case.calls.len(),
        "{} warning inventory: {v}",
        case.label
    );
    for (d, call) in ds.iter().zip(&case.calls) {
        let start = case.src.find(&format!("{call} =")).unwrap();
        let end = start + case.src[start..].find(')').unwrap() + 1;
        assert_eq!(
            d["span"],
            json!({"start":start,"end":end}),
            "{} {call}",
            case.label
        );
        assert_eq!(d["severity"], "warning");
    }
    assert_eq!(
        v["agency"].get("model_uncertainties"),
        case.uncertainties.as_ref(),
        "{} complete suppression inventory",
        case.label
    );
}

#[test]
fn m1_m2_m3_boundary_matrix_has_exact_calls_and_suppression_rows() {
    let cases = boundary_cases();
    assert_eq!(cases.len(), 8);
    let mut failed = vec![];
    for case in cases {
        let v = serde_json::to_value(report::check(&case.src)).unwrap();
        if std::panic::catch_unwind(|| assert_boundary(&case, &v)).is_err() {
            failed.push(case.label);
        }
    }
    assert!(
        failed.is_empty(),
        "all8 visited; boundary failures {failed:?}"
    );
}

#[test]
fn m7_result_alias_requires_the_same_type_not_just_value_ordinals() {
    use relca_core::agency;
    let src = source("lin-correlation").replace(
        "  type Entry",
        "  type OtherVerdict = enum { pass, fail }\n  type Entry",
    );
    let mut m = lower::lower(&syntax::parse(&src).unwrap()).unwrap();
    let (_, ds) = agency::derive(&m).unwrap();
    assert!(
        !ds.iter().any(|d| d.code == "W-LIN-2"),
        "typed positive control"
    );
    let a = m
        .state_variables
        .iter()
        .position(|v| v.name == "a")
        .unwrap();
    let other = relca_core::ir::TypeId(
        m.types
            .iter()
            .position(|t| t.name == "OtherVerdict")
            .unwrap(),
    );
    assert_ne!(m.state_variables[a].base, other);
    assert_eq!(
        m.types[m.state_variables[a].base.0].members,
        m.types[other.0].members
    );
    m.state_variables[a].base = other;
    if let Ok((derived, ds)) = agency::derive(&m) {
        let warnings: Vec<_> = ds.iter().filter(|d| d.code == "W-LIN-2").collect();
        assert_eq!(warnings.len(), 1, "ill-typed result must not suppress bV");
        let start = src.find("bV = B.check(e)").unwrap();
        assert_eq!(warnings[0].span.unwrap().start, start);
        assert!(
            serde_json::to_value(derived)
                .unwrap()
                .get("model_uncertainties")
                .is_none()
        );
    }
}

#[test]
fn m7_correlation_result_must_belong_to_the_same_target_group() {
    let case = boundary_cases()
        .into_iter()
        .find(|c| c.label == "foreign-group")
        .unwrap();
    assert_boundary(
        &case,
        &serde_json::to_value(report::check(&case.src)).unwrap(),
    );
}

#[test]
fn twenty_four_hand_oracles_have_exact_warning_sites_and_nonempty_optional_tables() {
    let mut seen = std::collections::BTreeSet::new();
    let mut failures = vec![];
    for name in NAMES {
        assert!(seen.insert(name));
        let s = source(name);
        let actual = serde_json::to_value(report::check(&s)).unwrap();
        if std::panic::catch_unwind(|| assert_report(name, &s, &actual)).is_err() {
            failures.push(name);
        }
    }
    assert_eq!(seen.len(), 24);
    assert!(
        failures.is_empty(),
        "all 24 were visited; failing hand oracles: {failures:?}"
    );
}

#[test]
fn old_syntax_preflight_separates_fixture_errors_from_missing_feature() {
    // Erase ONLY the two new syntax forms, not any given, reads, writes, bounds or query.
    for name in NAMES {
        let s = source(name)
            .replace(" basis \"sample X measured independent detection\"", "")
            .replace("tacit \"取引慣行の知識\"", "tacit");
        let m = lower::lower(&syntax::parse(&s).unwrap_or_else(|d| panic!("syntax {name}: {d:?}")))
            .unwrap_or_else(|d| panic!("lower {name}: {d:?}"));
        assert!(m.capabilities.iter().all(|c| c.duration > 0));
        let actual = serde_json::to_value(report::check(&s)).unwrap();
        for key in ["verdict", "bounds", "properties", "terminals"] {
            subset(&expected(name)[key], &actual[key]);
        }
    }
}

#[test]
fn basis_empty_text_unused_basis_and_tacit_bare_are_semantic_not_keyword_tests() {
    let s = source("lin-basis");
    for text in ["", "x", "arbitrary nonmeasurement text 日本語"] {
        let changed = s.replace("sample X measured independent detection", text);
        let v = serde_json::to_value(report::check(&changed)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{v}");
        assert_eq!(v["diagnostics"], json!([]));
        assert_eq!(
            v["agency"]["model_uncertainties"]["items"][0]["basis"],
            text
        );
        assert_eq!(v["terminals"], expected("lin-basis")["terminals"]);
    }
    let with_independent = s.replace(
        "kernel KA given [bad]",
        "kernel KA given [bad] basis \"unneeded\"",
    );
    let v = serde_json::to_value(report::check(&with_independent)).unwrap();
    assert_eq!(
        v["agency"]["model_uncertainties"]["items"]
            .as_array()
            .unwrap()
            .len(),
        1,
        "basis on independent KA is not raw"
    );
    let bare = source("lin-tacit").replace("tacit \"取引慣行の知識\"", "tacit");
    let v = serde_json::to_value(report::check(&bare)).unwrap();
    assert_eq!(v["diagnostics"], json!([]));
    assert!(v["agency"].get("source_descriptions").is_none());
    assert_eq!(
        v["agency"]["model_uncertainties"],
        expected("lin-tacit")["agency"]["model_uncertainties"],
        "bare tacit suppressions must not disappear without a record"
    );
}

#[test]
fn grammar_fail_closed_and_basis_remains_a_legal_identifier() {
    let s = source("lin-basis");
    for (replacement, code) in [
        ("basis \"one\" basis \"two\"", "U-SEM-0"),
        ("basis", "E-SYN-0"),
        ("unknown_header \"x\"", "U-SYN-0"),
    ] {
        let v = serde_json::to_value(report::check(&s.replace(
            "basis \"sample X measured independent detection\"",
            replacement,
        )))
        .unwrap();
        assert!(v["agency"].is_null(), "no partial success: {v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{code}: {v}"
        );
    }
    let v = serde_json::to_value(report::check(
        &source("lin-basis.broken-no-basis").replace("bV = B.check(e)", "basis = B.check(e)"),
    ))
    .unwrap();
    assert_eq!(
        v["verdict"], "satisfied",
        "basis is not globally reserved: {v}"
    );
    let bad_doc = source("lin-tacit").replace("source tacit", "source document");
    let v = serde_json::to_value(report::check(&bad_doc)).unwrap();
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "U-SYN-0")
    );
}

#[test]
fn correlation_needs_the_result_written_on_every_path_before_the_judgement() {
    // aV's result a is in bV's given. It counts only when aV has surely
    // finished before bV starts: not when aV runs in one branch of an if that
    // falls through, nor when join any may cancel aV (ADR 0024 追記 M3).
    let src = source("lin-correlation");
    let flow = "    aV = A.check(e)\n    bV = B.check(e)\n";
    assert_eq!(src.matches(flow).count(), 1);
    let fork = |remaining: &str| {
        let s = src
            .replace(
                "observed case { a, b: optional<Verdict>;",
                "observed case { a, b, z: optional<Verdict>;",
            )
            .replacen(
                "  actor P {",
                "  kernel KZ { * -> { 1: pass } }\n  actor Z { capacity 1\n    capability check(e:Entry) -> Verdict { duration fixed 1min; outcomes kernel KZ; writes z; cancel cooperative ack_within 1min }\n  }\n  actor P {",
                1,
            )
            .replace(
                "writes a }",
                "writes a; cancel cooperative ack_within 1min }",
            )
            .replace("include [S,R,P,A,B]", "include [S,R,P,A,B,Z]")
            .replace("live_tasks 2", "live_tasks 3");
        s.replace(
            flow,
            &format!(
                "    fork checks {{ aV = A.check(e); zV = Z.check(e) }}\n    join any checks where result == pass tie by [aV, zV] remaining {remaining} if none -> done\n    bV = B.check(e)\n"
            ),
        )
    };
    let cases = [
        (
            "if falls through",
            src.replace(
                flow,
                "    if c == pass { aV = A.check(e) } else { }\n    bV = B.check(e)\n",
            ),
            true,
        ),
        (
            "if else ends",
            src.replace(
                flow,
                "    if c == pass { aV = A.check(e) } else { end rejected }\n    bV = B.check(e)\n",
            ),
            false,
        ),
        (
            "join any cancel_then_drain",
            fork("cancel_then_drain"),
            true,
        ),
        ("join any drain", fork("drain"), false),
    ];
    for (name, s, warn) in cases {
        let v = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(v["verdict"], "satisfied", "{name}: {v}");
        let w: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .collect();
        let items = v["agency"]["model_uncertainties"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let correlated = items
            .iter()
            .filter(|i| i["kind"] == "wlin2_correlation" && i["call"] == "bV")
            .count();
        if warn {
            assert_eq!((w.len(), correlated), (1, 0), "{name}: {v}");
            assert!(
                w[0]["message"].as_str().unwrap().starts_with("bV "),
                "{name}"
            );
        } else {
            assert_eq!((w.len(), correlated), (0, 1), "{name}: {v}");
        }
    }
}

#[test]
fn rereading_the_subject_source_is_not_a_new_evidence_source() {
    // A reads R. If B reads S (already the subject's source), B's sources
    // {S} are contained in A's {S,R}: B warns, so a non-empty reads is not
    // enough. If B reads R too, the two are symmetric: both warn (ADR 0024
    // 追記 M1).
    let src = source("lin-basis.broken-no-basis");
    let from = "outcomes kernel KB;";
    assert_eq!(src.matches(from).count(), 1);
    for (reads, warn) in [("reads [S]; ", 1), ("reads [R]; ", 2)] {
        let s = src.replacen(from, &format!("{reads}{from}"), 1);
        let v = serde_json::to_value(report::check(&s)).unwrap();
        let n = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .count();
        assert_eq!(n, warn, "{reads}: {v}");
    }
}

#[test]
fn fork_and_utf8_warning_spans_are_exact_calls_even_without_effects() {
    let src = source("lin-basis.broken-no-basis");
    let fork = src.replace(
        "    aV = A.check(e)\n    bV = B.check(e)",
        "    fork checks { aV = A.check(e); bV = B.check(e) }\n    join all checks",
    );
    // A multibyte literal before the call shifts byte spans away from chars
    // (the grammar has no comments; a supported metadata string is used).
    let utf8 = src.replacen("actor A {", "actor A { boundary \"日本語の前置き\";", 1);
    assert_ne!(utf8, src);
    for s in [src.clone(), utf8, fork] {
        let v = serde_json::to_value(report::check(&s)).unwrap();
        let ds: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .collect();
        assert_eq!(ds.len(), 1, "{v}");
        let from = s.find("bV = B.check(e)").unwrap();
        assert_eq!(
            ds[0]["span"],
            json!({"start":from,"end":from+"bV = B.check(e)".len()})
        );
    }
}

#[test]
fn given_result_alias_constant_and_override_boundaries() {
    let s = source("lin-correlation");
    let alias = s
        .replace("a, b:", "po_state, b:")
        .replace("writes a }", "writes po_state }")
        .replace("[bad,a]", "[bad,po_state]");
    let v = serde_json::to_value(report::check(&alias)).unwrap();
    assert_eq!(
        v["diagnostics"],
        json!([]),
        "StateVarId alias, not binding spelling: {v}"
    );
    let constant = s.replace("writes a }", "writes a; finish { a = pass } }");
    let v = serde_json::to_value(report::check(&constant)).unwrap();
    assert_eq!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .count(),
        1,
        "finish constant is not a result: {v}"
    );
    let overridden=s.replace("kernel KA given [bad] { false -> { 1: pass }; true -> { 1: pass } }", "kernel KA given [bad] { false -> { 1: pass with a = fail }; true -> { 1: pass with a = fail } }");
    let v = serde_json::to_value(report::check(&overridden)).unwrap();
    assert_eq!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .count(),
        1,
        "nonmatching with override is not a result: {v}"
    );
}

#[test]
fn lexical_else_dead_calls_and_multiple_writer_targets_are_not_runtime_counts() {
    let src = source("lin-basis.broken-no-basis");
    // aV always passes, so the else branch is unreachable. Its bV call is still lexical.
    let dead = src.replace(
        "    bV = B.check(e)",
        "    if aV == fail { bV = B.check(e) } else {} ",
    );
    let v = serde_json::to_value(report::check(&dead)).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    assert_eq!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .count(),
        1,
        "dead lexical call remains a judgement: {v}"
    );
    // Two writer ancestors for each judgement: one warning, not one per writer.
    let multi = source("lin-same-source-different-target")
        .replace("check(e:Entry)", "check(e:Entry,other:Entry)")
        .replace("A.check(e1)", "A.check(e1,e2)")
        .replace("B.check(e2)", "B.check(e1,e2)");
    let v = serde_json::to_value(report::check(&multi)).unwrap();
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert_eq!(
        v["diagnostics"].as_array().unwrap().len(),
        1,
        "deduplicate call, not writer: {v}"
    );
    let basis = multi.replace(
        "kernel KB given [bad]",
        "kernel KB given [bad] basis \"two writers\"",
    );
    let v = serde_json::to_value(report::check(&basis)).unwrap();
    let u = &v["agency"]["model_uncertainties"]["items"][0];
    assert_eq!(u["targets"], json!(["e1", "e2"]));
    assert_eq!(u["error_latents"], json!(["bad"]));
}

#[test]
fn unused_kernel_empty_given_and_kernel_reads_do_not_create_causes() {
    let s = source("lin-shared-kernel.broken-drop-call");
    let unused=s.replace("  bounds {", "  kernel NeverUsed given [Invoice.difficulty] { false -> { 1: pass }; true -> { 1: pass } }\n  bounds {");
    let v = serde_json::to_value(report::check(&unused)).unwrap();
    assert_eq!(v["verdict"], "satisfied");
    assert!(v["agency"].get("common_causes").is_none());
    let s = source("lin-shared-kernel").replace(
        "given [Invoice.difficulty] { false -> { 1: pass }; true -> { 1: pass } }",
        "given [] { * -> { 1: pass } }",
    );
    let v = serde_json::to_value(report::check(&s)).unwrap();
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert!(
        v["agency"].get("common_causes").is_none(),
        "kernel_reads alone must not count"
    );
}

#[test]
fn public_invalid_ir_indices_and_cycles_fail_closed_without_panics() {
    use relca_core::{agency, ir};
    let valid =
        lower::lower(&syntax::parse(&source("lin-basis.broken-no-basis")).unwrap()).unwrap();
    let mut bads = vec![];
    let mut b = valid.clone();
    b.capabilities[2]
        .kernel
        .as_mut()
        .unwrap()
        .given
        .push(ir::StateVarId(usize::MAX));
    bads.push(b);
    let mut b = valid.clone();
    b.capabilities[0]
        .kernel_writes
        .push(ir::StateVarId(usize::MAX));
    bads.push(b);
    let mut b = valid.clone();
    b.state_variables[0].kind = ir::StateKind::ActorLatent(ir::ActorId(usize::MAX));
    bads.push(b);
    let mut b = valid.clone();
    if let ir::Stmt::Call(c) = &mut b.process.body[2].kind {
        c.args.push(ir::BindingId(usize::MAX));
    }
    bads.push(b);
    let mut b = valid.clone();
    if let ir::Stmt::Call(c) = &mut b.process.body[0].kind {
        c.args.push(c.binding);
    }
    bads.push(b);
    for (i, b) in bads.iter().enumerate() {
        let result = std::panic::catch_unwind(|| agency::derive(b));
        assert!(result.is_ok(), "malformed public IR must not panic: {i}");
        assert!(
            result.unwrap().is_err(),
            "malformed public IR must be unsupported, never checked: {i}"
        );
    }
}

#[test]
fn basis_cannot_rescue_probability_row_or_hidden_truth_errors() {
    let s = source("lin-basis");
    for (bad, code) in [
        (
            s.replace(
                "false -> { 1: pass }; true -> { 1: pass }",
                "false -> { 0.9: pass }; true -> { 1: pass }",
            ),
            "E-PROB-0",
        ),
        (
            s.replace(
                "false -> { 1: pass }; true -> { 1: pass }",
                "false -> { 1: pass }",
            ),
            "E-PROB-0",
        ),
        (
            s.replace(
                "    end done",
                "    if bad == true { end done } else { end done }",
            ),
            "E-POL-LATENT",
        ),
    ] {
        let v = serde_json::to_value(report::check(&bad)).unwrap();
        assert_eq!(v["verdict"], "invalid", "{v}");
        assert!(v["agency"].is_null());
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == code),
            "{v}"
        );
    }
}

#[test]
fn original_invoice_as_is_and_to_be_a_keep_four_calls_old_lineage_and_hand_arithmetic() {
    for (file, wrong, elapsed, work, clerk, verdict, producer, kernel) in [
        (
            "as-is.relca",
            "93/125000",
            17,
            17,
            14,
            "satisfied",
            "Clerk.enter",
            "human_entry",
        ),
        (
            "to-be-a.relca",
            "231/125000",
            9,
            12,
            8,
            "violated",
            "Extractor.extract",
            "ai_entry",
        ),
    ] {
        let src =
            std::fs::read_to_string(root().parent().unwrap().join("examples/invoice").join(file))
                .unwrap();
        let v = serde_json::to_value(report::check(&src)).unwrap();
        assert_eq!(v["verdict"], verdict);
        let causes = v["agency"]["common_causes"]["causes"].as_array().unwrap();
        assert_eq!(causes.len(), 1, "Invoice only");
        assert_eq!(
            causes[0]["targets"],
            json!([
                {"call":"entry","capability":producer,"kernel":kernel},
                {"call":"poV","capability":"Clerk.match_po","kernel":"po_match"},
                {"call":"receiptV","capability":"Clerk.match_receipt","kernel":"receipt_match"},
                {"call":"approvalV","capability":"Approver.approve","kernel":"approval_check"},
            ]),
            "selected process, never the unused fifth kernel"
        );
        let ds: Vec<_> = v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-LIN-2")
            .collect();
        assert_eq!(ds.len(), 1);
        let start = src
            .find("approvalV = Approver.approve(entry,poV,receiptV)")
            .unwrap();
        assert_eq!(
            ds[0]["span"],
            json!({"start":start,"end":start+"approvalV = Approver.approve(entry,poV,receiptV)".len()})
        );
        let line = v["agency"]["lineage"]
            .as_array()
            .unwrap()
            .iter()
            .find(|x| x["call"] == "approvalV")
            .unwrap();
        assert_eq!(
            line["subject_sources"],
            json!(["Invoice", "PurchaseOrder", "ReceiptRecord"]),
            "do not change old raw subjects to implement new grouping"
        );
        assert_eq!(line["independent_sources"], json!([]));
        let p = v["properties"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["id"] == "probability reached #1")
            .unwrap();
        assert_eq!(p["probability"], json!({"min":wrong,"max":wrong}));
        let t = v["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .find(|t| t["end"] == "posted")
            .unwrap();
        assert_eq!(t["elapsed"], json!({"min":elapsed,"max":elapsed}));
        assert_eq!(t["work"], json!({"min":work,"max":work}));
        let c = v["metrics"]["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["actor"] == "Clerk")
            .unwrap();
        assert_eq!(
            c["work"],
            json!({"min":null,"max":null,"reason":"not_configured"}),
            "the current Cut does not configure Clerk workload"
        );
        // Retain the old hand arithmetic only in the explicit no-defer
        // control; all structural assertions above still use the actual Cut.
        let control =
            serde_json::to_value(report::check(&deferred_control::without_defer(&src))).unwrap();
        let c = control["metrics"]["actors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["actor"] == "Clerk")
            .unwrap();
        assert_eq!(
            c["work"],
            json!({"min":clerk.to_string(),"max":(clerk+1).to_string(),"reason":null}),
            "returned route adds exactly one minute"
        );
        // Same successful route: Clerk clerk + Approver2 = human16/10,
        // excluding Ledger1 and (in A) Extractor1. Do not sum maxima of people.
        assert_eq!(clerk + 2, if file == "as-is.relca" { 16 } else { 10 });
    }
}

#[test]
fn runtime_clock_cut_preserves_successful_static_metadata_without_inventing_completion() {
    let src = source("lin-basis").replace("clock 30min", "clock 0min");
    let v = serde_json::to_value(report::check(&src)).unwrap();
    assert_eq!(v["bounds"]["truncated"], true, "{v}");
    assert_eq!(v["terminals"], json!([]));
    assert_eq!(
        v["agency"]["model_uncertainties"],
        expected("lin-basis")["agency"]["model_uncertainties"]
    );
    assert!(
        !v["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("W-LIN-2"))
    );
}

#[test]
fn contextual_metadata_grammar_records_only_used_suppression_and_preserves_old_names() {
    let bare = source("lin-basis.broken-no-basis");
    for s in [
        bare.replace("Entry", "basis"),
        bare.replace("make", "basis"),
        bare.replace("bV =", "basis ="),
    ] {
        let v = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(
            v["verdict"], "satisfied",
            "contextual basis must not reserve old names: {v}"
        );
    }
    let unused=source("lin-single").replace("  bounds {","  kernel Unused given [bad] basis \"not used\" { false -> { 1: pass }; true -> { 1: pass } }\n  bounds {");
    let v = serde_json::to_value(report::check(&unused)).unwrap();
    assert_eq!(v["verdict"], "satisfied", "{v}");
    assert!(
        v["agency"].get("model_uncertainties").is_none(),
        "unused kernel cannot suppress a call"
    );
    let desc = source("lin-tacit");
    for s in [
        desc.replace(
            "source tacit \"取引慣行の知識\"",
            "source tacit \"x\"; source tacit \"y\"",
        ),
        desc.replace(
            "kernel KB given [bad]",
            "kernel KB given [bad] basis \"x\" basis \"x\"",
        ),
    ] {
        let v = serde_json::to_value(report::check(&s)).unwrap();
        assert_eq!(v["verdict"], "unsupported");
        assert!(v["agency"].is_null());
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-SEM-0"),
            "duplicate declarations: {v}"
        );
    }
    for s in [
        desc.replace("source tacit", "source record"),
        bare.replace(
            "kernel KB given [bad] { false -> { 1: pass }; true -> { 1: pass } }",
            "kernel KB given [bad] basis \"x\" { }",
        ),
        bare.replace(
            "kernel KB given [bad] {",
            "kernel KB given [bad] { basis \"x\";",
        ),
    ] {
        let v = serde_json::to_value(report::check(&s)).unwrap();
        assert_ne!(v["verdict"], "satisfied");
        assert!(v["agency"].is_null());
    }
}
