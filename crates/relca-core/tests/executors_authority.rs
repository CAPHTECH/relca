//! ADR 0011 acceptance. Expectations come from the ADR and hand paths, never
//! from either interpreter. No new AST/IR shape is assumed by these tests.
use relca_core::{Verdict, diff, engine, lower, report, syntax};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{collections::BTreeMap, fs, path::Path};
#[path = "support/diff_increment.rs"]
mod diff_increment;

const SAFETY: &str = "verify safety no_resource_overcommit";
const CANCEL: &str = include_str!("../../../fixtures/exec-cancel/model.relca");
const AUTHORITY: &str = include_str!("../../../fixtures/exec-authority/model.relca");
const MISSING_GRANT: &str =
    include_str!("../../../fixtures/exec-authority.broken-missing-grant/model.relca");
const CONFIGURED: &str = include_str!("../../../fixtures/exec-vacuous/model.relca");

fn check(source: &str) -> Value {
    serde_json::to_value(report::check(source)).unwrap()
}

fn has_code(value: &Value, code: &str) -> bool {
    value["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|d| d["code"] == code)
}

fn clauses(value: &Value, expected: &[&str]) {
    let actual: Vec<_> = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["id"].as_str().unwrap().starts_with("contract "))
        .map(|p| p["verdict"].as_str().unwrap())
        .collect();
    assert_eq!(actual, expected, "{value}");
}

fn failure(source: &str, code: &str) {
    let value = check(source);
    assert!(has_code(&value, code), "missing {code}: {source}\n{value}");
    assert_eq!(
        value["verdict"],
        if code.starts_with("E-") {
            "invalid"
        } else {
            "unsupported"
        }
    );
    assert_eq!(value["properties"], json!([]));
    assert_eq!(value["terminals"], json!([]));
    assert_eq!(value["bounds"]["truncated"], Value::Null);
    for diagnostic in value["diagnostics"].as_array().unwrap() {
        if diagnostic["code"] != code {
            continue;
        }
        assert_eq!(diagnostic["severity"], "error");
        let start = diagnostic["span"]["start"].as_u64().unwrap() as usize;
        let end = diagnostic["span"]["end"].as_u64().unwrap() as usize;
        assert!(start <= end && end <= source.len());
        assert!(source.is_char_boundary(start) && source.is_char_boundary(end));
    }
}

fn model(process: &str, contract: &str) -> String {
    format!(
        r#"model M version "0.2" {{
      type T = enum {{ ok, bad }}
      people [p,q]
      actor A {{ executors [p]; capability a() -> T {{ commits; duration fixed 1min; result nondet {{ ok }} }} }}
      actor B {{ executors [q]; capability b() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
      authority G {{ basis "根拠"; grants p to A.a }}
      process P {{ {process} }}
      bounds {{ cases 1; live_tasks 2; clock 20min }}
      {SAFETY}
      contract Controls {{ {contract} }}
    }}"#
    )
}

fn rows(value: &Value, key: &str, id: &str) -> BTreeMap<String, Value> {
    let array = value[key].as_array().unwrap();
    let rows: BTreeMap<_, _> = array
        .iter()
        .map(|r| (r[id].as_str().unwrap().to_owned(), r.clone()))
        .collect();
    assert_eq!(rows.len(), array.len(), "duplicate {key}");
    rows
}

fn hand_report(actual: &Value, expected: &Value) {
    assert_eq!(ledger_shape::original_field_count(actual), 13);
    for key in ["verdict", "bounds", "mode", "unit"] {
        assert_eq!(actual[key], expected[key], "{key}: {actual}");
    }
    for (key, id) in [("properties", "id"), ("terminals", "end")] {
        assert_eq!(
            rows(actual, key, id),
            rows(expected, key, id),
            "{key}: {actual}"
        );
    }
    for diagnostic in expected["diagnostics"].as_array().into_iter().flatten() {
        assert!(
            has_code(actual, diagnostic["code"].as_str().unwrap()),
            "{actual}"
        );
    }
    for code in expected["absent_diagnostics"]
        .as_array()
        .into_iter()
        .flatten()
    {
        assert!(!has_code(actual, code.as_str().unwrap()), "{actual}");
    }
}

