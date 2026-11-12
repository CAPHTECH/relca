//! Review acceptance. Oracles are derived in invoice/calc.md P1-11, not captures.
use super::*;

type Check = Result<(), String>;
fn same(actual: &Value, expected: Value) -> Check {
    if actual == &expected {
        Ok(())
    } else {
        Err(format!("expected {expected}, got {actual}"))
    }
}
fn list(v: &Value) -> Result<&Vec<Value>, String> {
    v.as_array().ok_or_else(|| format!("not an array: {v}"))
}
fn one<'a>(v: &'a Value, key: &str, name: &str) -> Result<&'a Value, String> {
    let found: Vec<_> = list(v)?.iter().filter(|r| r[key] == name).collect();
    if found.len() == 1 {
        Ok(found[0])
    } else {
        Err(format!("not unique {key}={name}"))
    }
}
fn names(v: &Value, key: &str, expected: &[&str]) -> Check {
    let mut actual: Vec<_> = list(v)?
        .iter()
        .map(|r| r[key].as_str().ok_or("missing name"))
        .collect::<Result<_, _>>()?;
    let mut expected = expected.to_vec();
    actual.sort();
    expected.sort();
    if actual == expected {
        Ok(())
    } else {
        Err(format!("inventory {actual:?} != {expected:?}"))
    }
}
fn reject_edits(v: &Value, check: impl Fn(&Value) -> Check, edits: &[(&str, Value)]) {
    check(v).unwrap();
    for (path, value) in edits {
        let mut bad = v.clone();
        *bad.pointer_mut(path)
            .unwrap_or_else(|| panic!("missing edit {path}")) = value.clone();
        assert!(check(&bad).is_err(), "survived single fault {path}");
    }
}

