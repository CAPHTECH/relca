//! ADR 0018: independent hand expectations through public report entry points.
use relca_core::{
    compare::{self, Analysis},
    diff, report,
};
use serde_json::{Value, json};
#[path = "support/responsibility_defaults_oracle.rs"]
mod responsibility_oracle;
use std::{fs, path::PathBuf};
#[path = "support/diff_increment.rs"]
mod diff_increment;

pub const FIXTURES: [&str; 19] = [
    "resp-contest",
    "resp-contest.broken-no-authority",
    "resp-contest.broken-no-permission",
    "resp-contest.broken-route-excluded",
    "resp-contest.broken-zero-deadline",
    "resp-cut",
    "resp-cut.broken-no-explicit-cut",
    "resp-diff",
    "resp-diff.broken-no-authorized-decision",
    "resp-missing-duty",
    "resp-missing-duty.broken-no-duty",
    "resp-origin",
    "resp-origin.broken-no-direct-read",
    "resp-refusal",
    "resp-refusal.broken-cut-guard",
    "resp-standing",
    "resp-standing.broken-not-party",
    "resp-state",
    "resp-state.broken-no-direct-read",
];
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn check(s: &str) -> Value {
    serde_json::to_value(report::check(s)).unwrap()
}
pub fn partial(expected: &Value, actual: &Value) {
    match expected {
        Value::Object(fields) => {
            for (k, v) in fields {
                if k == "absent_diagnostics" {
                    for c in v.as_array().unwrap() {
                        assert_eq!(code_count(actual, c.as_str().unwrap()), 0, "{actual}");
                    }
                } else if k == "alternatives" {
                    let actual = actual[k].as_array().unwrap();
                    let hand = v.as_array().unwrap();
                    assert_eq!(hand.len(), actual.len());
                    for alternative in hand {
                        assert!(
                            actual.iter().any(|a| alternative
                                .as_object()
                                .unwrap()
                                .iter()
                                .all(|(k, v)| a.get(k) == Some(v))),
                            "missing {alternative}: {actual:?}"
                        );
                    }
                } else {
                    partial(
                        v,
                        actual
                            .get(k)
                            .unwrap_or_else(|| panic!("missing {k}: {actual}")),
                    );
                }
            }
        }
        Value::Array(a) => {
            let b = actual
                .as_array()
                .unwrap_or_else(|| panic!("not array: {actual}"));
            if a.first().is_some_and(|v| v.get("code").is_some()) {
                for v in a {
                    assert_eq!(
                        b.iter().filter(|x| x["code"] == v["code"]).count(),
                        1,
                        "diagnostic multiplicity: {actual}"
                    );
                    partial(v, b.iter().find(|x| x["code"] == v["code"]).unwrap());
                }
            } else {
                assert_eq!(a.len(), b.len(), "inventory: {expected} versus {actual}");
                for (i, v) in a.iter().enumerate() {
                    let x = if v.get("end").is_some() && v.get("id").is_none() {
                        b.iter().find(|x| x["end"] == v["end"]).unwrap()
                    } else if v.get("id").is_some() {
                        b.iter().find(|x| x["id"] == v["id"]).unwrap()
                    } else {
                        &b[i]
                    };
                    partial(v, x);
                }
            }
        }
        _ => assert_eq!(expected, actual),
    }
}
pub fn code_count(v: &Value, code: &str) -> usize {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .count()
}
fn unchecked(v: &Value, word: &str) -> bool {
    v["agency"]["unchecked"]
        .as_array()
        .unwrap()
        .contains(&json!(word))
}
fn contest(v: &Value) -> &Value {
    &v["contest_paths"][0]
}
fn duty(v: &Value) -> &Value {
    &v["responsibilities"][0]
}
fn failure(v: &Value, code: &str) {
    assert_eq!(
        v["verdict"],
        if code.starts_with('U') {
            "unsupported"
        } else {
            "invalid"
        },
        "{v}"
    );
    assert_eq!(code_count(v, code), 1, "{v}");
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert_eq!(v["agency"], Value::Null);
    assert_eq!(
        v["bounds"],
        json!({"cases":null,"clock":null,"live_tasks":null,"truncated":null})
    );
    assert!(v.get("responsibilities").is_none());
    assert!(v.get("contest_paths").is_none());
    assert!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .all(|d| d["code"] == code),
        "unintended earlier error: {v}"
    );
}

