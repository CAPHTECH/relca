//! ADR0052 hand arithmetic: rule_quality_calculations.md, never captured output.
#![allow(dead_code)]
#[path = "rule_integration_inputs.rs"]
pub mod old;
use serde_json::{Value, json};

pub fn source() -> String {
    include_str!("rule_quality.relca").into()
}
pub fn revocable_source() -> String {
    old::replace(
        &source(),
        "authority G { basis \"prior\"; grants p to change ApprovalRule }",
        "authority G { basis \"prior\"; grants p to change ApprovalRule; revocable_by p }",
    )
}
pub fn assert_revocable_report(v: &Value) {
    assert_eq!(v["verdict"], "satisfied");
    assert_report(v, false, false);
    assert!(
        v["unchecked"]
            .as_array()
            .unwrap()
            .contains(&json!("dynamic_authority_revocation"))
    );
}
pub fn invoice_rule() -> String {
    let s = old::append(
        include_str!("../../../../examples/invoice/phase1.relca"),
        "actor ApprovalRule { version \"r2026-04\"; decides approval_required: bool = true }",
    );
    let s = old::replace(
        &s,
        "include [Invoice,PurchaseOrder,ReceiptRecord,Clerk,Extractor,Approver,Ledger,Vendor]",
        "include [Invoice,PurchaseOrder,ReceiptRecord,Clerk,Extractor,Approver,Ledger,Vendor,ApprovalRule]",
    );
    without_defer(&old::replace(
        &s,
        "requires approval == pass && po == pass && receipt == pass else held",
        "requires (ApprovalRule.approval_required == false || approval == pass) && po == pass && receipt == pass else held",
    ))
}
/// ADR0053: keep the ADR0052 arithmetic as a declared no-defer CONTROL. The
/// actual invoice sources are separately required to withdraw under their Cut.
pub fn without_defer(src: &str) -> String {
    src.lines()
        .filter(|l| !l.trim_start().starts_with("defer "))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
pub fn exact(n: &str) -> Value {
    json!({"lower":n,"upper":n,"basis":"exact","reason":null})
}
pub fn approval_source(pass: bool) -> String {
    let s = old::replace(
        &old::fixture("rule-basic"),
        "result nondet { pass, fail }",
        if pass {
            "result nondet { pass }"
        } else {
            "result nondet { fail }"
        },
    );
    old::append(
        &s,
        "property case.Posted = reached(end.posted)\nmetric cohort.Rate = expected(count(case.Posted)) / arrivals\nmetric quantile.All = quantile(0.5, case.Posted)\nmetric quantile.Given = quantile(0.5, case.Posted given reached)\nsimulation replications 3 seed 7 confidence 0.5",
    )
}
pub fn assert_approval(q: &Value, posted: bool, sim: bool) {
    assert_quality(q, posted, sim, 3);
    for (j, r) in q["quantiles"].as_array().unwrap().iter().enumerate() {
        let expected = if posted {
            if sim {
                json!({"point":"3","observations":3,"censored":0,"reason":null})
            } else {
                exact("3")
            }
        } else {
            absent(
                if j == 0 {
                    "beyond_reached_mass"
                } else {
                    "zero_denominator"
                },
                sim,
            )
        };
        assert_eq!(r["value"], expected);
        assert_eq!(r["lower_bound"], Value::Null);
    }
}
pub fn absent(reason: &str, sim: bool) -> Value {
    if sim {
        json!({"point":null,"observations":3,"censored":0,"reason":reason})
    } else {
        json!({"lower":null,"upper":null,"basis":null,"reason":reason})
    }
}
pub fn assert_quality(q: &Value, enabled: bool, sim: bool, repetitions: u64) {
    assert!(q.is_object(), "quality must be present");
    assert_eq!(q["rules"], "quality-cohort-p1-1");
    assert_eq!(q["analysis"], if sim { "simulation" } else { "exhaustive" });
    let n = if enabled { "1" } else { "0" };
    let value = &q["cohort"][0]["value"];
    assert_eq!(value["reason"], Value::Null);
    if sim {
        assert_eq!(value["point"], n);
        assert_eq!(
            q["properties"][0]["cases"][0]["hits"],
            if enabled { repetitions } else { 0 }
        );
        assert_eq!(q["properties"][0]["cases"][0]["unknown"], 0);
        assert_eq!(q["properties"][0]["cases"][0]["trials"], repetitions);
    } else {
        assert_eq!(*value, exact(n));
        assert_eq!(q["properties"][0]["expected_count"], exact(n));
    }
    assert!(!q["checked"].as_array().unwrap().is_empty());
}
pub fn assert_report(v: &Value, enabled: bool, sim: bool) {
    assert_quality(&v["quality"], enabled, sim, 3);
    assert!(
        !v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["field"] == "quality.rule")
    );
    let qs = v["quality"]["quantiles"].as_array().unwrap();
    assert_eq!(qs.len(), 2);
    for (j, row) in qs.iter().enumerate() {
        assert_eq!(row["lower_bound"], Value::Null);
        let want = if enabled {
            if sim {
                json!({"point":"0","observations":3,"censored":0,"reason":null})
            } else {
                exact("0")
            }
        } else {
            absent(
                if j == 0 {
                    "beyond_reached_mass"
                } else {
                    "zero_denominator"
                },
                sim,
            )
        };
        assert_eq!(row["value"], want);
    }
}
pub fn assert_compare(v: &Value, sim: bool, tight: bool) {
    assert_eq!(
        v["verdict"],
        if sim && !tight {
            "inconclusive"
        } else {
            "violated"
        }
    );
    for (j, enabled) in [true, false].iter().enumerate() {
        assert_report(&v["plans"][j]["report"], *enabled, sim);
        let cell = &v["evaluation"]["hard"][0]["values"][j];
        let verdict = if sim {
            if tight {
                if *enabled { "violated" } else { "inconclusive" }
            } else if *enabled {
                "inconclusive"
            } else {
                "satisfied"
            }
        } else if *enabled {
            "violated"
        } else {
            "satisfied"
        };
        assert_eq!(cell["verdict"], verdict);
        assert_eq!(cell["reason"], Value::Null);
        assert_eq!(
            cell["interval"],
            if sim {
                if *enabled {
                    json!({"lower":"1/2","upper":"1"})
                } else {
                    json!({"lower":"0","upper":"1/2"})
                }
            } else {
                json!({"lower":if *enabled {"1"}else{"0"},"upper":if *enabled {"1"}else{"0"}})
            }
        );
        assert_eq!(
            v["evaluation"]["objectives"][0]["values"][j][if sim { "point" } else { "lower" }],
            if *enabled { "1" } else { "0" }
        );
    }
    if sim {
        assert_multiplicity(v);
        assert_eq!(v["evaluation"]["pareto"]["reason"], "simulation_estimates");
    }
}