fn single_a(v: &Value) -> Check {
    same(&v["verdict"], json!("violated"))?;
    let q = &v["quality"];
    names(
        &q["properties"],
        "name",
        &["WrongPosting", "Resolved", "Posted"],
    )?;
    for (name, n) in [
        ("WrongPosting", "231/125000"),
        ("Resolved", "1"),
        ("Posted", "119481/125000"),
    ] {
        let p = one(&q["properties"], "name", name)?;
        same(&p["expected_count"], exact(n))?;
        same(
            &p["cases"],
            json!([{"case_id":1,"arrival":0,"min":n,"max":n,"reason":null}]),
        )?;
    }
    names(&q["cohort"], "name", &["WrongPerArrival"])?;
    let c = one(&q["cohort"], "name", "WrongPerArrival")?;
    same(&c["value"], exact("231/125000"))?;
    same(&c["denominator"], json!({"kind":"arrivals","count":1}))?;
    outcomes(q, false)?;
    names(&q["quantiles"], "name", &["PostedP95", "PostedReachedP95"])?;
    for r in list(&q["quantiles"])? {
        same(&r["value"], exact("9"))?;
        same(&r["lower_bound"], Value::Null)?;
    }
    let t = one(&v["terminals"], "end", "posted")?;
    same(&t["elapsed"], json!({"min":9,"max":9}))?;
    same(&t["work"], json!({"min":12,"max":12}))
}
fn outcomes(q: &Value, withdrawn: bool) -> Check {
    let rows = list(&q["outcomes_per_arrival"])?;
    let mut keys: Vec<_> = rows
        .iter()
        .map(|r| (r["kind"].clone(), r["end"].clone()))
        .collect();
    keys.sort_by_key(|k| format!("{k:?}"));
    let mut expected = vec![
        (json!("end"), json!("posted")),
        (json!("end"), json!("returned")),
        (json!("end"), json!("exception")),
        (json!("rejected"), Value::Null),
        (json!("unfinished"), Value::Null),
        (json!("deadlock"), Value::Null),
    ];
    expected.sort_by_key(|k| format!("{k:?}"));
    if keys != expected {
        return Err("closed outcomes".into());
    }
    for r in rows {
        if withdrawn {
            same(&r["expected_count"], aborted())?;
            same(&r["rate"], aborted())?;
        } else {
            let n = match r["end"].as_str() {
                Some("posted") => "119481/125000",
                Some("returned") => "5519/125000",
                _ => "0",
            };
            same(&r["rate"], exact(n))?;
        }
    }
    Ok(())
}
fn explanation(v: &Value) -> Check {
    single_a(v)?;
    same(&v["explanation"]["counterexamples"], json!([]))?;
    same(
        one(&v["explanation"]["omitted"], "id", "probability reached #1")?,
        json!({"id":"probability reached #1","verdict":"violated","reason":"probabilistic_property"}),
    )
}
#[test]
fn f1_single_a_check_and_explain_have_independent_unedited_oracles() {
    for id in ["p1-a-check", "p1-a-explain"] {
        // Keep all numeric fault controls on an explicit no-defer input;
        // numeric_control also checks withdrawal on the actual, unedited Cut.
        let v = numeric_control(id);
        reject_edits(
            &v,
            single_a,
            &[
                ("/quality/cohort/0/value/lower", json!("0")),
                ("/terminals/1/work/max", json!(13)),
            ],
        );
        if id.ends_with("explain") {
            reject_edits(
                &v,
                explanation,
                &[(
                    "/explanation/counterexamples",
                    json!([{"id":"probability reached #1","trace":[]}]),
                )],
            );
        }
    }
}
fn human_objective(v: &Value) -> Check {
    let r = one(
        &v["evaluation"]["objectives"],
        "objective",
        "minimize metrics.overall.named_people_work",
    )?;
    names(&r["values"], "configuration", &["AsIs", "ToBeP", "ToBeA"])?;
    for (cfg, lo, hi) in [
        ("AsIs", "14", "17"),
        ("ToBeP", "14", "17"),
        ("ToBeA", "8", "11"),
    ] {
        same(
            one(&r["values"], "configuration", cfg)?,
            json!({"configuration":cfg,"lower":lo,"upper":hi,"basis":"path_range","reason":null}),
        )?;
    }
    Ok(())
}
#[test]
fn f1_compare_transports_hand_human_work_cells() {
    let v = numeric_control("p1-compare-exhaustive");
    reject_edits(
        &v,
        human_objective,
        &[("/evaluation/objectives/0/values/0/lower", json!("13"))],
    );
}
fn sample_aborted() -> Value {
    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
}
fn aborted() -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":"aborted"})
}
fn simulation_refusal(v: &Value, busy: bool) -> Check {
    same(&v["verdict"], json!("unsupported"))?;
    if !list(&v["diagnostics"])?.iter().any(|d| {
        d["code"] == "U-SEM-0"
            && d["message"]
                .as_str()
                .is_some_and(|s| s.contains("successor actions"))
    }) {
        return Err("missing nondeterminism refusal".into());
    }
    for (key, expected) in [
        ("completed_replications", json!(0)),
        ("attempted_replications", json!(1)),
        ("replications", json!(1000)),
        ("status", json!("unsupported")),
        ("nondeterminism", json!("reject-on-encounter")),
    ] {
        same(&v["simulation"][key], expected)?;
    }
    if busy {
        same(&v["scenario"]["name"], json!("BusyDay"))?;
        same(
            &v["scenario"]["arrivals"],
            json!({"form":"fixed_every","every":4,"from":0,"until":480,"count":120}),
        )?;
        same(&v["scenario"]["horizon"], json!(960))?;
        same(&v["scenario"]["drain"], json!({"from":476,"until":960}))?;
        if v.get("quality").is_some() {
            return Err("undeclared quality".into());
        }
        same(&v["metrics"]["overall"]["work"], sample_aborted())?;
        same(
            &v["metrics"]["overall"]["named_people_work"],
            sample_aborted(),
        )?;
        for count in ["completed", "unfinished", "rejected", "cut_off"] {
            same(
                &v["metrics"]["overall"]["counts"][count],
                json!({"stats":sample_aborted(),"observed_total":null}),
            )?;
        }
    } else {
        let q = &v["quality"];
        same(&q["checked"], json!([]))?;
        names(
            &q["properties"],
            "name",
            &["WrongPosting", "Resolved", "Posted"],
        )?;
        for p in list(&q["properties"])? {
            same(&p["expected_count"], sample_aborted())?;
            same(&p["unknown_count"], sample_aborted())?;
            same(
                &p["cases"],
                json!([{"case_id":1,"arrival":0,"hits":0,"unknown":0,"trials":0}]),
            )?;
        }
        names(&q["cohort"], "name", &["WrongPerArrival"])?;
        same(
            &q["cohort"][0]["value"],
            json!({"point":null,"numerator":sample_aborted(),"denominator":sample_aborted(),"samples":0,"reason":"aborted"}),
        )?;
        names(&q["quantiles"], "name", &["PostedP95", "PostedReachedP95"])?;
        for r in list(&q["quantiles"])? {
            same(
                &r["value"],
                json!({"point":null,"observations":0,"censored":0,"reason":"aborted"}),
            )?;
            same(&r["lower_bound"], Value::Null)?;
        }
        // Validate the six declared outcomes even though the sampled value type differs.
        let mut inventory = q.clone();
        for r in inventory["outcomes_per_arrival"]
            .as_array_mut()
            .ok_or("outcomes")?
        {
            same(&r["expected_count"], sample_aborted())?;
            same(
                &r["rate"],
                json!({"point":null,"samples":0,"reason":"aborted"}),
            )?;
            r["expected_count"] = aborted();
            r["rate"] = aborted();
        }
        outcomes(&inventory, true)?;
    }
    Ok(())
}
fn compare_refusal(v: &Value, busy: bool) -> Check {
    same(&v["verdict"], json!("unsupported"))?;
    names(&v["plans"], "configuration", &["AsIs", "ToBeP", "ToBeA"])?;
    for p in list(&v["plans"])? {
        simulation_refusal(&p["report"], busy)?;
    }
    Ok(())
}
#[test]
fn f1_simulation_entries_refuse_without_fabricating_samples() {
    let v = live("p1-a-simulate");
    reject_edits(
        &v,
        |v| simulation_refusal(v, false),
        &[
            ("/quality/cohort/0/value/point", json!("0")),
            ("/quality/properties/0/cases/0/trials", json!(1)),
        ],
    );
    for (id, busy) in [
        ("p1-compare-simulation", false),
        ("busyday-compare-simulation", true),
    ] {
        let v = live(id);
        let path = if busy {
            "/plans/1/report/metrics/overall/work/mean"
        } else {
            "/plans/1/report/quality/cohort/0/value/point"
        };
        reject_edits(
            &v,
            |v| compare_refusal(v, busy),
            &[
                (path, json!("0")),
                (
                    "/plans/1/report/simulation/completed_replications",
                    json!(1),
                ),
            ],
        );
        let mut bad = v.clone();
        bad["plans"].as_array_mut().unwrap().remove(1);
        assert!(compare_refusal(&bad, busy).is_err());
    }
    for id in [
        "busyday-as-is-simulate",
        "busyday-to-be-p-simulate",
        "busyday-to-be-a-simulate",
    ] {
        let v = live(id);
        reject_edits(
            &v,
            |v| simulation_refusal(v, true),
            &[
                ("/scenario/arrivals/count", json!(119)),
                ("/metrics/overall/work/mean", json!("0")),
            ],
        );
    }
}
fn rule_quality(v: &Value, b: bool) -> Check {
    let q = &v["quality"];
    same(
        &q["checked"],
        json!([
            "case_reachability",
            "expected_counts",
            "ratio_of_expected_counts",
            "denominator_disclosure",
            "outcomes_per_arrival"
        ]),
    )?;
    names(&q["properties"], "name", &["WrongPosting", "Resolved"])?;
    for (name, cond) in [
        (
            "WrongPosting",
            "registered == true && extraction_correct == false",
        ),
        ("Resolved", "end.posted || end.returned"),
    ] {
        let p = one(&q["properties"], "name", name)?;
        same(&p["condition"], json!(cond))?;
        let n = if name == "Resolved" {
            "1"
        } else if b {
            "81/12500"
        } else {
            "231/125000"
        };
        same(&p["expected_count"], exact(n))?;
        same(
            &p["cases"],
            json!([{"case_id":1,"arrival":0,"min":n,"max":n,"reason":null}]),
        )?;
    }
    names(&q["cohort"], "name", &["WrongPerArrival"])?;
    let c = &q["cohort"][0];
    same(&c["numerator"], json!("WrongPosting"))?;
    same(&c["denominator"], json!({"kind":"arrivals","count":1}))?;
    same(
        &c["value"],
        exact(if b { "81/12500" } else { "231/125000" }),
    )?;
    same(&c["bounds_with_undetermined"], Value::Null)?;
    let mut keys: Vec<_> = list(&q["outcomes_per_arrival"])?
        .iter()
        .map(|r| format!("{}:{}", r["kind"], r["end"]))
        .collect();
    keys.sort();
    let mut expected: Vec<_> = [
        ("end", Some("posted")),
        ("end", Some("returned")),
        ("end", Some("exception")),
        ("rejected", None),
        ("unfinished", None),
        ("deadlock", None),
    ]
    .iter()
    .map(|(kind, end)| format!("{}:{}", json!(kind), json!(end)))
    .collect();
    expected.sort();
    if keys != expected {
        return Err("closed Rule outcomes".into());
    }
    for r in list(&q["outcomes_per_arrival"])? {
        let n = match (b, r["end"].as_str()) {
            (false, Some("posted")) => "119481/125000",
            (false, Some("returned")) => "5519/125000",
            (true, Some("posted")) => "6003/6250",
            (true, Some("returned")) => "247/6250",
            _ => "0",
        };
        same(&r["expected_count"], exact(n))?;
        same(&r["rate"], exact(n))?;
    }
    for (id, verdict) in [
        (
            "contract InvoiceControls #1 invariant",
            if b { "violated" } else { "satisfied" },
        ),
        ("contract InvoiceControls #2 invariant", "satisfied"),
        ("contract InvoiceControls #5 separation", "vacuous"),
    ] {
        same(&one(&v["properties"], "id", id)?["verdict"], json!(verdict))?;
    }
    Ok(())
}
#[test]
fn f2_rule_withdraws_all_numeric_groups_but_retains_declarations_and_contract_ids() {
    for (id, b) in [("rule-a-check", false), ("rule-b-check", true)] {
        let v = numeric_control(id);
        reject_edits(
            &v,
            |v| rule_quality(v, b),
            &[
                ("/quality/cohort/0/value/lower", json!("0")),
                ("/quality/properties/0/expected_count/upper", json!("0")),
                ("/quality/properties/1/cases/0/min", json!("0")),
                ("/quality/outcomes_per_arrival/0/rate/lower", json!("0")),
                ("/quality/properties", json!([])),
                ("/quality/cohort/0/denominator/count", Value::Null),
            ],
        );
        let mut bad = v.clone();
        let p = bad["properties"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == "contract InvoiceControls #1 invariant")
            .unwrap();
        p["verdict"] = json!(if b { "satisfied" } else { "violated" });
        assert!(rule_quality(&bad, b).is_err());
    }
}
fn differences(v: &Value, rule: bool) -> Check {
    let expected = if rule {
        vec![
            ("actor", "Approver", "removed"),
            ("relation", "Approver.approve", "removed"),
            ("authority", "Approver.approve", "removed"),
            ("resource", "Approver", "removed"),
            ("rule", "ApprovalRule", "modified"),
        ]
    } else {
        vec![
            ("actor", "Extractor", "added"),
            ("relation", "Extractor.extract", "added"),
            ("relation", "Clerk.enter", "removed"),
            ("authority", "Clerk.enter", "removed"),
            ("authority", "Extractor.extract", "added"),
            ("resource", "Extractor", "added"),
            ("time", "Clerk.match_po", "modified"),
            ("time", "Clerk.match_receipt", "modified"),
            ("visibility", "Invoice", "modified"),
            ("relation", "entry", "modified"),
        ]
    };
    let rows = list(&v["differences"])?;
    let mut actual: Vec<_> = rows
        .iter()
        .map(|r| {
            (
                r["kind"].as_str().unwrap_or(""),
                r["target"].as_str().unwrap_or(""),
                r["change"].as_str().unwrap_or(""),
            )
        })
        .collect();
    let mut expected = expected;
    actual.sort();
    expected.sort();
    if actual != expected {
        return Err(format!("diff inventory {actual:?}"));
    }
    let row = |kind: &str, target: &str| {
        rows.iter()
            .find(|r| r["kind"] == kind && r["target"] == target)
            .ok_or_else(|| "diff row".to_owned())
    };
    if rule {
        let r = row("rule", "ApprovalRule")?;
        same(
            &r["from"],
            json!({"version":"r2026-04","values":[{"name":"approval_required","value":true}],"amendment":null}),
        )?;
        same(
            &r["to"],
            json!({"version":"r2026-05","values":[{"name":"approval_required","value":false}],"amendment":{"from":"r2026-04","to":"r2026-05","by":"cfo","under":"RuleChange","under_version":"r2026-04"}}),
        )?;
    } else {
        // ADR0051 hand nucleus: only entry changes producer; no intent or
        // annotation covers it, so W1/W3 each gain one (ADR0051 output/intent).
        same(
            rows.last().unwrap(),
            json!({
                "kind":"relation","target":"entry","change":"modified","reason":null,"loses":null,
                "from":{"producer":"Clerk.enter"},"to":{"producer":"Extractor.extract"}
            }),
        )?;
        let a = row("actor", "Extractor")?;
        let m = one(&a["annotations"], "kind", "maintain")?;
        same(&m["person"], json!("s1"))?;
        same(
            &m["links"],
            json!([{"actor":"Extractor","from_duties":[{"id":2,"person":"s1"}],"to_duties":[{"id":2,"person":"s1"}],"status":"linked"}]),
        )?;
        same(
            &row("relation", "Clerk.enter")?["annotations"],
            json!([{"kind":"keep_fallback"}]),
        )?;
        same(
            &row("authority", "Extractor.extract")?["to"]["responsibilities"],
            json!([{"id":2,"person":"s1","target":{"kind":"maintenance","value":"Extractor.maintenance"},"within":null}]),
        )?;
    }
    Ok(())
}
#[test]
fn f3_diff_rows_and_intent_links_are_closed_by_name() {
    for (id, rule) in [("p1-diff", false), ("rule-diff", true)] {
        let v = live(id);
        let mut edits = vec![("/differences", json!([]))];
        if rule {
            edits.push(("/differences/4/to/amendment/by", json!("a1")));
            edits.push(("/differences/4/to/values/0/value", json!(true)));
        } else {
            edits.push(("/differences/0/annotations", json!([])));
            edits.push((
                "/differences/0/annotations/0/links/0/to_duties/0/person",
                json!("a1"),
            ));
            edits.push(("/differences/2/annotations", json!([])));
            edits.push(("/differences/9/from/producer", json!("Extractor.extract")));
            edits.push(("/differences/9/to/producer", json!("Clerk.enter")));
        }
        reject_edits(&v, |v| differences(v, rule), &edits);
    }
}