#[test]
fn r_all_19_hand_oracles_and_raw_diff_transport() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir() && e.file_name().to_string_lossy().starts_with("resp-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    for name in names {
        let s = source(&name);
        let expected: Value = serde_json::from_str(
            &fs::read_to_string(root().join(&name).join("expected.json")).unwrap(),
        )
        .unwrap();
        let actual = if name.starts_with("resp-diff") {
            let v = serde_json::to_value(diff::diff(&s, "A", "B")).unwrap();
            for (side, selection) in [("from", "A"), ("to", "B")] {
                let single =
                    serde_json::to_value(report::check_configuration(&s, selection)).unwrap();
                assert_eq!(
                    v[side]["report"], single,
                    "raw report transport {name}/{selection}"
                );
            }
            // ADR 0031: diff computes responsibility and contest. The pre-ADR
            // list at 85c5485 is projected: old minus REMOVE_UNCHECKED.
            const OLD_UNCHECKED: [&str; 12] = [
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
                "obligation_tables",
                "contest_runtime_tables",
            ];
            assert_eq!(
                v["unchecked"],
                diff_increment::migrated_unchecked(&OLD_UNCHECKED)
            );
            assert!(
                v["differences"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|d| d["kind"] != "responsibility" && d["kind"] != "contest")
            );
            v
        } else {
            check(&s)
        };
        partial(&expected, &actual);
        if name == "resp-standing.broken-not-party" {
            failure(&actual, "E-CON-0");
        }
    }
}

