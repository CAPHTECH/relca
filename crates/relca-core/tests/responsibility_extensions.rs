//! ADR 0028. Hand oracles precede implementation; no new production IR API assumed.
use relca_core::{engine, lower, reference, report, syntax};
use serde_json::{Value, json};
use std::path::PathBuf;

#[path = "support/diff_increment.rs"]
mod diff_increment;
#[path = "support/responsibility_extension_population.rs"]
mod population;
pub use population::NAMES;
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
pub fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}
pub fn row(v: &Value) -> &Value {
    &v["responsibilities"][0]
}
pub fn stripped(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("responsible "))
        .collect::<Vec<_>>()
        .join("\n")
}
pub fn subset(e: &Value, a: &Value) {
    match e {
        Value::Object(o) => {
            for (k, v) in o {
                if k == "absent_diagnostics" {
                    for code in v.as_array().unwrap() {
                        assert!(
                            !a["diagnostics"]
                                .as_array()
                                .unwrap()
                                .iter()
                                .any(|d| d["code"] == *code),
                            "unexpected {code}: {a}"
                        );
                    }
                } else {
                    subset(v, a.get(k).unwrap_or_else(|| panic!("missing {k}: {a}")));
                }
            }
        }
        Value::Array(es) => {
            let actual = a.as_array().unwrap_or_else(|| panic!("not array: {a}"));
            if es.first().is_some_and(|v| v.get("code").is_some()) {
                for hand in es {
                    let found: Vec<_> = actual
                        .iter()
                        .filter(|d| d["code"] == hand["code"])
                        .collect();
                    assert_eq!(found.len(), 1, "diagnostic multiplicity: {hand}: {a}");
                    subset(hand, found[0]);
                }
            } else {
                assert_eq!(es.len(), actual.len(), "inventory: {e} versus {a}");
                for (i, hand) in es.iter().enumerate() {
                    let other = if hand.get("end").is_some() {
                        actual.iter().find(|v| v["end"] == hand["end"]).unwrap()
                    } else {
                        &actual[i]
                    };
                    subset(hand, other);
                }
            }
        }
        _ => assert_eq!(e, a),
    }
}
pub fn failure(src: &str, code: &str) {
    let a = check(src);
    assert_eq!(
        a["verdict"],
        if code.starts_with('U') {
            "unsupported"
        } else {
            "invalid"
        },
        "{code}: {a}"
    );
    let errors: Vec<_> = a["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["severity"] == "error")
        .collect();
    assert!(!errors.is_empty());
    assert!(
        errors.iter().all(|d| d["code"] == code),
        "intended error {code}: {a}"
    );
    assert_eq!(a["properties"], json!([]));
    assert_eq!(a["terminals"], json!([]));
    assert!(a["agency"].is_null());
    assert!(a.get("responsibilities").is_none());
}

#[test]
fn fifteen_fixture_pairs_are_exactly_one_source_edit() {
    for (base, fault, from, to) in [
        ("direction", "reversed", "(r -> ok)", "(ok -> r)"),
        (
            "equivalence",
            "wrong-antecedent",
            "(r == false || ok == true)",
            "(r == true || ok == true)",
        ),
        ("bool", "enum", "r: bool = false", "r: Flag = off"),
        (
            "maintenance",
            "indirect-only",
            "{ reads [Record]; duration fixed 2min",
            "{ duration fixed 2min",
        ),
        (
            "collision",
            "capability",
            "result nondet { committed } } }",
            "result nondet { committed } }; capability maintenance() -> Ack { duration fixed 0min; result nondet { committed } } }",
        ),
        ("within", "short", "within 5min", "within 4min"),
        ("duration", "short", "within 10min", "within 9min"),
        (
            "unchecked",
            "within",
            "for Judge.check\n",
            "for Judge.check within 5min\n",
        ),
        (
            "design",
            "unknown",
            "for Extractor.maintenance",
            "for Unknown.maintenance",
        ),
        ("classification", "operator", "(r == true)", "(r > true)"),
        ("relay", "short", "within 9min", "within 8min"),
        ("unknown-origin", "no-source", "reads [Record]; ", ""),
        ("atomic", "consequent", "ok = true", "ok = false"),
        (
            "unknown-time",
            "no-direct",
            "{ reads [Record]; duration fixed 2min",
            "{ duration fixed 2min",
        ),
        ("refusal", "bypass", "end returned ", ""),
    ] {
        let good = source(&format!("resp2-{base}"));
        let bad = source(&format!("resp2-{base}.broken-{fault}"));
        assert_eq!(good.matches(from).count(), 1, "one fault site {base}");
        assert_eq!(good.replacen(from, to, 1), bad, "single source edit {base}");
    }
    let mut names: Vec<_> = std::fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("resp2-"))
        .collect();
    names.sort();
    assert_eq!(names, NAMES);
}