fn fixture(name: &str) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures")
        .join(name);
    let source = fs::read_to_string(dir.join("model.relca")).unwrap();
    let expected: Value =
        serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap()).unwrap();
    if name.starts_with("exec-vacuous") {
        let actual = serde_json::to_value(diff::diff(&source, "AsIs", "ToBeA")).unwrap();
        assert_eq!(actual["verdict"], expected["verdict"], "{actual}");
        for (side, selection) in [("from", "AsIs"), ("to", "ToBeA")] {
            hand_report(&actual[side]["report"], &expected[side]["report"]);
            hand_report(
                &serde_json::to_value(report::check_configuration(&source, selection)).unwrap(),
                &expected[side]["report"],
            );
        }
        for (key, id) in [("properties", "id"), ("terminals", "end")] {
            assert_eq!(rows(&actual, key, id), rows(&expected, key, id));
        }
        let differences = |v: &Value| -> BTreeMap<(String, String), Value> {
            let array = v["differences"].as_array().unwrap();
            let rows: BTreeMap<_, _> = array
                .iter()
                .map(|d| {
                    (
                        (
                            d["kind"].as_str().unwrap().into(),
                            d["target"].as_str().unwrap().into(),
                        ),
                        d.clone(),
                    )
                })
                .collect();
            assert_eq!(rows.len(), array.len());
            rows
        };
        assert_eq!(differences(&actual), differences(&expected));
        // ADR 0031: the old diagnostics stay empty; only the enumerated
        // W-DIFF-1 / W-DIFF-3 increments for the rows are added.
        assert_eq!(
            diff_increment::old_diagnostics(&actual, &[]),
            Vec::<Value>::new()
        );
        assert_eq!(actual["mismatch"], Value::Null);
    } else {
        hand_report(&check(&source), &expected);
    }
}