/// Every directly authored rejection is enumerated here, also consumed by CLI
/// and the old-binary preflight. No hidden source literals in individual tests.
pub fn rejection_inputs() -> Vec<(String, String, &'static str)> {
    let f = source("resp-contest");
    let o = source("resp-origin");
    let st = source("resp-state");
    let mut rows = Vec::new();
    let mut add = |label: &str, base: &str, from: &str, to: &str, code| {
        assert!(base.contains(from), "bad mutation {label}");
        rows.push((label.to_owned(), base.replacen(from, to, 1), code));
    };
    for (label, from, to, code) in [
        (
            "unknown-reversible",
            "reversible_by [a1]",
            "reversible_by [nobody]",
            "E-NAME-0",
        ),
        (
            "unknown-standing-end",
            "standing over [end.returned]",
            "standing over [end.unknown]",
            "E-NAME-0",
        ),
        (
            "unknown-contest-end",
            "on end.returned",
            "on end.unknown",
            "E-NAME-0",
        ),
        (
            "unknown-party",
            "contest Vendor on",
            "contest Unknown on",
            "E-NAME-0",
        ),
        (
            "unknown-route-actor",
            "route Clerk.recheck",
            "route Unknown.recheck",
            "E-NAME-0",
        ),
        (
            "unknown-route-cap",
            "route Clerk.recheck",
            "route Clerk.unknown",
            "E-NAME-0",
        ),
        ("standing-nonparty", "    control party\n", "", "E-CON-0"),
        (
            "standing-mismatch",
            "standing over [end.returned]",
            "standing over [end.posted]",
            "E-CON-0",
        ),
        ("missing-deadline", " respond_by 5min", "", "E-SYN-0"),
        ("missing-route", " route Clerk.recheck", "", "E-SYN-0"),
        ("hours", "respond_by 5min", "respond_by 5hour", "U-SYN-0"),
        ("days", "respond_by 5min", "respond_by 5day", "U-SYN-0"),
        (
            "policy-control",
            "control party",
            "control policies",
            "U-SYN-0",
        ),
        (
            "environment-control",
            "control party",
            "control environments",
            "U-SYN-0",
        ),
        (
            "start-standing",
            "standing over [end.returned]",
            "standing over [Approver.approve.start]",
            "U-SYN-0",
        ),
        (
            "empty-standing",
            "standing over [end.returned]",
            "standing over []",
            "E-SYN-0",
        ),
        (
            "empty-reversible",
            "reversible_by [a1]",
            "reversible_by []",
            "E-SYN-0",
        ),
        (
            "missing-basis",
            "basis \"invoice claim\"",
            "basis",
            "E-SYN-0",
        ),
        (
            "duplicate-control",
            "control party",
            "control party\n    control party",
            "U-SEM-0",
        ),
        (
            "duplicate-standing",
            "standing over [end.returned]",
            "standing over [end.returned,end.returned]",
            "U-SEM-0",
        ),
        (
            "duplicate-reversible",
            "reversible_by [a1]",
            "reversible_by [a1,a1]",
            "U-SEM-0",
        ),
        (
            "excludes",
            "    contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min",
            "    excludes Approver.approve.start",
            "U-SYN-0",
        ),
        (
            "waive",
            "    contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min",
            "    waive Approver.approve.start",
            "U-SYN-0",
        ),
        (
            "runtime-contest",
            "    contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min",
            "    contest Approver.approve.start",
            "U-SYN-0",
        ),
        (
            "horizon",
            "cut Scope {",
            "cut Scope { horizon receipt..resolution within 20hour;",
            "U-SYN-0",
        ),
        (
            "misplaced-standing",
            "  process Flow {",
            "  standing p on Approver.approve\n  process Flow {",
            "U-SYN-0",
        ),
        (
            "actor-responsible",
            "    control party",
            "    responsible a1 for Approver.approve",
            "U-SYN-0",
        ),
    ] {
        add(label, &f, from, to, code);
    }
    for (label, from, to, code) in [
        (
            "unknown-responsible",
            "responsible a1 for",
            "responsible nobody for",
            "E-NAME-0",
        ),
        (
            "unknown-target-actor",
            "responsible a1 for Approver.approve",
            "responsible a1 for Unknown.approve",
            "E-NAME-0",
        ),
        (
            "unknown-target-cap",
            "responsible a1 for Approver.approve",
            "responsible a1 for Approver.unknown",
            "E-NAME-0",
        ),
        (
            "duplicate-duty",
            "responsible a1 for Approver.approve",
            "responsible a1 for Approver.approve\n  responsible a1 for Approver.approve",
            "U-SEM-0",
        ),
        (
            "binding-only",
            "responsible a1 for Approver.approve",
            "responsible a1 for (approval == pass)",
            "U-SEM-0",
        ),
        (
            "duty-event-target",
            "responsible a1 for Approver.approve",
            "responsible a1 for Approver.approve.commit",
            "U-SYN-0",
        ),
    ] {
        add(label, &o, from, to, code);
    }
    add(
        "unknown-state",
        &st,
        "registered == false || approval == pass",
        "unknown == false",
        "E-NAME-0",
    );
    add(
        "wrong-state-value",
        &st,
        "registered == false || approval == pass",
        "registered == unknown",
        "E-TYPE-0",
    );
    let no_people = source("resp-cut");
    add(
        "duty-without-people",
        &no_people,
        "  process Flow {",
        "  responsible nobody for Approver.approve\n  process Flow {",
        "E-NAME-0",
    );
    let excluded=o.replace("actor Invoice { source document }","actor Invoice { source document }\n  actor Hidden { source record; latent truth: Verdict = pass }")
        .replace("cut Scope { include [Invoice,Reader,Approver,Ledger] }","cut Scope { include [Invoice,Reader,Approver,Ledger]; exclude Hidden reason \"outside\" loses \"truth\" }");
    add(
        "excluded-local-latent",
        &excluded,
        "responsible a1 for Approver.approve",
        "responsible a1 for (Hidden.truth == pass)",
        "E-CUT-1",
    );
    for word in ["control", "responsible", "standing"] {
        add(&format!("reserved-{word}"), &o, "Approver", word, "U-SYN-0");
    }
    rows
}

#[test]
fn r_static_rejections_matrix() {
    for (label, s, code) in rejection_inputs() {
        let v = check(&s);
        eprintln!("static row {label}");
        failure(&v, code);
    }
}

