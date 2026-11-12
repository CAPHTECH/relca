//! Closed transport, downstream gates, and independent multi-case arithmetic.
#[path = "support/r2_hand.rs"]
mod h;
#[path = "support/r2_inputs.rs"]
mod i;
use relca_core::{
    compare::{self, Analysis},
    diff, report, sensitivity,
};
use serde_json::{Value, json};
fn control() -> Value {
    let v = json!(report::check(&i::source("R01")));
    h::check(&v, "R01");
    h::strict_roundtrip(&v);
    v
}
fn decode_reject(v: Value) {
    assert!(serde_json::from_value::<report::Report>(v).is_err());
}
#[test]
fn recovery_wire_is_closed_at_every_object_and_nullable_fields_are_required() {
    let v = control();
    for path in [
        "/response_recovery",
        "/response_recovery/paths/0",
        "/response_recovery/paths/0/commands/0",
        "/response_recovery/paths/0/commands/0/queries/0",
        "/response_recovery/paths/0/commands/0/resolutions/0",
        "/response_recovery/paths/0/commands/0/tasks/0",
    ] {
        let obj = v.pointer(path).unwrap().as_object().unwrap();
        for k in obj.keys() {
            let mut bad = v.clone();
            bad.pointer_mut(path)
                .unwrap()
                .as_object_mut()
                .unwrap()
                .remove(k);
            decode_reject(bad);
        }
        let mut bad = v.clone();
        bad.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("invented".into(), json!(0));
        decode_reject(bad);
    }
    for (path, keys) in [
        (
            "/response_recovery",
            vec!["rules", "coverage", "scope", "paths"],
        ),
        ("/response_recovery/paths/0", vec!["id", "commands"]),
        (
            "/response_recovery/paths/0/commands/0",
            vec![
                "case_id",
                "command",
                "provider",
                "capability",
                "saved_key",
                "original_site",
                "queries",
                "resolutions",
                "tasks",
                "unresolved",
                "stop_reason",
            ],
        ),
        (
            "/response_recovery/paths/0/commands/0/queries/0",
            vec!["id", "binding", "site", "at", "status", "evidence"],
        ),
        (
            "/response_recovery/paths/0/commands/0/resolutions/0",
            vec!["query_ref", "site", "at", "fact"],
        ),
        (
            "/response_recovery/paths/0/commands/0/tasks/0",
            vec![
                "binding",
                "capability",
                "executor",
                "result",
                "started_at",
                "finished_at",
                "site",
            ],
        ),
    ] {
        let mut actual = v
            .pointer(path)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<Vec<_>>();
        let mut expected = keys;
        actual.sort();
        expected.sort();
        assert_eq!(actual, expected, "{path}");
    }
}
#[test]
fn recovery_wire_versions_evidence_references_and_nulls_are_semantic_not_only_shape() {
    let v = control();
    for (path, bad) in [
        ("/response_recovery/rules", json!("response-recovery-p3a-2")),
        ("/response_recovery/coverage", json!("success_rate")),
        ("/response_recovery/paths/0/commands/0/saved_key", json!(2)),
        ("/response_recovery/paths/0/commands/0/case_id", json!(2)),
        (
            "/response_recovery/paths/0/commands/0/provider",
            json!("Other"),
        ),
        (
            "/response_recovery/paths/0/commands/0/command",
            json!("another"),
        ),
        (
            "/response_recovery/paths/0/commands/0/queries/0/status",
            json!("not_committed"),
        ),
        (
            "/response_recovery/paths/0/commands/0/queries/0/status",
            json!("still_unknown"),
        ),
        (
            "/response_recovery/paths/0/commands/0/queries/0/evidence",
            Value::Null,
        ),
        (
            "/response_recovery/paths/0/commands/0/resolutions/0/query_ref",
            json!("absent"),
        ),
        (
            "/response_recovery/paths/0/commands/0/resolutions/0/fact",
            json!("not_committed"),
        ),
        (
            "/response_recovery/paths/0/commands/0/resolutions/0/at",
            json!(2),
        ),
        (
            "/response_recovery/paths/0/commands/0/unresolved",
            json!(true),
        ),
        (
            "/response_recovery/paths/0/commands/0/stop_reason",
            json!("receipt_unknown"),
        ),
        ("/response_recovery/paths/0/commands/0/queries", json!([])),
        (
            "/ledger/derivation/bundle",
            json!("relca-conclusions-p2-uncertainty-2"),
        ),
    ] {
        let mut b = v.clone();
        *b.pointer_mut(path).unwrap() = bad;
        decode_reject(b);
    }
    let u = json!(report::check(&i::source("R02")));
    h::check(&u, "R02");
    h::strict_roundtrip(&u);
    assert_eq!(
        u["response_recovery"]["paths"][0]["commands"][0]["queries"][0].get("evidence"),
        Some(&Value::Null)
    );
    let mut b = u.clone();
    b["case_outcomes"][0]["terminals"][0]["completed_at"] = h::point(3);
    decode_reject(b);
    let mut b = u;
    b["response_recovery"]["paths"][0]["commands"][0]["queries"][0]["evidence"] = json!("forged");
    decode_reject(b);
}
#[test]
fn joint_path_query_and_resolution_ids_cannot_cross_cases_or_reports() {
    let v = json!(report::check(&i::desk()));
    h::admitted(&v);
    h::strict_roundtrip(&v);
    for path in h::rows(&v["response_recovery"]["paths"]) {
        let cs = h::rows(&path["commands"]);
        assert_eq!(cs.len(), 2);
        assert_eq!(cs[0]["saved_key"], 1);
        assert_eq!(cs[1]["saved_key"], 2);
        assert_ne!(cs[0]["queries"][0]["id"], cs[1]["queries"][0]["id"]);
    }
    let mut bad = v.clone();
    bad["response_recovery"]["paths"][0]["commands"][0]["resolutions"][0]["query_ref"] =
        v["response_recovery"]["paths"][0]["commands"][1]["queries"][0]["id"].clone();
    decode_reject(bad);
}
#[test]
fn desk_resource_serializes_queries_not_cases_or_entire_handlers() {
    let v = json!(report::check(&i::desk()));
    assert_eq!(v["bounds"]["truncated"], false, "{}", v["diagnostics"]);
    for index in 0..2 {
        let c = &v["case_outcomes"][index];
        assert_eq!(c["case_id"], index + 1);
        let t = h::row(&c["terminals"], "end", "shipped");
        assert_eq!(t["elapsed"], json!({"min":7,"max":9}));
        assert_eq!(t["work"], h::point(7));
        let tasks = &v["metrics"]["cases"][index]["tasks"];
        assert_eq!(
            h::row(tasks, "binding", "status")["wait"],
            json!({"min":"0","max":"2","reason":null})
        );
        let b = &h::row(&v["metrics"]["cases"][index]["terminals"], "end", "shipped")["breakdown"];
        assert_eq!(b["in_service"], h::scalar("7"));
        assert_eq!(
            b["queue"]["resource"],
            json!({"min":"0","max":"2","reason":null})
        );
    }
    assert_eq!(v["metrics"]["overall"]["work"], h::scalar("14"));
    let p = h::row(&v["metrics"]["actors"], "actor", "Psp");
    assert_eq!(p["queue"]["wait_total"]["stats"], h::scalar("2"));
    assert_eq!(p["queue"]["length_time_average"]["stats"], h::scalar("2/9"));
    assert_eq!(
        h::row(&v["metrics"]["resources"], "resource", "QueryDesk")["occupancy_time"],
        h::scalar("4")
    );
}
#[test]
fn refund_approval_separation_and_authority_keep_their_own_contract_violations() {
    for (old, new, contract) in [
        (
            "executors [c]; control policy",
            "executors [f]; control policy",
            "separation",
        ),
        (
            "grants f to Finance.approve_refund",
            "grants q to Finance.approve_refund",
            "authority_basis",
        ),
        ("if v == approve {", "if v == reject {", "invariant"),
    ] {
        let base = if contract == "invariant" {
            i::source("R09")
        } else {
            i::source("R04")
        };
        let s = i::edit(&base, old, new);
        let v = json!(report::check(&s));
        h::verdict(&v, "violated");
        assert!(h::rows(&v["properties"]).iter().any(|p|p["id"].as_str().unwrap().contains(contract)&&p["verdict"]=="violated"),"{contract}: {}",v["diagnostics"]);
    }
}
#[test]
fn refund_cannot_declare_g_set_writer_or_overwrite_protected_charge_truth() {
    for (old, new) in [
        (
            "writes refunded; finish {refunded = true}",
            "writes [refunded,keys]; finish {refunded = true}",
        ),
        (
            "writes refunded; finish {refunded = true}",
            "writes [refunded,charged_truth]; finish {refunded = true; charged_truth = false}",
        ),
    ] {
        h::rejection(
            &json!(report::check(&i::edit(&i::source("R04"), old, new))),
            "U-SEM-0",
            "recovery_query_unmodeled",
        );
    }
}
#[test]
fn comparison_keeps_dead_handler_end_inventory_and_children_not_root_wait_tables() {
    let s = i::paired("R01", "R07");
    for v in [
        json!(compare::compare(&s, &["A", "B"], Analysis::Exhaustive)),
        json!(diff::diff(&s, "A", "B")),
    ] {
        assert_ne!(v["verdict"], "unsupported", "{v}");
        assert_ne!(v["verdict"], "invalid");
        let children = if v.get("plans").is_some() {
            h::rows(&v["plans"])
                .iter()
                .map(|p| &p["report"])
                .collect::<Vec<_>>()
        } else {
            vec![&v["from"]["report"], &v["to"]["report"]]
        };
        for (r, id) in children.iter().zip(["R01", "R07"]) {
            h::check(r, id);
            h::waiting(r, id);
        }
        assert!(v.get("waiting").is_none());
        assert!(v["tables"].get("waiting").is_none());
        let refs = v["ledger"]["subjects"].to_string();
        assert!(refs.contains("ledger"));
    }
    let mismatched = i::paired("R01", "R04");
    let v = json!(compare::compare(
        &mismatched,
        &["A", "B"],
        Analysis::Exhaustive
    ));
    h::verdict(&v, "invalid");
    assert!(
        v["mismatch"].is_object(),
        "lexical handler refunded must be included even if unreachable in A"
    );
    let d = json!(diff::diff(&mismatched, "A", "B"));
    assert_ne!(d["verdict"], "invalid");
    assert_ne!(d["verdict"], "unsupported");
}
#[test]
fn agency_partial_response_control_is_not_promoted_by_query_or_resolve() {
    let v = control();
    for word in [
        "response_control",
        "W-AGY-0",
        "W-AGY-2",
        "W-LIN-2",
        "W-RESP-2",
    ] {
        assert!(v.to_string().contains(word), "unchecked {word}");
    }
    assert!(!v["agency"].to_string().contains("resolve effect"));
    let s = i::paired("R01", "R07");
    let v = json!(diff::diff(&s, "A", "B"));
    assert!(v["agency"].to_string().contains("not_comparable"));
    assert!(v["agency"].to_string().contains("response_control"));
}
#[test]
fn opaque_defer_in_dead_handler_call_suppresses_quantities_not_counts_or_core() {
    let s = i::edit(
        &i::source("R01"),
        "cut C { include [Psp,CS,Finance,Shop] }",
        "cut C { include [Psp,CS,Finance,Shop]; defer CS.take_over reason \"not measured\" loses \"effort\" }",
    );
    let s = i::edit(
        &s,
        "} else { stop receipt_unknown }\n } else",
        "} else { unused = CS.take_over(status); stop receipt_unknown }\n } else",
    );
    let v = json!(report::check(&s));
    h::admitted(&v);
    assert_eq!(
        h::row(&v["properties"], "id", "receipt_resolution")["verdict"],
        "satisfied"
    );
    h::count(&v, "received", "1");
    h::count(&v, "completed", "1");
    assert_eq!(v["metrics"]["overall"]["work"]["min"], Value::Null);
    assert_eq!(v["metrics"]["overall"]["work"]["reason"], "not_configured");
    assert_eq!(
        h::row(&v["quality"]["cohort"], "name", "ChargedPerArrival")["value"]["reason"],
        "not_configured"
    );
}
fn suite(points: &[(&str, String)]) -> Value {
    json!(sensitivity::run::run(
        &i::suite(points),
        sensitivity::input::Mode::Exhaustive
    ))
}
#[test]
fn duration_and_loss_p_are_allowed_finite_sensitivity_not_changed_recovery_policy() {
    let a = i::probability(true, "0.1");
    let b = i::edit(
        &a,
        "duration fixed 2min; receipt_query",
        "duration fixed 4min; receipt_query",
    );
    let v = suite(&[("two", a), ("four", b)]);
    assert!(
        h::rows(&v["points"])
            .iter()
            .all(|p| p["status"] == "evaluated"),
        "{}",
        v["diagnostics"]
    );
    let v = suite(&[
        ("zero", i::probability(false, "0")),
        ("tenth", i::probability(false, "0.1")),
        ("fifth", i::probability(false, "0.2")),
    ]);
    for (p, unknown) in h::rows(&v["points"]).iter().zip(["0", "1/100", "1/25"]) {
        assert_eq!(p["status"], "evaluated");
        if unknown != "0" {
            assert_eq!(
                h::row(
                    &p["report"]["case_outcomes"][0]["terminals"],
                    "kind",
                    "receipt_unknown"
                )["probability"],
                json!({"min":unknown,"max":unknown})
            );
        }
    }
    assert_eq!(v["scope"]["continuous_region_checked"], false);
    assert_eq!(v["scope"]["robust_guarantee"], false);
}
#[test]
fn recovery_policy_deadlines_availability_query_count_and_cut_are_same_question_gate() {
    let a = i::source("T02");
    for (old, new) in [
        ("availability available", "availability unavailable"),
        ("by 6min", "by 7min"),
        (
            "status = Psp.query_status(case.id) for command pay",
            "extra = Psp.query_status(case.id) for command pay\nstatus = Psp.query_status(case.id) for command pay",
        ),
        (
            "cut C { include [Psp,CS,Finance,Shop] }",
            "cut C { question \"different\"; include [Psp,CS,Finance,Shop] }",
        ),
    ] {
        let changed = i::edit(&a, old, new);
        let v = suite(&[("base", a.clone()), ("changed", changed)]);
        h::verdict(&v, "invalid");
        assert_eq!(v["points"][1]["reason"], "sensitivity_contract_mismatch");
        assert!(
            h::rows(&v["points"])
                .iter()
                .all(|p| p["status"] != "evaluated")
        );
    }
}
#[test]
fn unsupported_contest_and_quantile_crossings_are_not_unlocked() {
    let q = i::append(
        &i::source("R01"),
        "metric quantile.Close = quantile(0.5, case.Charged)",
    );
    h::rejection(
        &json!(report::check(&q)),
        "U-SEM-0",
        "recovery_feature_combination",
    );
    let s = i::append(
        &i::source("R01"),
        "actor Customer {control party; standing over [end.shipped] basis \"assumed contract\"; capacity 1; capability contest()->Ack {duration fixed 1min;result nondet {recorded}}}\ncontract Contest {contest Customer on end.shipped route Customer.contest reversible_by [f] respond_by 1min}",
    );
    let s = i::edit(
        &s,
        "include [Psp,CS,Finance,Shop]",
        "include [Psp,CS,Finance,Shop,Customer]",
    );
    h::rejection(
        &json!(report::check(&s)),
        "U-SEM-0",
        "recovery_feature_combination",
    );
}
#[test]
fn probability_pair_keeps_d_ud_uu_masses_and_one_arrival() {
    for available in [true, false] {
        let source = i::append(
            &i::probability(available, "0.1"),
            "verify probability reached end.shipped within 3min >= 0.99",
        );
        let v = json!(report::check(&source));
        h::admitted(&v);
        assert_eq!(
            h::row(&v["properties"], "id", "probability reached #2")["probability"],
            json!({"min":"99/100","max":"99/100"}),
            "{}",
            v["diagnostics"]
        );
        let ts = &v["case_outcomes"][0]["terminals"];
        let end = h::row(ts, "end", "shipped");
        let mass = if available { "1" } else { "99/100" };
        assert_eq!(end["probability"], json!({"min":mass,"max":mass}));
        assert_eq!(
            end["elapsed"],
            json!({"min":2,"max":if available {8}else{3}})
        );
        if !available {
            let u = h::row(ts, "kind", "receipt_unknown");
            assert_eq!(u["probability"], json!({"min":"1/100","max":"1/100"}));
            assert_eq!(u["observed_work"], h::point(4));
            assert_eq!(u["work"], Value::Null);
        }
        h::count(&v, "received", "1");
    }
}
#[test]
fn people_occupancy_includes_charge_query_and_refund_on_the_same_psp_actor() {
    for (id, c, f, q) in [
        ("R01", "3", "0", "3"),
        ("R04", "4", "1", "4"),
        ("R05", "5", "1", "4"),
    ] {
        let source = i::edit(
            &i::source(id),
            "capacity 1; control policy; state keys",
            "executors [q]; capacity 1; control policy; state keys",
        );
        let v = json!(report::check(&source));
        h::admitted(&v);
        let people = &v["metrics"]["people"];
        assert_eq!(
            h::row(people, "person", "c")["work"],
            h::scalar(c),
            "{}",
            v["diagnostics"]
        );
        assert_eq!(h::row(people, "person", "q")["work"], h::scalar(q));
        if f != "0" {
            assert_eq!(h::row(people, "person", "f")["work"], h::scalar(f));
        }
        let work = match id {
            "R01" => "7",
            "R04" => "9",
            _ => "10",
        };
        assert_eq!(v["metrics"]["overall"]["work"], h::scalar(work));
        assert_eq!(
            h::row(
                &v["metrics"]["capabilities"],
                "capability",
                "Psp.query_status"
            )["starts"],
            h::scalar("1")
        );
    }
}
#[test]
fn ordinary_call_is_not_query_evidence_and_query_response_is_a_rejected_new_command() {
    let source = i::edit(
        &i::source("R01"),
        "receipt_query charge_keyed key key availability available",
        "result nondet {committed_once}",
    );
    h::rejection(
        &json!(report::check(&source)),
        "E-TYPE-0",
        "recovery_evidence_mismatch",
    );
    let source = i::edit(
        &i::source("R01"),
        "query_status(case.id) for command pay",
        "query_status(case.id) for command pay response_loss none",
    );
    h::rejection(
        &json!(report::check(&source)),
        "U-SEM-0",
        "recovery_feature_combination",
    );
}

#[test]
fn diff_recovery_transport_rejects_swapped_child_ledgers() {
    let v = json!(diff::diff(&i::paired("R01", "R07"), "A", "B"));
    h::check(&v["from"]["report"], "R01");
    h::check(&v["to"]["report"], "R07");
    let r: diff::DiffReport = serde_json::from_value(v.clone()).expect("valid control first");
    assert_eq!(json!(r), v);
    let mut bad = v.clone();
    bad["from"]["report"]["ledger"] = v["to"]["report"]["ledger"].clone();
    bad["to"]["report"]["ledger"] = v["from"]["report"]["ledger"].clone();
    assert!(serde_json::from_value::<diff::DiffReport>(bad).is_err());
}