macro_rules! fixtures {
    ($($test:ident => $name:literal),+ $(,)?) => {$(#[test] fn $test() { fixture($name); })+};
}
fixtures! {
    f1_separation => "exec-separation",
    f1_shared_approver => "exec-separation.broken-shared-approver",
    f2_permitted => "exec-permitted",
    f2_narrow_permit => "exec-permitted.broken-narrow-permit",
    f3_authority => "exec-authority",
    f3_missing_grant => "exec-authority.broken-missing-grant",
    f4_shared_person => "exec-shared-person",
    f4_reused_person => "exec-shared-person.broken-reused-person",
    f5_vacuous => "exec-vacuous",
    f5_shared_approver => "exec-vacuous.broken-shared-approver",
    f6_named => "exec-anonymous",
    f6_anonymous => "exec-anonymous.broken-no-executor",
    f7_cancel => "exec-cancel",
    f7_ignore_cancel => "exec-cancel.broken-ignore-cancel",
}

#[test]
fn e_syntax_context_forward_declarations_and_contract_only() {
    let base = model(
        "x = A.a(); y = B.b(); end done",
        "separation A.a.finish, B.b.finish; permitted A.a; authority_basis A.a G",
    );
    for source in [
        base.clone(),
        base.replace(';', "\n"),
        base.replace(SAFETY, ""),
        base.replace("people [p,q]", "")
            .replace("bounds {", "people [p,q]\nbounds {"),
    ] {
        let value = check(&source);
        assert_eq!(value["verdict"], "satisfied", "{value}");
        clauses(&value, &["satisfied"; 3]);
    }
    // G already follows its first reference in this variant.
    let forward = base.replace("authority G { basis \"根拠\"; grants p to A.a }", "");
    let forward = forward
        .rsplit_once('}')
        .map(|(s, _)| format!("{s} authority G {{ basis \"根拠\"; grants [p] to A.a }} }}"))
        .unwrap();
    assert_eq!(check(&forward)["verdict"], "satisfied");
    for word in [
        "people",
        "executors",
        "permit",
        "authority",
        "basis",
        "grants",
        "issued_by",
        "not_by",
        "to",
        "separation",
        "permitted",
        "authority_basis",
        "scope",
        "expires",
        "revocable_by",
        "delegated_from",
    ] {
        for source in [
            base.replace("actor A", &format!("actor {word}"))
                .replace("A.a", &format!("{word}.a")),
            base.replace("[p,q]", &format!("[{word},q]"))
                .replace("[p]", &format!("[{word}]"))
                .replace("grants p to", &format!("grants {word} to")),
            base.replace("x =", &format!("{word} =")),
        ] {
            assert_eq!(
                check(&source)["verdict"],
                "satisfied",
                "context {word}: {source}"
            );
        }
    }
    failure(
        &base.replace("actor A", "actor by").replace("A.a", "by.a"),
        "E-SYN-0",
    );
    failure(&model("end done", "").replace(SAFETY, ""), "U-SEM-0");
}

#[test]
fn e_static_matrix_names_capacity_and_duplicate_declarations() {
    let base = model("end done", "");
    for (from, to, code) in [
        ("[p]", "[missing]", "E-NAME-0"),
        ("people [p,q]", "", "E-NAME-0"),
        ("people [p,q]", "people [p,q]; people [p,q]", "U-SEM-0"),
        ("people [p,q]", "people [p,p,q]", "U-SEM-0"),
        ("executors [p]", "executors [p,p]", "U-SEM-0"),
        ("executors [p]", "executors [p]; executors [q]", "U-SEM-0"),
        ("executors [p]", "capacity 0; executors [p]", "E-CAP-0"),
        ("executors [p]", "capacity 2; executors [p]", "E-CAP-0"),
        ("executors [p]", "", "E-CAP-0"),
        ("basis \"根拠\";", "", "E-SYN-0"),
        ("grants p to A.a", "", "E-SYN-0"),
        (
            "basis \"根拠\"",
            "basis \"根拠\"; basis \"again\"",
            "U-SEM-0",
        ),
        ("grants p to A.a", "grants [p,p] to A.a", "U-SEM-0"),
        ("grants p to A.a", "grants missing to A.a", "E-NAME-0"),
        ("grants p to A.a", "grants p to Missing.a", "E-NAME-0"),
        ("grants p to A.a", "grants p to A.missing", "E-NAME-0"),
        (
            "grants p to A.a",
            "grants p to A.a; issued_by missing",
            "E-NAME-0",
        ),
        (
            "grants p to A.a",
            "grants p to A.a; not_by [missing]",
            "E-NAME-0",
        ),
        (
            "grants p to A.a",
            "grants p to A.a; not_by [q,q]",
            "U-SEM-0",
        ),
        (
            "grants p to A.a",
            "grants p to A.a; not_by [q]; not_by [q]",
            "U-SEM-0",
        ),
        (
            "grants p to A.a",
            "grants p to A.a; issued_by p; issued_by q",
            "U-SEM-0",
        ),
        ("grants p to A.a", "grants p to A.a; not_by [p]", "E-CON-0"),
        (
            "grants p to A.a",
            "grants p to A.a; issued_by q; not_by [q]",
            "E-CON-0",
        ),
        (
            "process P",
            "authority G { basis \"other\"; grants q to B.b }; process P",
            "U-SEM-0",
        ),
    ] {
        failure(&base.replace(from, to), code);
    }
    for permit in [
        "permit missing to A.a",
        "permit p to Missing.a",
        "permit p to A.missing",
    ] {
        failure(
            &base.replace("process P", &format!("{permit}; process P")),
            "E-NAME-0",
        );
    }
    failure(
        &base.replace("process P", "permit [p,p] to A.a; process P"),
        "U-SEM-0",
    );
    for (clause, code) in [
        ("separation Missing.a.finish, B.b.finish", "E-NAME-0"),
        ("separation A.a.finish, Missing.b.finish", "E-NAME-0"),
        ("separation A.missing.finish, B.b.finish", "E-NAME-0"),
        ("permitted Missing.a", "E-NAME-0"),
        ("permitted A.missing", "E-NAME-0"),
        ("authority_basis Missing.a G", "E-NAME-0"),
        ("authority_basis A.missing G", "E-NAME-0"),
        ("authority_basis A.a Missing", "E-NAME-0"),
        ("authority_basis B.b G", "E-CON-0"),
        ("separation A.a.finish, A.a.finish", "E-CON-0"),
        ("separation A.a.finish(ok), A.a.finish(ok)", "E-CON-0"),
        ("separation A.a.commit, A.a.commit", "E-CON-0"),
        ("separation A.a.start, B.b.commit", "E-CON-0"),
        ("separation A.a.finish(nope), B.b.finish", "E-TYPE-0"),
    ] {
        failure(&model("end done", clause), code);
    }
    // All declaration references are checked even outside the selected flow.
    let selected = serde_json::to_value(report::check_configuration(
        &CONFIGURED.replace("executors [c1]", "executors [missing]"),
        "ToBeA",
    ))
    .unwrap();
    assert!(has_code(&selected, "E-NAME-0"));
}

#[test]
fn e_static_matrix_supported_not_overrestricted() {
    let base = model(
        "x = A.a(); end done",
        "permitted A.a; authority_basis A.a G",
    );
    for source in [
        base.clone(),
        base.replace("executors [p]", "capacity 1; executors [p]"),
        base.replace(
            "grants p to A.a",
            "grants p to A.a; grants [p,q] to A.a; issued_by p",
        ),
        base.replace("grants p to A.a", "grants [p,q] to A.a"),
        base.replace("basis \"根拠\"", "basis \"\""),
        base.replace(
            "process P",
            "permit [p,q] to A.a; permit p to A.a; process P",
        ),
        base.replace("actor B { executors [q]", "actor B { capacity 1"),
        base.replace("people [p,q]", "people [p,q,A,done,x,G]"),
    ] {
        assert_eq!(check(&source)["verdict"], "satisfied", "{source}");
    }
    let anonymous_grant = model("end done", "")
        .replace("actor B { executors [q]", "actor B { capacity 1")
        .replace("grants p to A.a", "grants p to A.a; grants q to B.b");
    assert_eq!(check(&anonymous_grant)["verdict"], "satisfied");
}

#[test]
fn e_anonymous_targets_rejected_individually_even_uncalled() {
    for clause in [
        "separation A.a.finish, B.b.finish",
        "permitted A.a",
        "authority_basis A.a G",
    ] {
        for process in ["end done", "x = A.a(); end done"] {
            failure(
                &model(process, clause).replace("executors [p]", "capacity 1"),
                "U-SEM-0",
            );
        }
    }
    failure(
        &model("end done", "separation A.a.finish, B.b.finish")
            .replace("executors [q]", "capacity 1"),
        "U-SEM-0",
    );
    failure(
        &model("end done", "")
            .replace("executors [p]", "capacity 1")
            .replace("process P", "permit p to A.a; process P"),
        "U-SEM-0",
    );
}

#[test]
fn e_syntax_malformed_and_outside_forms_fail_closed() {
    let base = model("end done", "");
    for (from, to) in [
        ("people [p,q]", "people []"),
        ("people [p,q]", "people p"),
        ("people [p,q]", "people [p,q"),
        ("executors [p]", "executors []"),
        ("executors [p]", "executors p"),
        ("executors [p]", "executors [p"),
        ("grants p to A.a", "grants [] to A.a"),
        ("grants p to A.a", "grants p A.a"),
        ("grants p to A.a", "grants to A.a"),
        ("grants p to A.a", "grants p to A"),
        ("grants p to A.a", "grants p to"),
        ("basis \"根拠\"", "basis"),
        ("grants p to A.a", "grants p to A.a; issued_by"),
        ("grants p to A.a", "grants p to A.a; not_by []"),
        ("grants p to A.a", "grants p to A.a; not_by p"),
        ("grants p to A.a", "grants p to A.a; not_by [q"),
        ("authority G {", "authority {"),
        (
            "authority G { basis \"根拠\"; grants p to A.a }",
            "authority G {}",
        ),
        (
            "authority G { basis \"根拠\"; grants p to A.a }",
            "authority G",
        ),
    ] {
        failure(&base.replace(from, to), "E-SYN-0");
    }
    for declaration in [
        "permit [] to A.a",
        "permit p A.a",
        "permit p to A",
        "permit p to",
        "permit to A.a",
    ] {
        failure(
            &base.replace("process P", &format!("{declaration}; process P")),
            "E-SYN-0",
        );
    }
    for clause in [
        "separation A.a.finish B.b.finish",
        "separation A.a.finish,",
        "separation , B.b.finish",
        "permitted",
        "authority_basis A.a",
        "permitted A.a permitted B.b",
    ] {
        failure(&model("end done", clause), "E-SYN-0");
    }
    for clause in [
        "permitted A.a.start",
        "authority_basis A.a.commit G",
        "separation case.received, A.a.finish",
        "separation end.done, A.a.finish",
        "separation A.a.unknown, B.b.finish",
        "contest A.a.start",
        "excludes A.a.start",
        "waive A.a.start",
    ] {
        failure(&model("end done", clause), "U-SYN-0");
    }
    for (from, to) in [
        ("executors [p]", "executors { p }"),
        ("people [p,q]", "people [p,q]; executors [p]"),
        // ADR 0027 accepts scope/expiry here. Keep unsupported coverage on
        // genuinely out-of-scope syntax, not yesterday's accepted duration.
        ("grants p to A.a", "grants p to A.a; scope end.done == true"),
        ("grants p to A.a", "grants p to A.a; expires 1.5min"),
        ("grants p to A.a", "grants p to A.a; revoke p"),
        ("executors [p]", "executors [p]; permit p to A.a"),
        ("executors [p]", "executors [p]; people [p]"),
        (
            "executors [p]",
            "executors [p]; authority H { basis \"x\"; grants p to A.a }",
        ),
        ("process P", "standing p on A.a; process P"),
    ] {
        failure(&base.replace(from, to), "U-SYN-0");
    }
    // ADR0044 accepts rule-change grants. Preserve the exact old specimen,
    // now a lower name error for undeclared R, rather than deleting coverage.
    let rule_change = base.replace("grants p to A.a", "grants p to change R");
    assert_ne!(base, rule_change);
    failure(&rule_change, "E-NAME-0");
    // ADR 0035 で移した形: finish の代入の源の executor.id は受理する構文になった
    // (ADR 0035 の「入れる」の executor.id の行と U10)。この入力は input_by を宣言せず
    // writes にも書かないので、ADR 0012 の「finish に書く変数が writes に無ければ
    // E-NAME-0」で invalid になる。受理の正例は fixtures/aset-executor(finish)と
    // fixtures/aset-person-start(start)、条件で読む形の U-SYN-0 は直下の if の入力が保つ。
    failure(
        &base.replace("commits;", "commits; finish { input_by = executor.id };"),
        "E-NAME-0",
    );
    failure(
        &model("if executor.id == p { end done } else { end other }", ""),
        "U-SYN-0",
    );
    // ADR 0018 limited migration: a well-formed duty is no longer unsupported.
    // Keep this exact former input as a positive, not a deleted assertion.
    let duty = serde_json::to_value(report::check(
        &base.replace("process P", "responsible p for A.a; process P"),
    ))
    .unwrap();
    assert_eq!(duty["verdict"], "satisfied");
    assert_eq!(duty["responsibilities"][0]["person"], "p");
    assert_eq!(duty["responsibilities"][0]["target"]["value"], "A.a");
}

#[test]
fn e_fixture_anonymous_execution_skeleton_control_not_executor_acceptance() {
    // Remove only the new layer and check the authored flow/intents/arithmetic
    // with existing Phase 0 semantics. In particular F4broken becomes e5, NOT
    // its named-person oracle e8. A green control is not executor verification.
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut count = 0;
    for entry in fs::read_dir(root).unwrap() {
        let dir = entry.unwrap().path();
        let name = dir.file_name().unwrap().to_str().unwrap();
        if !dir.is_dir() || !name.starts_with("exec-") {
            continue;
        }
        let original = fs::read_to_string(dir.join("model.relca")).unwrap();
        let prefix = original
            .split_once("  contract Controls {")
            .map_or(original.as_str(), |(prefix, _)| prefix);
        let had_contract = prefix.len() != original.len();
        let mut source = prefix
            .lines()
            .filter(|line| {
                !["people ", "permit ", "authority "]
                    .iter()
                    .any(|word| line.trim_start().starts_with(word))
            })
            .collect::<Vec<_>>()
            .join("\n");
        if had_contract {
            source.push('}');
        }
        for pool in ["[a1,c1]", "[p,q]"] {
            source = source.replace(&format!("executors {pool}"), "capacity 2");
        }
        for pool in ["[p]", "[q]", "[c1]", "[a1]"] {
            source = source.replace(&format!("executors {pool}"), "capacity 1");
        }
        source = source.replace("capacity 1; capacity 1;", "capacity 1;");
        let selections: &[Option<&str>] = if name.starts_with("exec-vacuous") {
            &[Some("AsIs"), Some("ToBeA")]
        } else {
            &[None]
        };
        for &selection in selections {
            let value = serde_json::to_value(match selection {
                None => report::check(&source),
                Some(s) => report::check_configuration(&source, s),
            })
            .unwrap();
            assert_eq!(value["verdict"], "satisfied", "{name}: {source}\n{value}");
            let (elapsed, work, result) =
                if name.starts_with("exec-separation") || name.starts_with("exec-anonymous") {
                    (2, 2, None)
                } else if name.starts_with("exec-permitted") || name.starts_with("exec-authority") {
                    (1, 1, None)
                } else if name.starts_with("exec-shared-person") {
                    (5, 8, None)
                } else if name.starts_with("exec-vacuous") {
                    if selection == Some("AsIs") {
                        (3, 3, None)
                    } else {
                        (2, 2, None)
                    }
                } else if name == "exec-cancel" {
                    (3, 4, Some(1))
                } else {
                    (6, 7, Some(1))
                };
            metrics(&value, elapsed, elapsed, work, result);
            count += 1;
        }
    }
    assert_eq!(
        count, 16,
        "14 directories / 16 anonymous skeletons, including the statically rejected named fixture"
    );
}

#[test]
fn e_fixture_pairs_have_exactly_the_declared_single_source_edit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    for (base, fault, from, to) in [
        (
            "exec-separation",
            "shared-approver",
            "executors [a1]",
            "executors [a1,c1]",
        ),
        (
            "exec-permitted",
            "narrow-permit",
            "  permit q to Runner.run\n",
            "",
        ),
        (
            "exec-authority",
            "missing-grant",
            " grants q to Runner.run;",
            "",
        ),
        (
            "exec-shared-person",
            "reused-person",
            "actor B { executors [q]",
            "actor B { executors [p]",
        ),
        (
            "exec-vacuous",
            "shared-approver",
            "executors [a1]",
            "executors [a1,c1]",
        ),
        ("exec-anonymous", "no-executor", " executors [p];", ""),
        (
            "exec-cancel",
            "ignore-cancel",
            "cancel cooperative ack_within 1min",
            "cancel forbidden_after_start",
        ),
    ] {
        let source = fs::read_to_string(root.join(base).join("model.relca")).unwrap();
        assert_eq!(source.matches(from).count(), 1, "one fault site for {base}");
        let broken = fs::read_to_string(
            root.join(format!("{base}.broken-{fault}"))
                .join("model.relca"),
        )
        .unwrap();
        assert_eq!(
            source.replace(from, to),
            broken,
            "single semantic fault {base}"
        );
    }
}

fn expected_assumptions(
    actors: &[(&str, Option<u64>, bool, bool)],
    inferred_capacities: &[(&str, u64)],
) -> Value {
    let mut result = Vec::new();
    for &(actor, anonymous, called, permission) in actors {
        result.push(json!({"field":format!("{actor}.boundary"),"value":actor,"affects":"Actor boundary is represented by its name only"}));
        if let Some(n) = anonymous {
            result.push(json!({"field":format!("{actor}.executors"),"value":format!("anonymous×{n}"),"affects":"executor identity, separation of duties and permission checks are unsupported"}));
        }
        result.push(json!({"field":format!("{actor}.control"),"value":if called {"policy"} else {"environment"},"affects":"classification of policy and environment choices"}));
        if permission {
            result.push(json!({"field":format!("{actor}.permission"),"value":"default(executors)","affects":"Actor executors may execute their own Capabilities; authority is not inferred"}));
        }
        if let Some((_, n)) = inferred_capacities.iter().find(|(name, _)| *name == actor) {
            result.push(json!({"field":format!("{actor}.capacity"),"value":format!("count(executors)={n}"),"affects":"Actor capacity is the number of declared executors"}));
        }
    }
    result.push(json!({"field":"schedule","value":"work-conserving","affects":"enabled tasks start without intentional idle; competing starts remain nondeterministic"}));
    json!(result)
}

#[test]
fn e_assumptions_exact_defaults_explicit_union_and_zero_capability() {
    let base = model("x = A.a(); end done", "permitted A.a")
        .replace(
            "actor B {",
            "actor NamedEmpty { executors [q] }; actor AnonymousEmpty {}; actor B {",
        )
        .replace("actor B { executors [q]", "actor B { capacity 1")
        .replace(
            "capability a()",
            "capability other() -> T { duration fixed 1min; result nondet { ok } }; capability a()",
        );
    for (permit, default) in [
        ("", true),
        ("permit p to A.a;", true),
        ("permit p to A.a; permit q to A.a;", true),
        ("permit p to A.a; permit q to A.other;", false),
    ] {
        let value = check(&base.replace("process P", &format!("{permit} process P")));
        assert_eq!(
            value["assumptions"],
            expected_assumptions(
                &[
                    ("A", None, true, default),
                    ("NamedEmpty", None, false, false),
                    ("AnonymousEmpty", None, false, true),
                    ("B", Some(1), false, true)
                ],
                &[("A", 1), ("NamedEmpty", 1)],
            ),
            "{value}"
        );
    }
    for source in [
        include_str!("../../../fixtures/exec-permitted/model.relca"),
        include_str!("../../../fixtures/exec-permitted.broken-narrow-permit/model.relca"),
        AUTHORITY,
        MISSING_GRANT,
    ] {
        assert_eq!(
            check(source)["assumptions"],
            expected_assumptions(&[("Runner", None, true, false)], &[("Runner", 2)])
        );
    }
}

#[test]
fn e_separation_allpairs_path_local_and_bundle() {
    // Four assignments (p,p,p),(p,q,p),(q,p,p),(q,q,p); first three
    // violate, last satisfies; sites 0..1/1..2/2..3, e3/w3.
    let source = model(
        "x = A.a(); z = A.a(); y = B.b(); end done",
        "separation A.a.finish, B.b.finish",
    )
    .replace("executors [p]", "executors [p,q]")
    .replace("executors [q]", "executors [p]");
    for source in [
        source.clone(),
        source.replace(
            "separation A.a.finish, B.b.finish",
            "separation B.b.finish, A.a.finish",
        ),
    ] {
        let value = check(&source);
        clauses(&value, &["violated"]);
        metrics(&value, 3, 3, 3, None);
    }
    // Gate's two outcomes choose exactly one side, never a pair on one path.
    let source=model("g = Gate.go(); if g == ok { x = A.a() } else { y = B.b() }; end done","separation A.a.finish, B.b.finish")
        .replace("actor A", "actor Gate { capacity 1; capability go() -> T { duration fixed 1min; result nondet { ok,bad } } }; actor A")
        .replace("executors [q]","executors [p]");
    let value = check(&source);
    clauses(&value, &["vacuous"]);
    metrics(&value, 2, 2, 2, None);
    for (clause, verdict) in [
        ("separation A.a.finish, A.a.commit", "violated"),
        ("separation A.a.finish, A.a.finish(ok)", "violated"),
        ("separation A.a.finish(ok), A.a.finish(bad)", "vacuous"),
    ] {
        let value = check(&model("x = A.a(); end done", clause));
        clauses(&value, &[verdict]);
        metrics(&value, 1, 1, 1, None);
    }
}

fn metrics(value: &Value, min: u64, max: u64, work: u64, result_at: Option<u64>) {
    assert_eq!(
        value["terminals"],
        json!([{"end":"done","elapsed":{"min":min,"max":max},"work":{"min":work,"max":work},"result_at":result_at.map(|r|json!({"min":r,"max":r}))}]),
        "{value}"
    );
    assert_eq!(value["bounds"]["truncated"], false);
    assert_eq!(value["properties"][0]["verdict"], "satisfied");
}

#[test]
fn e_allassignments_time_and_anonymous_named_independence() {
    let base =
        include_str!("../../../fixtures/exec-shared-person.broken-reused-person/model.relca");
    for people in ["[p,q]", "[q,p]"] {
        let source = base.replace(
            "actor A { executors [p]",
            &format!("actor A {{ executors {people}"),
        );
        // A=p first -> e8; A=q, B=p in either start order -> e5. Work8.
        let value = check(&source);
        metrics(&value, 5, 8, 8, None);
        assert_eq!(value["verdict"], "satisfied");
    }
    let value = check(&base.replace("actor A { executors [p]", "actor A { capacity 1"));
    metrics(&value, 5, 5, 8, None);
    assert_eq!(value["verdict"], "satisfied");
}

#[test]
fn e_named_due_ack_and_permitted_start_cancel() {
    let tied=CANCEL.replace("fixed 5min","fixed 2min").replace("contract Controls {","contract Controls { response Fast.win.finish -> one_of(Slow.work.finish) by 1min after Fast.win.finish otherwise unresolved;");
    let value = check(&tied);
    clauses(&value, &["violated", "satisfied", "violated", "violated"]);
    metrics(&value, 3, 3, 4, Some(1));
    let value = check(&CANCEL.replace("process Flow", "permit q to Slow.work; process Flow"));
    clauses(&value, &["violated", "vacuous", "vacuous"]);
    metrics(&value, 3, 3, 4, Some(1));
    assert_eq!(value["verdict"], "violated");
}

#[test]
fn e_authority_no_commit_other_authority_and_other_capability() {
    for (source, expected) in [(AUTHORITY, "satisfied"), (MISSING_GRANT, "violated")] {
        clauses(&check(&source.replace("commits;", "")), &[expected]);
    }
    let other = MISSING_GRANT.replace(
        "process Flow",
        "authority H { basis \"other\"; grants q to Runner.run }; process Flow",
    );
    clauses(&check(&other), &["violated"]);
    let other=MISSING_GRANT.replace("capability run()", "capability other() -> T { duration fixed 1min; result nondet { ok } }; capability run()")
        .replace("grants p to Runner.run;", "grants p to Runner.run; grants q to Runner.other;");
    clauses(&check(&other), &["violated"]);
    for source in [
        include_str!("../../../fixtures/exec-permitted.broken-narrow-permit/model.relca"),
        MISSING_GRANT,
    ] {
        clauses(&check(&source.replace("[p,q]", "[q,p]")), &["violated"]);
    }
}

#[test]
fn e_cut_retains_start_violation_and_never_promotes_inactive_or_fulfilled() {
    let source = model(
        "x = A.a(); y = B.b(); end done",
        "permitted A.a; authority_basis A.a G; separation A.a.start, B.b.finish",
    )
    .replace(
        "capability a() -> T { commits; duration fixed 1min",
        "capability a() -> T { commits; duration fixed 2min",
    )
    .replace("clock 20min", "clock 1min");
    for (source, expected) in [
        (source.clone(), ["inconclusive"; 3]),
        (
            source.replace("process P", "permit q to A.a; process P"),
            ["violated", "inconclusive", "inconclusive"],
        ),
    ] {
        let value = check(&source);
        clauses(&value, &expected);
        assert_eq!(value["bounds"]["truncated"], true);
        assert_eq!(value["terminals"], json!([]));
    }
    let fulfilled = model("x = A.a(); y = B.b(); end done", "authority_basis A.a G")
        .replace(
            "capability b() -> T { duration fixed 1min",
            "capability b() -> T { duration fixed 5min",
        )
        .replace("clock 20min", "clock 3min");
    clauses(&check(&fulfilled), &["inconclusive"]);
}

#[test]
fn e_assignment_budget_public_zero_and_violation_retention() {
    let source = model(
        "x = A.a(); y = B.b(); end done",
        "permitted A.a; authority_basis A.a G; separation A.a.finish, B.b.finish",
    );
    let ir = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    let zero = engine::explore_with_budget(&ir, 0);
    assert!(zero.truncated);
    assert!(
        zero.properties
            .iter()
            .all(|p| p.verdict == Verdict::Inconclusive)
    );
    assert!(zero.diagnostics.iter().any(|d| d.code == "W-BUDGET-0"));
    let violating = source.replace("process P", "permit q to A.a; process P");
    let ir = lower::lower(&syntax::parse(&violating).unwrap()).unwrap();
    let retained = (1..128)
        .map(|n| engine::explore_with_budget(&ir, n))
        .find(|out| out.truncated && out.properties[1].verdict == Verdict::Violated)
        .expect("start violation before a later budget cut");
    assert_eq!(retained.properties[2].verdict, Verdict::Inconclusive);
    let end = model("end done", "");
    let ir = lower::lower(&syntax::parse(&end).unwrap()).unwrap();
    let one = engine::explore_with_budget(&ir, 1);
    assert!(!one.truncated);
    assert_eq!(one.terminals.len(), 1);
}

#[test]
fn e_abort_same_bundle_preserves_two_violations_in_every_clause_order() {
    let response = "response A.a.finish -> one_of(end.done) by 18446744073709551615min after A.a.finish otherwise unresolved";
    let separation = "separation A.a.finish, A.a.commit";
    let authority = "authority_basis A.a G";
    for (clauses_text, expected) in [
        (
            format!("{response}; {separation}; {authority}"),
            ["unsupported", "violated", "violated"],
        ),
        (
            format!("{authority}; {separation}; {response}"),
            ["violated", "violated", "unsupported"],
        ),
    ] {
        let value = check(
            &model("x = A.a(); end done", &clauses_text)
                .replace("grants p to A.a", "grants q to A.a"),
        );
        clauses(&value, &expected);
        assert_eq!(value["verdict"], "violated");
        assert!(has_code(&value, "U-SEM-0"));
        assert_eq!(value["bounds"]["truncated"], Value::Null);
        assert_eq!(value["terminals"], json!([]));
    }
}

#[test]
fn e_engine_zero_public_finish_and_release() {
    // Not in reference population: reference intentionally excludes duration0.
    let source = model(
        "x = A.a(); y = B.b(); end done",
        "separation A.a.start, A.a.finish; permitted A.a; authority_basis A.a G",
    )
    .replace("fixed 1min", "fixed 0min")
    .replace("executors [q]", "executors [p]");
    let value = check(&source);
    clauses(&value, &["violated", "satisfied", "satisfied"]);
    metrics(&value, 0, 0, 0, None);
}

#[test]
fn e_ids_wire_mixed_contract_inventory_and_inactive_diff() {
    let source=model("x = A.a(); y = B.b(); end done","permitted A.a; authority_basis A.a G; invariant A.a.finish -> x == ok; at_most_once A.a.commit per case.id; response case.received -> one_of(end.done) by 5min after case.received otherwise unresolved; requires_event B.b.start after A.a.finish")
        .replace("contract Controls {", "contract Empty {}\ncontract { separation A.a.finish, B.b.finish }\nverify reachable end.done\ncontract Controls {");
    let value = check(&source);
    let ids: Vec<_> = value["properties"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "safety no_resource_overcommit",
            "contract #1 separation",
            "reachable end.done",
            "contract Controls #2 permitted",
            "contract Controls #3 authority_basis",
            "contract Controls #4 invariant",
            "contract Controls #5 at_most_once",
            "contract Controls #6 response",
            "contract Controls #7 requires_event"
        ]
    );
    assert_eq!(
        value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "actors",
            "agency",
            "assumptions",
            "bounds",
            "cut",
            "cut_context",
            "diagnostics",
            "ledger",
            "mode",
            "model",
            "properties",
            "terminals",
            "uncertainty",
            "unit",
            "verdict"
        ]
    );
    assert!(value["model"]["sha256"].is_string());
    let configured=source.replace("bounds {","process EmptyFlow { end done }\ncut C { include [A,B] }\nconfiguration From { cut C; process P }\nconfiguration To { cut C; process EmptyFlow }\nbounds {");
    let value = serde_json::to_value(diff::diff(&configured, "From", "To")).unwrap();
    let rows = value["properties"].as_array().unwrap();
    assert_eq!(rows.len(), ids.len());
    for (i, (row, id)) in rows.iter().zip(ids).enumerate() {
        assert_eq!(row["id"], id);
        assert_eq!(row["from"], "satisfied");
        assert_eq!(
            row["to"],
            if [0, 2, 7].contains(&i) {
                "satisfied"
            } else {
                "vacuous"
            }
        );
    }
    assert_eq!(value["verdict"], "vacuous");
}