/// Additional positive/control variants. Preflight enumerates exactly these
/// same sources, rather than a separately reconstructed subset.
pub fn extra_inputs() -> Vec<(String, String)> {
    let mut out = Vec::new();
    out.push(("migrated-old-responsible".into(),r#"model M version "0.2" {
      type T = enum { ok, bad }
      people [p,q]
      actor A { executors [p]; capability a() -> T { commits; duration fixed 1min; result nondet { ok } } }
      actor B { executors [q]; capability b() -> T { duration fixed 1min; result nondet { ok } } }
      authority G { basis "根拠"; grants p to A.a }
      responsible p for A.a; process P { end done }
      bounds { cases 1; live_tasks 2; clock 20min }
      verify safety no_resource_overcommit
      contract Controls {  }
    }"#.into()));
    let f = source("resp-contest");
    let o = source("resp-origin");
    let st = source("resp-state");
    let cut = source("resp-cut.broken-no-explicit-cut");
    for (label, entry) in [
        (
            "cut-exclude-label",
            "exclude Vendor.contest.returned reason \"outside\" loses \"contest\"",
        ),
        (
            "cut-defer-all",
            "defer Vendor.contest reason \"later\" loses \"contest\"",
        ),
        (
            "cut-exclude-all",
            "exclude Vendor.contest reason \"outside\" loses \"contest\"",
        ),
        (
            "cut-rates",
            "defer Vendor.contest.rates reason \"later\" loses \"rates\"",
        ),
        ("cut-reentry", "reentry [contest Vendor -> Clerk.recheck]"),
        (
            "cut-reason-text",
            "defer Other reason \"Vendor.contest.returned\" loses \"contest\"",
        ),
        (
            "cut-missing-loses",
            "defer Vendor.contest.returned reason \"later\"",
        ),
    ] {
        out.push((
            label.into(),
            cut.replace(
                "include [Invoice,Reader,Clerk,Approver,Vendor]",
                &format!("include [Invoice,Reader,Clerk,Approver,Vendor]; {entry}"),
            ),
        ));
    }
    out.push(("cut-exclude-party".into(),cut.replace("include [Invoice,Reader,Clerk,Approver,Vendor]","include [Invoice,Reader,Clerk,Approver]; exclude Vendor reason \"outside\" loses \"contest\"")));
    out.push((
        "state-single-effect".into(),
        st.replace(
            "registered == false || approval == pass",
            "registered == false",
        ),
    ));
    out.push((
        "state-guard-unknown".into(),
        st.replace("if ap == pass", "if approval == pass"),
    ));
    out.push((
        "conditional-refusal".into(),
        o.replace(
            "    if approval == pass",
            "    other = Approver.approve(entry)\n    if approval == pass || other == pass",
        ),
    ));
    out.push(("unused-read".into(),source("resp-origin.broken-no-direct-read")
        .replace("  responsible a1 for","  actor Unused { executors [a1]; capability inspect() -> Verdict { reads [Invoice]; duration fixed 1min; result nondet { pass,fail } } }\n  responsible a1 for")
        .replace("include [Invoice,Reader,Approver,Ledger]","include [Invoice,Reader,Approver,Ledger,Unused]")));
    out.push((
        "all-people".into(),
        f.replace("people [c1,a1,cfo]", "people [c1,a1,a2,cfo]")
            .replace("executors [a1]", "executors [a1,a2]")
            .replace(
                "permit a1 to Approver.approve",
                "permit [a1,a2] to Approver.approve",
            )
            .replace("reversible_by [a1]", "reversible_by [a1,a2]"),
    ));
    out.push((
        "no-decision".into(),
        f.replace(
            "if approval == fail { end returned } else { end posted }",
            "end returned",
        ),
    ));
    let md = source("resp-missing-duty.broken-no-duty");
    out.push((
        "no-people-no-duty".into(),
        md.replace("  people [p]\n", "")
            .replace("executors [p]", "capacity 1"),
    ));
    out.push((
        "legacy-people-no-feature".into(),
        md.replace("    control party\n", ""),
    ));
    out.push((
        "legacy-no-feature".into(),
        f.lines()
            .filter(|l| !new_line(l))
            .collect::<Vec<_>>()
            .join("\n"),
    ));
    out.push((
        "contest-and-duty".into(),
        f.replace(
            "  process Flow {",
            "  responsible a1 for Approver.approve\n  process Flow {",
        ),
    ));
    out.push(("party-choice".into(),f.replace("  actor Vendor {","  actor Vendor {\n    capacity 1\n    capability choose(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass,fail } }").replace("approval = Approver.approve(entry)","approval = Vendor.choose(entry)")));
    out.push((
        "static-only".into(),
        f.replace("  verify safety no_resource_overcommit\n", ""),
    ));
    out.push((
        "mixed-ordinal".into(),
        f.replace(
            "    contest Vendor on",
            "    permitted Approver.approve\n    contest Vendor on",
        )
        .replace(
            "respond_by 5min\n",
            "respond_by 5min\n    authority_basis Approver.approve ApprovalGrant\n",
        ),
    ));
    out.push(("clock-cut".into(), f.replace("clock 20min", "clock 1min")));
    out.push(("live-cut".into(), f.replace("live_tasks 2", "live_tasks 0")));
    out.push((
        "violated-cut".into(),
        source("resp-contest.broken-no-authority").replace("clock 20min", "clock 1min"),
    ));
    out.push(("runtime-abort".into(),f.replace("    contest Vendor on","    response Approver.approve.finish -> one_of(end.returned) by 18446744073709551615min after Approver.approve.finish otherwise unresolved\n    contest Vendor on")));
    out.push((
        "multi-case-diff".into(),
        source("resp-diff").replace(
            "  bounds { cases 1;",
            "  arrivals fixed [0min,4min]\n  policy fifo tie_by case_id\n  bounds { cases 2;",
        ),
    ));
    out.push(("standing-foreign-process".into(),f.lines().filter(|l|!l.trim_start().starts_with("contest ")).collect::<Vec<_>>().join("\n")
        .replace("standing over [end.returned]","standing over [end.foreign]")
        .replace("  cut Scope {","  process Other { end foreign }\n  configuration A { cut Scope; process Flow }\n  configuration B { cut Scope; process Flow }\n  cut Scope {")));
    out.push((
        "cut-dead-end".into(),
        cut.replace("result nondet { pass,fail }", "result nondet { pass }"),
    ));
    out.push(("simulation-probability".into(),simulation_source(&f.replace("result nondet { pass,fail }","result nondet { fail }")
        .replace("  people [c1,a1,cfo]","  observed case { marked: bool = false }\n  people [c1,a1,cfo]")
        .replace("reads [Invoice]; duration","reads [Invoice]; writes marked; finish { marked = true }; duration")
        .replace("  contract Controls {","  verify probability reached marked == true within 20min >= 0.5\n  contract Controls {"))));
    for word in [
        "party",
        "over",
        "basis",
        "for",
        "contest",
        "on",
        "route",
        "reversible_by",
        "respond_by",
    ] {
        out.push((format!("contextual-{word}"), o.replace("Approver", word)));
    }
    out
}
pub fn new_line(line: &str) -> bool {
    let l = line.trim_start();
    ["control ", "standing ", "responsible ", "contest "]
        .iter()
        .any(|p| l.starts_with(p))
}

#[test]
fn r_cross_cut_origin_refusal_party_and_static_boundaries() {
    for (label, s) in extra_inputs() {
        if label == "standing-foreign-process" {
            let v = serde_json::to_value(report::check_configuration(&s, "A")).unwrap();
            assert_eq!(v["verdict"], "satisfied");
            assert_eq!(code_count(&v, "W-CONTEST-0"), 0);
            continue;
        }
        if label == "multi-case-diff" {
            let v = serde_json::to_value(compare::compare(&s, &["A", "B"], Analysis::Exhaustive))
                .unwrap();
            let rows: Vec<_> = v["tables"]["properties"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["id"] == "contract Controls #1 contest")
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["case_id"], Value::Null);
            for p in v["plans"].as_array().unwrap() {
                assert_eq!(p["report"]["contest_paths"].as_array().unwrap().len(), 1);
                assert_eq!(p["report"]["case_outcomes"].as_array().unwrap().len(), 2);
            }
            continue;
        }
        if label == "runtime-abort" {
            let v = check(&s);
            assert_eq!(v["verdict"], "unsupported");
            assert_eq!(v["bounds"]["truncated"], Value::Null);
            assert_eq!(v["contest_paths"][0]["verdict"], "satisfied");
            continue;
        }
        let v = check(&s);
        assert!(
            ![json!("invalid"), json!("unsupported")].contains(&v["verdict"]),
            "{label}: {v}"
        );
        // ADR 0054 "範囲" / "意味": exact selected-IR U/E candidates for
        // every accepted cross-cut input, including the people-free model.
        let ast = relca_core::syntax::parse(&s).unwrap();
        let model = relca_core::lower::lower(&ast).unwrap();
        responsibility_oracle::assert_report(&model, &v);
        match label.as_str() {
            "cut-rates" | "cut-reentry" | "cut-reason-text" | "cut-dead-end" => {
                assert_eq!(code_count(&v, "W-CONTEST-0"), 1)
            }
            l if l.starts_with("cut-") => {
                assert_eq!(code_count(&v, "W-CONTEST-0"), 0);
                assert_eq!(
                    code_count(&v, "W-CUT-1"),
                    usize::from(l == "cut-missing-loses")
                );
            }
            "state-single-effect" => {
                assert_eq!(duty(&v)["refusal"]["status"], "present");
                assert_eq!(v["properties"].as_array().unwrap().len(), 1);
                assert_eq!(v["verdict"], "satisfied");
            }
            "state-guard-unknown" => {
                assert_eq!(duty(&v)["refusal"]["status"], "not_derived");
                assert!(unchecked(&v, "state_guards"));
                assert!(unchecked(&v, "responsibility_refusal"));
                assert_eq!(code_count(&v, "W-RESP-2"), 0);
            }
            "conditional-refusal" => {
                assert_eq!(duty(&v)["refusal"]["status"], "conditional");
                assert_eq!(
                    duty(&v)["refusal"]["effects"][0]["judgements"][0]["bypasses"],
                    1
                );
                assert_eq!(code_count(&v, "W-RESP-2"), 1);
                assert_eq!(v["verdict"], "satisfied");
            }
            "unused-read" => {
                assert_eq!(duty(&v)["observation"]["status"], "missing");
                assert_eq!(code_count(&v, "W-RESP-1"), 1);
            }
            "all-people" => {
                assert_eq!(
                    contest(&v)["checks"]["reversal_authority"]["people"]
                        .as_array()
                        .unwrap()
                        .len(),
                    2
                );
                assert_eq!(contest(&v)["verdict"], "violated");
            }
            "no-decision" => {
                assert_eq!(contest(&v)["verdict"], "violated");
                assert_eq!(
                    contest(&v)["checks"]["reversal_authority"]["people"][0]["reason"],
                    "no_decision"
                );
            }
            // ADR 0054 removes the people gate. Poster is the E carrier in
            // both inputs; no person or duty is inferred from that effect.
            "no-people-no-duty" | "legacy-people-no-feature" => {
                assert_eq!(code_count(&v, "W-RESP-0"), 1);
                assert_eq!(
                    responsibility_oracle::expected(&model)[0].message,
                    "Poster has no declared responsibility"
                );
            }
            "party-choice" => {
                assert_eq!(v["terminals"].as_array().unwrap().len(), 2);
                assert!(
                    !v["assumptions"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|a| a["field"] == "Vendor.control")
                );
            }
            "mixed-ordinal" => {
                let ids: Vec<_> = v["properties"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|p| p["id"].as_str().unwrap())
                    .collect();
                assert_eq!(
                    ids,
                    vec![
                        "safety no_resource_overcommit",
                        "contract Controls #1 permitted",
                        "contract Controls #2 contest",
                        // ADR 0022: the runtime row shares the static ordinal.
                        "contract Controls #2 contest_response",
                        "contract Controls #3 authority_basis"
                    ]
                );
            }
            "static-only" => {
                assert_eq!(v["properties"].as_array().unwrap().len(), 2);
                assert_eq!(contest(&v)["verdict"], "satisfied");
                assert_eq!(v["terminals"].as_array().unwrap().len(), 2);
            }
            "clock-cut" | "live-cut" => {
                assert_eq!(v["verdict"], "inconclusive");
                assert_eq!(contest(&v)["verdict"], "satisfied");
            }
            "violated-cut" => {
                assert_eq!(v["verdict"], "violated");
                assert_eq!(contest(&v)["verdict"], "violated");
            }
            _ => {}
        }
    }
}