pub fn multiplicity(v: &Value) -> Result<(u64, &str), &'static str> {
    let m = v
        .pointer("/evaluation/statistics/multiplicity/m")
        .and_then(Value::as_u64)
        .ok_or("missing or invalid multiplicity.m")?;
    let alpha = v
        .pointer("/evaluation/statistics/multiplicity/alpha")
        .and_then(Value::as_str)
        .ok_or("missing or invalid multiplicity.alpha")?;
    Ok((m, alpha))
}

pub fn assert_multiplicity(v: &Value) {
    assert_eq!(multiplicity(v).expect("multiplicity schema"), (2, "1/8"));
}

pub fn assert_constant_quality(rule: &Value, constant: &Value) {
    let mut constant = constant.clone();
    for property in constant["properties"].as_array_mut().unwrap() {
        let condition = property["condition"].as_str().unwrap();
        property["condition"] = json!(condition.replace("rule_enabled", "ApprovalRule.enabled"));
    }
    assert_eq!(
        serde_json::to_vec(rule).unwrap(),
        serde_json::to_vec(&constant).unwrap()
    );
}

pub fn constant_control(src: &str, enabled: bool) -> String {
    let s = old::replace(
        src,
        "actor ApprovalRule { version \"r1\"; decides enabled: bool = true }",
        "",
    );
    let s = old::replace(
        &s,
        "observed case { hit: bool = false }",
        if enabled {
            "observed case { hit: bool = false; rule_enabled: bool = true }"
        } else {
            "observed case { hit: bool = false; rule_enabled: bool = false }"
        },
    );
    let s = old::replace(&s, "include [ApprovalRule,A]", "include [A]");
    let s = old::replace(
        &s,
        "; amend ApprovalRule from \"r1\" to \"r2\" by p under G { enabled = false }",
        "",
    );
    let s = old::replace(
        &s,
        "authority G { basis \"prior\"; grants p to change ApprovalRule }",
        "",
    );
    s.replace("ApprovalRule.enabled", "rule_enabled")
}