#[test]
fn every_fixture_old_syntax_skeleton_is_legal_before_implementation() {
    for name in NAMES {
        let a = check(&stripped(&source(name)));
        assert_eq!(a["verdict"], "satisfied", "old syntax skeleton {name}: {a}");
        assert_eq!(a["bounds"]["truncated"], false, "{name}");
        assert!(
            !a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["severity"] == "error"),
            "{name}: {a}"
        );
    }
}

#[test]
fn all_thirty_hand_fixture_expectations() {
    for name in NAMES {
        let a = check(&source(name));
        eprintln!("hand fixture {name}");
        subset(&expected(name), &a);
    }
}

fn ou(v: &Value) -> Value {
    json!({"observation":row(v)["observation"],"refusal":row(v)["refusal"],"warnings":row(v)["warnings"]})
}
fn known_ou(actor: &str, effect: &str, direct: bool) -> Value {
    json!({"observation":{"status":if direct {"present"} else {"missing"},"required_sources":["Record"],"direct":if direct {vec!["Record"]} else {vec![]},"indirect":["Record"],"missing_sources":if direct {vec![]} else {vec!["Record"]},"reason":null},
        "refusal":{"status":"present","effects":[{"capability":effect,"call":"out","status":"all_paths","judgements":[{"actor":actor,"capability":format!("{actor}.check"),"call":"j","stops":"all_paths","bypasses":0}]}],"reason":null},"warnings":if direct {vec![]} else {vec!["W-RESP-1"]}})
}

#[test]
fn implication_equivalence_and_bare_bool_have_complete_ou_equality() {
    let implication = check(&source("resp2-direction"));
    assert_eq!(ou(&implication), known_ou("Guard", "SetR.put", true));
    assert_eq!(ou(&implication), ou(&check(&source("resp2-equivalence"))));
    assert_ne!(
        ou(&implication),
        ou(&check(&source("resp2-equivalence.broken-wrong-antecedent")))
    );
    let src = source("resp2-bool");
    assert_eq!(
        ou(&check(&src)),
        ou(&check(&src.replace("for (r)", "for (r == true)")))
    );
    for (r, ok, truth) in [
        (false, false, true),
        (false, true, true),
        (true, false, false),
        (true, true, true),
    ] {
        assert_eq!(!r || ok, truth, "independent four-row oracle");
        // Constant atomic poststates discriminate implication from conjunction,
        // reversed implication, and result-by-result partial assignments.
        let src = source("resp2-atomic")
            .replace("r = true", &format!("r = {r}"))
            .replace("ok = true", &format!("ok = {ok}"));
        let a = check(&src);
        assert_eq!(
            row(&a)["refusal"]["effects"]
                .as_array()
                .expect("accepted implication")
                .len(),
            usize::from(!truth)
        );
    }
}