#[test]
fn r_unchecked_independent_and_assumptions() {
    let f = source("resp-contest");
    for (has_resp, has_contest, s) in [
        (
            false,
            false,
            extra_inputs()
                .into_iter()
                .find(|(l, _)| l == "legacy-no-feature")
                .unwrap()
                .1,
        ),
        (true, false, source("resp-origin")),
        (false, true, f.clone()),
        (
            true,
            true,
            f.replace(
                "  process Flow {",
                "  responsible a1 for Approver.approve\n  process Flow {",
            ),
        ),
    ] {
        let v = check(&s);
        assert_eq!(v["verdict"], "satisfied", "{v}");
        assert_eq!(unchecked(&v, "W-RESP-1"), !has_resp);
        assert_eq!(unchecked(&v, "contest_path"), !has_contest);
        assert!(!unchecked(&v, "W-LIN-2"), "W-LIN-2 is checked in Agency");
        assert!(
            !unchecked(&v, "requires"),
            "ADR 0026 checks Capability requires"
        );
        for old in ["fallback_path", "via_model"] {
            assert!(unchecked(&v, old));
        }
        // ADR 0043 (plan C03): executors leaves unchecked only when the effects
        // viewpoint is derived.
        assert_eq!(
            unchecked(&v, "executors"),
            v["agency"]["viewpoints"]["effects"]["status"] != "derived"
        );
        let fields: Vec<_> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| a["field"].as_str().unwrap())
            .collect();
        for (field, exists) in [
            ("contest.scope", has_contest),
            ("contest.reversal", has_contest),
            ("responsibility.scope", has_resp),
            ("responsibility.time_frame", has_resp),
        ] {
            assert_eq!(
                fields.iter().filter(|&&f| f == field).count(),
                usize::from(exists)
            );
        }
        if !has_resp && !has_contest {
            assert!(v.get("responsibilities").is_none());
            assert!(v.get("contest_paths").is_none());
        }
    }
    let only_standing = check(&source("resp-cut"));
    assert!(unchecked(&only_standing, "contest_path"));
    assert!(unchecked(&only_standing, "W-RESP-1"));
}