#[test]
fn e_agency_remains_actor_level_and_diff_does_not_claim_authority_or_resource() {
    let before = serde_json::to_value(diff::diff(CONFIGURED, "AsIs", "ToBeA")).unwrap();
    let after = serde_json::to_value(diff::diff(
        &CONFIGURED.replace("executors [a1]", "executors [a1,c1]"),
        "AsIs",
        "ToBeA",
    ))
    .unwrap();
    for side in ["from", "to"] {
        let agency = &before[side]["report"]["agency"];
        assert!(agency.is_object());
        assert_eq!(agency, &after[side]["report"]["agency"]);
        // ADR 0043 (plan C03): executors leaves unchecked only when the
        // effects viewpoint is derived.
        assert_eq!(
            agency["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("executors")),
            agency["viewpoints"]["effects"]["status"] != "derived"
        );
        let names: Vec<_> = agency["origin"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["actor"].as_str().unwrap())
            .collect();
        assert!(
            names
                .iter()
                .all(|n| ["Clerk", "Approver", "Extractor"].contains(n))
        );
    }
    // ADR 0031: diff now computes resource and authority (and the other
    // REMOVE_UNCHECKED words). The pre-ADR lists at 85c5485 are projected, not
    // dropped: the old unchecked minus those words, the old checked plus them.
    const OLD_UNCHECKED: [&str; 9] = [
        "time_intent",
        "resource",
        "meaning",
        "authority",
        "visibility",
        "contest",
        "preserve",
        "override",
        "maintain",
    ];
    const OLD_CHECKED: [&str; 5] = [
        "comparison_mismatch",
        "actor",
        "relation",
        "intent_claims",
        "retire_loses",
    ];
    for word in ["resource", "authority"] {
        assert!(OLD_UNCHECKED.contains(&word));
    }
    assert_eq!(
        before["unchecked"],
        diff_increment::migrated_unchecked(&OLD_UNCHECKED)
    );
    assert_eq!(
        before["checked"],
        diff_increment::migrated_checked(&OLD_CHECKED)
    );
}

#[test]
fn e_receipt_and_terminal_keep_adr0010_classification_outside_separation() {
    // Review minor-2: the executor-less rejection belongs to separation only.
    // ADR 0037 owns invariant receipt antecedents; the remaining ADR 0010
    // clauses keep E-SYN-0 for a known word in the wrong position.
    for clause in [
        "response case.received -> one_of(end.done) by after case.received otherwise unresolved",
        "requires_event A.a.start after case.received",
        "at_most_once end.done per case.id",
        "requires_event A.a.start after end.done",
    ] {
        failure(&model("x = A.a(); end done", clause), "E-SYN-0");
    }
    for clause in [
        "separation case.received, A.a.finish",
        "separation A.a.finish, case.received",
        "separation end.done, A.a.finish",
        "separation A.a.finish, end.done",
    ] {
        failure(&model("x = A.a(); end done", clause), "U-SYN-0");
    }
}