#[test]
fn maintenance_preserves_effect_identity_full_ou_and_unknown_o_does_not_hide_u() {
    for name in ["resp2-maintenance", "resp2-collision", "resp2-refusal"] {
        assert_eq!(
            ou(&check(&source(name))),
            known_ou("Judge", "Effect.put", true),
            "{name}"
        );
    }
    assert_eq!(
        ou(&check(&source("resp2-maintenance.broken-indirect-only"))),
        known_ou("Judge", "Effect.put", false)
    );
    let good = check(&source("resp2-unknown-origin"));
    let bad = check(&source("resp2-unknown-origin.broken-no-source"));
    assert_eq!(
        row(&good)["refusal"],
        row(&bad)["refusal"],
        "unknown O must not erase known U"
    );
    assert_eq!(row(&bad)["refusal"]["status"], "missing");
    assert_eq!(row(&bad)["refusal"]["effects"][0]["capability"], "Bad.set");
    let collision = source("resp2-collision.broken-capability");
    failure(&collision, "E-CON-0");
    let ordinary = stripped(&collision).replace(
        "e = Reader.read();",
        "unused = Effect.maintenance(); e = Reader.read();",
    );
    assert_eq!(
        check(&ordinary)["verdict"],
        "satisfied",
        "maintenance is not globally reserved"
    );
}

#[test]
fn time_exact_details_and_mixed_unchecked_are_not_runtime_contracts() {
    for name in [
        "resp2-within",
        "resp2-within.broken-short",
        "resp2-duration",
        "resp2-duration.broken-short",
        "resp2-relay",
        "resp2-relay.broken-short",
        "resp2-unknown-time.broken-no-direct",
        "resp2-refusal.broken-bypass",
    ] {
        let a = check(&source(name));
        assert_eq!(
            row(&a)["time_frame"],
            expected(name)["responsibilities"][0]["time_frame"],
            "complete time oracle {name}"
        );
        assert_eq!(
            a["verdict"], "satisfied",
            "time is not a root contract {name}"
        );
    }
    let old = check(&source("resp2-unchecked"));
    assert_eq!(row(&old)["time_frame"], json!({"status":"unchecked"}));
    let src = source("resp2-within");
    let all = check(&src);
    assert!(
        !all["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_time_frame"))
    );
    let mixed = check(&src.replace(
        "  process Flow",
        "  responsible q for Effect.maintenance\n  process Flow",
    ));
    assert_eq!(mixed["responsibilities"][0], all["responsibilities"][0]);
    assert_eq!(
        mixed["responsibilities"][1]["time_frame"],
        json!({"status":"unchecked"})
    );
    assert!(
        mixed["agency"]["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility_time_frame"))
    );
    let zero = src
        .replace("fixed 1min", "fixed 0min")
        .replace("fixed 2min", "fixed 0min")
        .replace("fixed 3min", "fixed 0min")
        .replace("within 5min", "within 0min");
    assert_eq!(
        row(&check(&zero))["time_frame"]["status"],
        "satisfied",
        "zero is a known duration"
    );
    for unit in ["1h", "1d"] {
        let a = check(&src.replace("within 5min", &format!("within {unit}")));
        assert_eq!(
            row(&a)["time_frame"]["within"],
            if unit == "1h" { 60 } else { 1440 }
        );
    }
    let distribution = source("resp2-duration").replace("7min:0.25}", "7min:0.25,99min:0}");
    assert_eq!(
        row(&check(&distribution))["time_frame"],
        expected("resp2-duration")["responsibilities"][0]["time_frame"]
    );
}