const REQUIRES: &str = r#"model RequiresInvoice version "0.2" {
 type Verdict = enum { pass, fail }
 type Ack = enum { committed, rejected_duplicate, held }
 observed case { approval: optional<Verdict>; po: Verdict = pass; receipt: Verdict = pass; registered: bool = false }
 actor Ledger { capacity 1; state keys: set<CaseId>
   capability post(key:CaseId) -> Ack {
     requires approval == pass && po == pass && receipt == pass else held
     commits; duration fixed 1min; writes [keys,registered]
     atomic { if key in keys { result rejected_duplicate } else { keys += key; registered = true; result committed } }
     cancel forbidden_after_start
   }
 }
 cut C { include [Ledger] }
 process P { posted = Ledger.post(case.id); if posted == held { end held } else { end posted } }
 bounds { cases 1; live_tasks 1; clock 5min }
 metrics { window 5min; currency "JPY" }
 contract InvoiceControls {
   invariant registered -> approval == pass
   at_most_once Ledger.post.commit per case.id
   response Ledger.post.start -> one_of(Ledger.post.commit) by 1min after Ledger.post.start otherwise unresolved
 }
 verify safety no_resource_overcommit
}"#;
fn requires_result(v: &Value, held: bool) -> Check {
    let n = if held { 0 } else { 1 };
    same(
        &v["verdict"],
        json!(if held { "vacuous" } else { "violated" }),
    )?;
    let t = one(&v["terminals"], "end", if held { "held" } else { "posted" })?;
    same(&t["elapsed"], json!({"min":n,"max":n}))?;
    same(&t["work"], json!({"min":n,"max":n}))?;
    same(
        &one(
            &v["properties"],
            "id",
            "contract InvoiceControls #1 invariant",
        )?["verdict"],
        json!(if held { "vacuous" } else { "violated" }),
    )?;
    // The start-triggered obligation is vacuous iff there was no start.
    same(
        &one(
            &v["properties"],
            "id",
            "contract InvoiceControls #3 response",
        )?["verdict"],
        json!(if held { "vacuous" } else { "satisfied" }),
    )?;
    same(
        &one(
            &v["properties"],
            "id",
            "contract InvoiceControls #2 at_most_once",
        )?["verdict"],
        json!(if held { "vacuous" } else { "satisfied" }),
    )?;
    if held {
        same(
            &v["capability_refusals"],
            json!([{"case_id":1,"call":"posted","capability":"Ledger.post","result":"held","at":{"min":0,"max":0},"coverage":"complete"}]),
        )?;
    } else {
        // No requires declaration remains: the optional report field is absent,
        // not an empty table borrowed from the requires-bearing input.
        if v.get("capability_refusals").is_some() {
            return Err("undeclared refusal inventory".into());
        }
    }
    same(
        &v["metrics"]["overall"]["work"],
        range(&n.to_string(), &n.to_string()),
    )
}
#[test]
fn f4_requires_refusal_is_not_start_work_or_commit() {
    for held in [true, false] {
        let s = if held {
            REQUIRES.to_owned()
        } else {
            change(
                REQUIRES,
                "requires approval == pass && po == pass && receipt == pass else held",
                "",
            )
        };
        let (v, exit) = edited(&s, "check", &[]);
        assert_eq!(exit, if held { 5 } else { 1 }, "{v}");
        let path = "/terminals/0/work/max";
        reject_edits(&v, |v| requires_result(v, held), &[(path, json!(9))]);
        let mut bad = v.clone();
        let response = bad["properties"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|p| p["id"] == "contract InvoiceControls #3 response")
            .unwrap();
        response["verdict"] = json!(if held { "satisfied" } else { "vacuous" });
        assert!(requires_result(&bad, held).is_err());
    }
}
fn diagnostic(v: &Value, code: &str) -> Check {
    if list(&v["diagnostics"])?.iter().any(|d| d["code"] == code) {
        Ok(())
    } else {
        Err(format!("missing {code}"))
    }
}
#[test]
fn minor3_not_by_rejects_even_an_explicitly_granted_amender() {
    let s = change(
        &source("rules"),
        "grants cfo to change ApprovalRule",
        "grants [cfo,a1] to change ApprovalRule",
    );
    let s = change(&s, "by cfo under RuleChange", "by a1 under RuleChange");
    let (v, exit) = edited(&s, "check", &["--configuration", "ToBeB"]);
    assert_eq!(exit, 3, "{v}");
    reject_edits(
        &v,
        |v| {
            same(&v["verdict"], json!("invalid"))?;
            diagnostic(v, "E-CON-0")
        },
        &[("/diagnostics", json!([]))],
    );
    let allowed = change(&s, "not_by [a1];", "");
    let (v, exit) = edited(&allowed, "check", &["--configuration", "ToBeB"]);
    assert_eq!(exit, 1, "{v}");
    deferred_invoice::assert_deferred(&v);
    let out = deferred_invoice::numeric_control(&allowed, "check", &["--configuration", "ToBeB"]);
    assert_eq!(out.status.code(), Some(1));
    rule_quality(&serde_json::from_slice(&out.stdout).unwrap(), true).unwrap();
    assert!(
        !v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "E-CON-0")
    );
}
fn amendment_result(v: &Value, amended: bool) -> Check {
    let rule = &one(&v["actors"], "name", "ApprovalRule")?["rule"];
    same(
        &rule["version"]["effective"],
        json!(if amended { "r2026-05" } else { "r2026-04" }),
    )?;
    same(
        &one(&rule["values"], "name", "approval_required")?["effective"],
        json!(!amended),
    )?;
    let end = if amended { "posted" } else { "exception" };
    let t = one(&v["terminals"], "end", end)?;
    same(
        &t["elapsed"],
        json!({"min":if amended {7} else {6},"max":if amended {7} else {6}}),
    )?;
    same(
        &t["work"],
        json!({"min":if amended {10} else {9},"max":if amended {10} else {9}}),
    )?;
    same(
        &one(
            &v["properties"],
            "id",
            "contract InvoiceControls #1 invariant",
        )?["verdict"],
        json!(if amended { "violated" } else { "vacuous" }),
    )?;
    if !amended {
        if list(&v["terminals"])?.iter().any(|t| t["end"] == "posted") {
            return Err("unamended posting".into());
        }
        let r = one(&v["capability_refusals"], "capability", "Ledger.post")?;
        same(&r["result"], json!("held"))?;
        same(&r["at"], json!({"min":6,"max":6}))?;
    }
    Ok(())
}
#[test]
fn minor4_removing_only_amendment_leaves_rule_true_and_post_held() {
    let base = source("rules");
    let no = change(
        &base,
        "amend ApprovalRule from \"r2026-04\" to \"r2026-05\" by cfo under RuleChange { approval_required = false }",
        "",
    );
    // A change-rule intent cannot describe an absent amendment; leave it out in
    // BOTH members of the executable pair. The only inter-member edit is amend.
    let intent = "intent { change rule ApprovalRule reason \"full exemption is structural only, not a low-risk classification\" }";
    let yes = change(&base, intent, "");
    let no = change(&no, intent, "");
    for (s, amended) in [(&yes, true), (&no, false)] {
        let (v, exit) = edited(s, "check", &["--configuration", "ToBeB"]);
        // The old static contest is violated without an active approver on B,
        // even when requires prevents posting; it wins over vacuous clauses.
        assert_eq!(exit, 1, "{v}");
        amendment_result(&v, amended).unwrap();
        let mut bad = v.clone();
        let t = bad["terminals"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|t| t["end"] == if amended { "posted" } else { "exception" })
            .unwrap();
        t["work"]["max"] = json!(99);
        assert!(amendment_result(&bad, amended).is_err());
    }
}
fn duplicate(v: &Value, lead: u64, work: u64) -> Check {
    let t = one(&v["terminals"], "end", "posted")?;
    same(&t["elapsed"], json!({"min":lead,"max":lead}))?;
    same(&t["work"], json!({"min":work,"max":work}))?;
    let p = one(
        &v["properties"],
        "id",
        "contract InvoiceControls #3 at_most_once",
    )?;
    same(&p["verdict"], json!("satisfied"))?;
    if p.get("notes").is_some() {
        return Err("mechanism_unmodeled note".into());
    }
    Ok(())
}
fn causes(v: &Value, ai: bool) -> Check {
    let c = &v["agency"]["common_causes"]["causes"];
    names(c, "latent", &["Invoice.difficulty"])?;
    let targets = &c[0]["targets"];
    names(targets, "call", &["entry", "poV", "receiptV", "approvalV"])?;
    for (call, cap, kernel) in [
        (
            "entry",
            if ai {
                "Extractor.extract"
            } else {
                "Clerk.enter"
            },
            if ai { "ai_entry" } else { "human_entry" },
        ),
        ("poV", "Clerk.match_po", "po_match"),
        ("receiptV", "Clerk.match_receipt", "receipt_match"),
        ("approvalV", "Approver.approve", "approval_check"),
    ] {
        same(
            one(targets, "call", call)?,
            json!({"call":call,"capability":cap,"kernel":kernel}),
        )?;
    }
    Ok(())
}
#[test]
fn minor2_all_three_duplicate_key_totals_and_four_common_cause_calls() {
    let s = change(
        &source("phase1"),
        "if posted == committed { end posted } else { end exception }",
        "again = Ledger.post(case.id,poV,receiptV,approvalV)\nif again == rejected_duplicate { end posted } else { end exception }",
    );
    for (cfg, code, lead, work, id) in [
        ("AsIs", 0, 18, 18, "p1-as-is-check"),
        ("ToBeP", 0, 15, 18, "p1-to-be-p-check"),
        ("ToBeA", 1, 10, 13, "p1-to-be-a-check"),
    ] {
        let (v, exit) = edited(&s, "check", &["--configuration", cfg]);
        assert_eq!(exit, code, "{v}");
        duplicate(&v, lead, work).unwrap();
        let mut bad = v.clone();
        let t = bad["terminals"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|t| t["end"] == "posted")
            .unwrap();
        t["work"]["max"] = json!(work + 1);
        assert!(duplicate(&bad, lead, work).is_err());
        let v = live(id);
        reject_edits(
            &v,
            |v| causes(v, cfg == "ToBeA"),
            &[("/agency/common_causes/causes/0/targets", json!([]))],
        );
    }
}
const INTEGER: &str = r#"model IntegerConditional version "0.2" {
 type V = enum { pass }
 observed case { n: 0..3 = 0 }
 latent case { flag: bool = true }
 kernel K given [flag] { true -> { 1: pass }; false -> { 1: pass } }
 actor A { capacity 1; capability run() -> V { duration fixed 1min; kernel_reads latent [flag]; outcomes kernel K } }
 process P { x = A.run(); end done }
 cut C { include [A] }
 bounds { cases 1; live_tasks 1; clock 5min }
 verify safety no_resource_overcommit
}"#;
fn integer_agency(v: &Value) -> Check {
    same(&v["verdict"], json!("satisfied"))?;
    if !v["agency"].is_object() {
        return Err("integer domain erased Agency".into());
    }
    let t = one(&v["terminals"], "end", "done")?;
    same(&t["elapsed"], json!({"min":1,"max":1}))?;
    same(&t["work"], json!({"min":1,"max":1}))
}
#[test]
fn minor5_bounded_integer_and_conditional_kernel_keep_agency_end_to_end() {
    for s in [
        INTEGER.to_owned(),
        change(INTEGER, "outcomes kernel K", "result nondet { pass }"),
    ] {
        let (v, exit) = edited(&s, "check", &[]);
        assert_eq!(exit, 0, "{v}");
        integer_agency(&v).unwrap();
        let mut bad = v.clone();
        bad.as_object_mut().unwrap().remove("agency");
        assert!(integer_agency(&bad).is_err());
    }
}