pub fn simulation_source(s: &str) -> String {
    let at = s.rfind('}').unwrap();
    format!(
        "{}\n simulation replications 4 seed 17 confidence 0.95\n{}",
        &s[..at],
        &s[at..]
    )
}

#[test]
fn r_structural_simulation_and_compare_global_rows() {
    let (_, s) = extra_inputs()
        .into_iter()
        .find(|(l, _)| l == "simulation-probability")
        .unwrap();
    // D2 only: preserve the non-encounter statistical control by targeting
    // posted. A reached returned now has a genuine nondeterministic choice.
    let s = s.replace("result nondet { fail }", "result nondet { pass }");
    let without = s
        .lines()
        .filter(|l| !new_line(l))
        .collect::<Vec<_>>()
        .join("\n");
    let with = serde_json::to_value(report::simulate(&s)).unwrap();
    let plain = serde_json::to_value(report::simulate(&without)).unwrap();
    assert_eq!(
        with["simulation"], plain["simulation"],
        "static clause must not add a Bonferroni row"
    );
    assert_eq!(with["simulation"]["multiplicity"]["rows"], 1);
    assert_eq!(with["case_outcomes"], plain["case_outcomes"]);
    let lower = |s: &str| relca_core::lower::lower(&relca_core::syntax::parse(s).unwrap()).unwrap();
    let a = relca_core::engine::simulate(&lower(&s));
    let b = relca_core::engine::simulate(&lower(&without));
    assert_eq!(a.completed_replications, b.completed_replications);
    for (left, right) in a.replications.iter().zip(&b.replications) {
        assert_eq!(
            left.draws, right.draws,
            "unreached declaration does not add random draws"
        );
        assert_eq!(left.cases.len(), right.cases.len());
        for (a, b) in left.cases.iter().zip(&right.cases) {
            assert_eq!(a.case_id, b.case_id);
            assert_eq!(a.arrival, b.arrival);
            assert_eq!(a.terminal, b.terminal);
            assert_eq!(
                a.queries, b.queries,
                "unreached runtime row is not a statistical query"
            );
            assert_eq!(a.deadlock, b.deadlock);
        }
    }
    for label in ["resp-contest", "resp-contest.broken-no-authority"] {
        let s = source(label);
        for singleton in [false, true] {
            let s = simulation_source(&if singleton {
                s.replace("result nondet { pass,fail }", "result nondet { fail }")
            } else {
                s.clone()
            });
            let v = serde_json::to_value(report::simulate(&s)).unwrap();
            let p = v["properties"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["id"] == "contract Controls #1 contest")
                .unwrap();
            assert_eq!(
                p["verdict"],
                if label == "resp-contest" {
                    "satisfied"
                } else {
                    "violated"
                }
            );
            assert_eq!(p["basis"], "structural");
            for k in ["witness", "hits", "unknown", "trials", "interval", "cases"] {
                assert!(p.get(k).is_none(), "fake {k}");
            }
            assert_eq!(
                v["verdict"],
                if label == "resp-contest" {
                    "unsupported"
                } else {
                    "violated"
                },
                "D2 choice is unsupported; preserve existing priority of known static violation"
            );
            assert!(
                v["diagnostics"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|d| d["code"] == "U-SEM-0")
            );
        }
    }
    for label in ["resp-diff", "resp-diff.broken-no-authorized-decision"] {
        let base = source(label);
        for mode in [Analysis::Exhaustive, Analysis::Simulation] {
            let s = if mode == Analysis::Simulation {
                simulation_source(
                    &base.replace("result nondet { pass,fail }", "result nondet { fail }"),
                )
            } else {
                base.clone()
            };
            let v = serde_json::to_value(compare::compare(&s, &["A", "B"], mode)).unwrap();
            assert_eq!(v["mismatch"], Value::Null, "{v}");
            assert!(
                v["tables"]["properties"]
                    .as_array()
                    .is_some_and(|r| !r.is_empty()),
                "{v}"
            );
            for (i, selection) in ["A", "B"].iter().enumerate() {
                let single = if mode == Analysis::Simulation {
                    serde_json::to_value(report::simulate_configuration(&s, selection)).unwrap()
                } else {
                    serde_json::to_value(report::check_configuration(&s, selection)).unwrap()
                };
                assert_eq!(v["plans"][i]["report"], single);
            }
            let rows: Vec<_> = v["tables"]["properties"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|r| r["id"] == "contract Controls #1 contest")
                .collect();
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0]["case_id"], Value::Null);
            assert_eq!(rows[0]["arrival"], Value::Null);
            for word in ["responsibility", "contest"] {
                assert!(v["unchecked"].as_array().unwrap().contains(&json!(word)));
            }
        }
    }
}