pub fn rejection_inputs() -> Vec<(String, String, &'static str)> {
    let b = source("resp2-bool");
    let t = source("resp2-within");
    let mut out = Vec::new();
    for (label, from, to, code) in [
        (
            "optional",
            "r: bool = false",
            "r: optional<bool>",
            "E-TYPE-0",
        ),
        ("latent", "observed case", "latent case", "E-TYPE-0"),
        (
            "enum-names",
            "type Flag = enum { off,on }",
            "type Flag = enum { true,false }",
            "E-TYPE-0",
        ),
        ("unknown-bool", "for (r)", "for (unknown)", "E-NAME-0"),
        (
            "missing-for",
            "responsible p for (r)",
            "responsible p (r)",
            "E-SYN-0",
        ),
        ("missing-target", "for (r)", "for", "E-SYN-0"),
        ("missing-operand", "for (r)", "for (r -> )", "E-SYN-0"),
        ("missing-eq-rhs", "for (r)", "for (r == )", "E-SYN-0"),
        ("missing-close", "for (r)", "for (r -> r", "E-SYN-0"),
        ("chain", "for (r)", "for (r -> r -> r)", "U-SYN-0"),
        ("precedence", "for (r)", "for (r && r -> r)", "U-SYN-0"),
        ("operator", "for (r)", "for (r > true)", "U-SYN-0"),
        ("negation", "for (r)", "for (!r)", "U-SYN-0"),
    ] {
        let mut s = b.replace(from, to);
        if label == "enum-names" {
            s = s.replace("r: bool = false", "r: Flag = false");
        }
        out.push((label.into(), s, code));
    }
    for (label, from, to, code) in [
        ("missing-duration", "within 5min", "within", "E-SYN-0"),
        (
            "missing-cap",
            "Judge.check within 5min",
            "Judge.",
            "E-SYN-0",
        ),
        (
            "event-target",
            "Judge.check within 5min",
            "Judge.check.commit",
            "U-SYN-0",
        ),
        (
            "overflow",
            "within 5min",
            "within 18446744073709551615d",
            "U-SEM-0",
        ),
        (
            "duplicate",
            "for Judge.check within 5min",
            "for Judge.check within 5min\n  responsible p for Judge.check within 4min",
            "U-SEM-0",
        ),
    ] {
        out.push((label.into(), t.replace(from, to), code));
    }
    out
}

#[test]
fn dedicated_syntax_type_name_and_missing_token_boundaries() {
    for (label, s, code) in rejection_inputs() {
        eprintln!("boundary {label}");
        failure(&s, code);
    }
    let b = source("resp2-bool");
    for target in ["((r && r) -> r)", "(r -> (r || r))", "(r -> (r -> r))"] {
        for src in [
            b.replace("(r)", target),
            b.replace("(r)", target).replace(' ', "\n"),
            b.replace("(r)", target).replace('\n', "\r\n"),
        ] {
            assert_eq!(
                check(&src)["verdict"],
                "satisfied",
                "grouping and newline: {src}"
            );
        }
    }
    let s = source("resp2-classification.broken-operator");
    let a = check(&s);
    let p = s.find("r > true").unwrap() + 2;
    assert_eq!(a["diagnostics"][0]["span"], json!({"start":p,"end":p+1}));
    // Comments are not in the language; put multi-byte text in an existing
    // string field before the error so the span must count UTF-8 bytes.
    let insert = "  metrics { window 10min; currency \"日本円\" }\n";
    let at = s.find('{').unwrap() + 2;
    let a = check(&format!("{}{insert}{}", &s[..at], &s[at..]));
    assert_eq!(
        a["diagnostics"][0]["span"],
        json!({"start":p+insert.len(),"end":p+insert.len()+1})
    );
}