fn ast_items(src: &str, keep: impl Fn(&ast::Item) -> bool) -> Result<Vec<String>, String> {
    let m = syntax::parse(src).map_err(|e| format!("{e:?}"))?;
    Ok(m.items.iter().filter(|i| keep(i)).map(structure).collect())
}
fn ast_equal(actual: Vec<String>, expected: Vec<String>) -> Check {
    if actual == expected {
        Ok(())
    } else {
        let mismatch = actual.iter().zip(&expected).position(|(a, b)| a != b);
        Err(format!(
            "AST lengths {}/{}; first mismatch {:?}: actual={:?}; expected={:?}",
            actual.len(),
            expected.len(),
            mismatch,
            mismatch.map(|i| &actual[i]),
            mismatch.map(|i| &expected[i])
        ))
    }
}
fn common_item(i: &ast::Item) -> bool {
    !matches!(
        i,
        ast::Item::Process(_) | ast::Item::Configuration(_) | ast::Item::Compare(_)
    )
}
fn rule_ast(src: &str) -> Check {
    let mut expected = source("phase1");
    for (from, to) in [
        (
            "actor ReceiptRecord { source record }",
            "actor ReceiptRecord { source record }\nactor ApprovalRule { grain rule; control policy; version \"r2026-04\"; decides approval_required: bool = true }",
        ),
        (
            "  responsible a1",
            "  authority RuleChange { basis \"prior rule amendment procedure\"; grants cfo to change ApprovalRule; not_by [a1]; under ApprovalRule \"r2026-04\" }\n  responsible a1",
        ),
        (
            "post(key:CaseId,p:Verdict,r:Verdict,a:Verdict)",
            "post(key:CaseId)",
        ),
        (
            "requires approval == pass && po == pass && receipt == pass else held",
            "requires (ApprovalRule.approval_required == false || approval == pass) && po == pass && receipt == pass else held",
        ),
        (
            "reads [entry,approval]",
            "reads [entry,approval,po,receipt]",
        ),
        (
            "question \"One receipt of one invoice\"",
            "question \"One receipt of one invoice; full approval exemption is a structural approximation only\"",
        ),
        (
            "include [Invoice,PurchaseOrder,ReceiptRecord,Clerk",
            "include [Invoice,PurchaseOrder,ReceiptRecord,ApprovalRule,Clerk",
        ),
        (
            "    externality",
            "    defer LowRiskExemption reason \"case-classification rule values are unsupported\" loses \"risk of selectively exempted cases\"\n    externality",
        ),
        ("simulation replications 1000 seed 0 confidence 0.95", ""),
        (
            "verify probability reached registered == true && extraction_correct == false <= 0.001",
            "",
        ),
        ("property case.Posted = reached(end.posted)", ""),
        (
            "metric quantile.PostedP95 = quantile(0.95, case.Posted)",
            "",
        ),
        (
            "metric quantile.PostedReachedP95 = quantile(0.95, case.Posted given reached)",
            "",
        ),
    ] {
        expected = change(&expected, from, to);
    }
    ast_equal(
        ast_items(src, common_item)?,
        ast_items(&expected, common_item)?,
    )?;
    // Process skeleton: A only changes Ledger's actual arguments. B removes
    // the approval call and its guard, not an arbitrary suffix or all guards.
    let p = syntax::parse(&source("phase1")).unwrap();
    let a = p
        .items
        .iter()
        .find_map(|i| {
            if let ast::Item::Process(p) = i {
                (p.name.value == "ToBeAFlow").then_some(p)
            } else {
                None
            }
        })
        .unwrap();
    let base = source("phase1");
    let a_src = change(
        &base[a.span.start..a.span.end],
        "Ledger.post(case.id,poV,receiptV,approvalV)",
        "Ledger.post(case.id)",
    );
    let b_src = change(
        &change(&a_src, "ToBeAFlow", "ToBeBFlow"),
        "approvalV = Approver.approve(entry,poV,receiptV)\n    if approvalV == pass {} else { end returned }",
        "",
    );
    let expected_flows = format!("model Expected version \"0.2\" {{ {a_src} {b_src} }}");
    ast_equal(
        ast_items(src, |i| matches!(i, ast::Item::Process(_)))?,
        ast_items(&expected_flows, |_| true)?,
    )?;
    let configs = r#"model Expected version "0.2" {
      configuration ToBeA { cut InvoiceCut; process ToBeAFlow }
      configuration ToBeB { cut InvoiceCut; process ToBeBFlow
        amend ApprovalRule from "r2026-04" to "r2026-05" by cfo under RuleChange { approval_required = false }
        intent { change rule ApprovalRule reason "full exemption is structural only, not a low-risk classification" }
      }
    }"#;
    ast_equal(
        ast_items(src, |i| {
            matches!(i, ast::Item::Configuration(_) | ast::Item::Compare(_))
        })?,
        ast_items(configs, |_| true)?,
    )
}
fn busy_ast(src: &str) -> Check {
    // Connect the old enum presentation to the new five-model kernel seam.
    // This is the sole result-presentation edit, not a probability projection.
    let enum_kernels = change(&source("phase1"), "Entry { confidence = high }", "high");
    ast_equal(
        ast_items(src, |i| matches!(i, ast::Item::Kernel(_)))?,
        ast_items(&enum_kernels, |i| matches!(i, ast::Item::Kernel(_)))?,
    )?;
    let mut expected = source("configurations");
    for (from, to) in [
        (
            "actor Invoice { source document",
            "actor Invoice { instance per_case; source document",
        ),
        (
            "question \"One receipt of one invoice\"",
            "question \"One receipt per invoice under artificial fixed arrivals\"",
        ),
        (
            "reentry [contest Vendor -> Clerk.recheck, review cut quarterly by cfo]",
            "reentry [contest Vendor -> Clerk.recheck, review cut quarterly by cfo]\nhorizon receipt..resolution within 60min\nhorizon contest within 5d structural_only",
        ),
        ("window 60min", "window 960min"),
    ] {
        expected = change(&expected, from, to);
    }
    ast_equal(
        ast_items(src, |i| !matches!(i, ast::Item::Scenario(_)))?,
        ast_items(&expected, |i| {
            !matches!(i, ast::Item::Bounds(_) | ast::Item::Simulation(_))
        })?,
    )?;
    let scenario = r#"model Expected version "0.2" { scenario BusyDay {
        arrivals fixed_every 4min during [0min,480min)
        queue capacity 200 on_overflow reject
        bounds { cases 120; live_tasks 2; clock 960min }
        horizon 960min
        simulation replications 1000 seed 41020 confidence 0.95
    } }"#;
    ast_equal(
        ast_items(src, |i| matches!(i, ast::Item::Scenario(_)))?,
        ast_items(scenario, |_| true)?,
    )
}
#[test]
fn f5_rules_and_busyday_ast_have_only_individually_enumerated_differences() {
    let rules = source("rules");
    rule_ast(&rules).unwrap();
    let busy = source("busyday");
    busy_ast(&busy).unwrap();
    assert!(rule_ast(&change(&rules, "0.85: Entry", "0.84: Entry")).is_err());
    assert!(busy_ast(&change(&busy, "0.4: pass", "0.3: pass")).is_err());
    // Atomic, requires and per_case are not globally masked by either seam.
    assert!(rule_ast(&change(&rules, "keys += key;", " ")).is_err());
    assert!(rule_ast(&change(&rules, "else held", "else committed")).is_err());
    assert!(busy_ast(&change(&busy, "instance per_case;", "")).is_err());
}