/// Run explicitly while production still lacks ADR 0018. Writes only inside
/// this worktree. It never updates expectations or baseline observations.
#[test]
#[ignore = "old-binary preflight, not an ADR 0018 acceptance run"]
fn r_preflight_every_authored_input() {
    let work = root().parent().unwrap().to_path_buf();
    let dest = work.join(".tmp/preflight-inputs");
    fs::create_dir_all(&dest).unwrap();
    let mut inputs = Vec::new();
    for name in FIXTURES {
        inputs.push((name.to_owned(), source(name)));
    }
    for (name, s, _) in rejection_inputs() {
        inputs.push((format!("reject-{name}"), s));
    }
    inputs.extend(extra_inputs());
    // Simulation and compare variants are generated from these same explicit bases.
    for name in [
        "resp-contest",
        "resp-contest.broken-no-authority",
        "resp-diff",
        "resp-diff.broken-no-authorized-decision",
    ] {
        for single in [false, true] {
            let s = source(name);
            inputs.push((
                format!("simulation-{name}-{single}"),
                simulation_source(&if single {
                    s.replace("result nondet { pass,fail }", "result nondet { fail }")
                } else {
                    s
                }),
            ));
        }
    }
    let mut table =
        String::from("| input | selection | exit | error diagnostics |\n|---|---|---|---|\n");
    let mut failures = Vec::new();
    let mut count = 0;
    for (name, s) in inputs {
        // This one inherited old input used an inline model declaration. Keep
        // its process while removing only the newly accepted responsibility.
        let preflight_source = s.replace("responsible p for A.a; process P", "process P");
        let skeleton = preflight_source
            .lines()
            .filter(|l| !new_line(l))
            .collect::<Vec<_>>()
            .join("\n");
        let path = dest.join(format!("{name}.relca"));
        fs::write(&path, &skeleton).unwrap();
        let selections: Vec<Option<&str>> = if s.contains("configuration A {") {
            vec![Some("A"), Some("B")]
        } else {
            vec![None]
        };
        for selection in selections {
            let mut cmd = std::process::Command::new(work.join("target/debug/relca"));
            cmd.arg("check").arg(&path);
            if let Some(c) = selection {
                cmd.args(["--configuration", c]);
            }
            let result = cmd.output().unwrap();
            let v: Value = serde_json::from_slice(&result.stdout).unwrap();
            let errors: Vec<_> = v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|d| d["severity"] == "error")
                .map(|d| d["code"].as_str().unwrap())
                .collect();
            let intended_old_rejection = name.starts_with("reject-reserved-")
                || matches!(
                    name.as_str(),
                    "reject-excludes"
                        | "reject-waive"
                        | "reject-runtime-contest"
                        | "reject-horizon"
                        | "reject-misplaced-standing"
                );
            // Removing the only static property leaves no query in this
            // intentional control. Do not silently add a synthetic verify.
            let intended_abort = name == "runtime-abort" || name == "static-only";
            if intended_old_rejection {
                assert!(errors.iter().all(|&c| c == "U-SYN-0"));
            } else if intended_abort {
                assert!(errors.iter().all(|&c| c == "U-SEM-0"));
            } else if !errors.is_empty() {
                failures.push(format!("{name}[{selection:?}]: {errors:?}"));
            }
            table.push_str(&format!(
                "| {name} | {selection:?} | {:?} | {errors:?} |\n",
                result.status.code()
            ));
            count += 1;
        }
    }
    fs::write(
        work.join(".tmp/preflight-table.md"),
        format!("{count} inputs\n\n{table}\nFailures: {failures:?}\n"),
    )
    .unwrap();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