#[test]
fn shared_if_and_invariant_grammar_do_not_gain_responsibility_formulas() {
    let src = stripped(&source("resp2-within"));
    for condition in ["r", "r -> r"] {
        let a = check(&src.replace("if j == pass", &format!("if {condition}")));
        assert_ne!(
            a["verdict"], "satisfied",
            "process if must not gain {condition}"
        );
        assert!(
            a["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["severity"] == "error")
        );
        let a = check(&src.replace(
            "  verify safety",
            &format!(
                "  contract C {{ invariant Effect.put.commit -> {condition} }}\n  verify safety"
            ),
        ));
        assert_ne!(
            a["verdict"], "satisfied",
            "invariant postcondition must not gain {condition}"
        );
    }
    let bound = source("resp2-within").replace("for Judge.check within 5min", "for (j)");
    failure(&bound, "U-SEM-0");
    let duplicate = source("resp2-bool").replace(
        "  process Flow",
        "  responsible p for (r == true)\n  process Flow",
    );
    failure(&duplicate, "U-SEM-0");
}

#[test]
fn contradiction_is_not_broken_and_atomic_writes_are_not_split() {
    for name in ["resp2-atomic", "resp2-atomic.broken-consequent"] {
        let src = source(name).replace("(r -> ok)", "(r == true && r == false)");
        let a = check(&src);
        assert_eq!(
            row(&a)["refusal"]["effects"],
            json!([]),
            "never-true norm has no true-to-false transition"
        );
    }
    assert_eq!(
        row(&check(&source("resp2-atomic")))["refusal"]["effects"],
        json!([])
    );
    assert_eq!(
        row(&check(&source("resp2-atomic.broken-consequent")))["refusal"]["effects"][0]["capability"],
        "SetBoth.put"
    );
}

/// Extra hand-model population. Every member has an old-syntax skeleton check
/// below, so unsupported DSL unrelated to ADR 0028 cannot masquerade as RED.
pub fn matrix_inputs() -> Vec<(String, String)> {
    let union=source("resp2-direction")
        .replace("  actor Record { source record }","  actor Record { source record }\n  actor Other { source document }\n  actor SetOk { capacity 1; capability emit() -> Ack { commits; reads [Other]; writes ok; finish { ok = true }; duration fixed 1min; result nondet { committed } } }")
        .replace("capability check(e:Entry) -> Verdict { reads [Record]","capability check(e:Entry) -> Verdict { reads [Record,Other]")
        .replace("capability read()","capability read(s:Ack)")
        .replace("e = Reader.read();","doneOk = SetOk.emit(); e = Reader.read(doneOk);")
        .replace("include [Record,Reader,Guard,SetR]","include [Record,Other,SetOk,Reader,Guard,SetR]");
    let zero=source("resp2-atomic")
        .replace("  actor Record", "  kernel K { * -> { 1: committed with r = true, ok = true; 0: committed with r = true, ok = false } }\n  actor Record")
        .replace("finish { r = true; ok = true }; ","")
        .replace("result nondet { committed }","outcomes kernel K");
    let paths=source("resp2-relay")
        .replace("  actor Record", "  actor Choice { capacity 1; capability pick() -> Verdict { duration fixed 1min; result nondet { pass,fail } } }\n  actor Record")
        .replace("e = Reader.read();","c = Choice.pick(); e = Reader.read();")
        .replace("relay = Relay.forward(j); out = Effect.put(relay); end done", "if c == pass { short = Effect.put(j); end done } else { relay = Relay.forward(j); long = Effect.put(relay); end done }")
        .replace("include [Record,Reader,Judge,Relay,Effect]","include [Choice,Record,Reader,Judge,Relay,Effect]");
    let repeat = source("resp2-relay")
        .replace(
            "relay = Relay.forward(j); out = Effect.put(relay)",
            "relay = Relay.forward(j); again = Relay.forward(relay); out = Effect.put(again)",
        )
        .replace("within 9min", "within 13min");
    let unused=source("resp2-maintenance")
        .replace("  actor Record", "  actor Other { source document }\n  actor Record")
        .replace("result nondet { committed } } }","result nondet { committed } }; capability unused() -> Ack { commits; reads [Other]; duration fixed 0min; result nondet { committed } } }")
        .replace("include [Record,Reader,Judge,Effect]","include [Other,Record,Reader,Judge,Effect]");
    let two_effects = source("resp2-maintenance")
        .replace("if j == pass", "unprotected = Effect.put(j); if j == pass");
    vec![
        ("union".into(), union.clone()),
        (
            "union-missing".into(),
            union.replace("reads [Record,Other]", "reads [Record]"),
        ),
        ("zero-mass".into(), zero.clone()),
        (
            "positive-mass".into(),
            zero.replace("1: committed", "0.5: committed")
                .replace("0: committed", "0.5: committed"),
        ),
        ("two-paths".into(), paths),
        ("repeat-capability".into(), repeat),
        ("unused-reads".into(), unused),
        ("two-effects".into(), two_effects),
    ]
}

#[test]
fn every_extra_matrix_skeleton_is_legal_before_implementation() {
    for (label, src) in matrix_inputs() {
        let a = check(&stripped(&src));
        assert_eq!(a["verdict"], "satisfied", "matrix preflight {label}: {a}");
    }
}

#[test]
fn support_union_zero_mass_max_paths_repeated_calls_and_maintenance_effect_fold() {
    for (label, src) in matrix_inputs() {
        let a = check(&src);
        assert_eq!(a["verdict"], "satisfied", "{label}: {a}");
        let duty = row(&a);
        match label.as_str() {
            "union" | "union-missing" => {
                let missing = label == "union-missing";
                assert_eq!(
                    duty["observation"],
                    json!({"status":if missing {"missing"} else {"present"},"required_sources":["Record","Other"],"direct":if missing {vec!["Record"]} else {vec!["Record","Other"]},"indirect":["Record","Other"],"missing_sources":if missing {vec!["Other"]} else {vec![]},"reason":null})
                );
            }
            "zero-mass" => assert_eq!(
                duty["refusal"]["effects"],
                json!([]),
                "zero probability is not a possible breaking write"
            ),
            "positive-mass" => {
                assert_eq!(duty["refusal"]["effects"][0]["capability"], "SetBoth.put")
            }
            "two-paths" => {
                assert_eq!(duty["time_frame"]["status"], "satisfied");
                assert_eq!(
                    duty["time_frame"]["required_duration"], 9,
                    "max(5,9), not sum14 or shortest5"
                );
                assert_eq!(
                    duty["time_frame"]["paths"][0]["calls"],
                    json!(["j", "relay", "long"])
                );
            }
            "repeat-capability" => {
                assert_eq!(
                    duty["time_frame"]["required_duration"], 13,
                    "2+4+4+3, not CapabilityId dedup9"
                );
                assert_eq!(
                    duty["time_frame"]["paths"][0]["calls"],
                    json!(["j", "relay", "again", "out"])
                );
            }
            "unused-reads" => {
                assert_eq!(
                    duty["observation"]["required_sources"],
                    json!(["Other", "Record"]),
                    "uncalled declaration reads remain required"
                );
                assert_eq!(duty["observation"]["missing_sources"], json!(["Other"]));
                assert_eq!(
                    duty["refusal"]["effects"].as_array().unwrap().len(),
                    1,
                    "only this Actor's called effects"
                );
                assert_eq!(duty["refusal"]["effects"][0]["capability"], "Effect.put");
            }
            "two-effects" => {
                assert_eq!(
                    duty["refusal"]["status"], "missing",
                    "one protected effect cannot hide another unprotected one"
                );
                assert_eq!(duty["refusal"]["effects"].as_array().unwrap().len(), 2);
                assert_eq!(duty["refusal"]["effects"][0]["call"], "unprotected");
                assert_eq!(duty["refusal"]["effects"][0]["status"], "none");
                assert_eq!(duty["refusal"]["effects"][1]["call"], "out");
                assert_eq!(duty["refusal"]["effects"][1]["status"], "all_paths");
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn reports_explain_simulate_diff_compare_and_mutate_transport_same_static_rows() {
    let src = source("resp2-within.broken-short");
    let a = check(&src);
    assert_eq!(a["verdict"], "satisfied");
    let explain = serde_json::to_value(relca_core::explain::explain(&src)).unwrap();
    assert_eq!(explain["responsibilities"], a["responsibilities"]);
    assert_eq!(
        explain["explanation"]["counterexamples"],
        json!([]),
        "static time violation is not a runtime trace"
    );
    let sim = src.replace(
        "  bounds",
        "  simulation replications 2 seed 0 confidence 0.5\n  bounds",
    );
    let sim = serde_json::to_value(report::simulate(&sim)).unwrap();
    assert_eq!(sim["responsibilities"], a["responsibilities"]);
    let configured=src.replace("  bounds","  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Flow }\n  bounds");
    let d = serde_json::to_value(relca_core::diff::diff(&configured, "A", "B")).unwrap();
    let c = serde_json::to_value(relca_core::compare::compare(
        &configured,
        &["A", "B"],
        relca_core::compare::Analysis::Exhaustive,
    ))
    .unwrap();
    for nested in [
        &d["from"]["report"],
        &d["to"]["report"],
        &c["plans"][0]["report"],
        &c["plans"][1]["report"],
    ] {
        assert_eq!(nested["responsibilities"], a["responsibilities"]);
    }
    assert!(
        c["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("responsibility"))
    );
    // ADR 0031: diff computes responsibility once authority is compared. The
    // pre-ADR diff list at 85c5485 is projected: old minus REMOVE_UNCHECKED.
    const OLD_DIFF_UNCHECKED: [&str; 10] = [
        "time_intent",
        "resource",
        "meaning",
        "authority",
        "visibility",
        "contest",
        "preserve",
        "override",
        "maintain",
        "responsibility",
    ];
    assert_eq!(
        d["unchecked"],
        diff_increment::migrated_unchecked(&OLD_DIFF_UNCHECKED)
    );
    let m = serde_json::to_value(relca_core::mutation::mutate(&src)).unwrap();
    assert_eq!(m["original"]["responsibilities"], a["responsibilities"]);
    // MutationReport nests executed reports at mutations[i].mutants[j].report.
    let mut executed = 0;
    for mutant in m["mutations"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|k| k["mutants"].as_array().unwrap())
    {
        if let Some(r) = mutant
            .get("report")
            .filter(|r| !r.is_null() && r["agency"].is_object())
        {
            assert_eq!(r["responsibilities"].as_array().unwrap().len(), 1);
            assert_eq!(r["responsibilities"][0]["time_frame"]["within"], 4);
            executed += 1;
        }
    }
    assert!(executed > 0, "mutant reports must actually be exercised");
}

#[test]
fn forged_time_status_number_and_path_fail_closed_in_both_public_ir_entries() {
    let src = source("resp2-within");
    let model =
        lower::lower(&syntax::parse(&src).expect("ADR 0028 syntax")).expect("ADR 0028 lower");
    for (pointer, value) in [
        ("/status", json!("violated")),
        ("/required_duration", json!(4)),
        ("/paths/0/calls", json!(["out", "j"])),
        ("/paths/0/duration_upper", json!(4)),
        ("/within", json!(4)),
    ] {
        let mut bad = model.clone();
        let time = &mut bad.responsibility.as_mut().unwrap().responsibilities[0].time_frame;
        let mut wire = serde_json::to_value(&*time).unwrap();
        *wire.pointer_mut(pointer).unwrap() = value;
        *time = serde_json::from_value(wire).unwrap();
        let e = engine::explore(&bad);
        let r = reference::explore(&bad);
        assert_eq!(
            r.verdict(),
            relca_core::Verdict::Unsupported,
            "reference forged {pointer}"
        );
        assert!(e.terminals.is_empty(), "forged {pointer} cannot execute");
        for ds in [&e.diagnostics, &r.outcome.diagnostics] {
            assert!(ds.iter().any(|d| d.code == "U-SEM-0"));
        }
    }
}
