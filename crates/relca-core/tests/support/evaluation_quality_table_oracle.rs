//! Hand values, not a projection of either production tables or nested reports.
use serde_json::{Value, json};

pub fn exact(n: &str) -> Value {
    json!({"lower":n,"upper":n,"basis":"exact","reason":null})
}
fn unknown(reason: &str) -> Value {
    json!({"lower":null,"upper":null,"basis":null,"reason":reason})
}
fn undetermined(l: &str, u: &str) -> Value {
    json!({"lower":l,"upper":u,"basis":"per_case_relaxation","reason":"undetermined"})
}
fn sample() -> Value {
    json!({"mean":null,"min":null,"max":null,"samples":0,"reason":"aborted"})
}
fn row(
    plans: &[&str],
    scope: &str,
    subject: &str,
    end: Option<&str>,
    field: &str,
    value: Value,
) -> Value {
    json!({"key":{"scope":scope,"subject":subject,"end":end,"field":field},
        "values":plans.iter().map(|p|json!({"configuration":p,"status":"reported","value":value})).collect::<Vec<_>>()})
}
fn property(rows: &mut Vec<Value>, plans: &[&str], name: &str, value: Value, sampled: bool) {
    rows.push(row(plans, "property", name, None, "expected_count", value));
    if sampled {
        rows.push(row(
            plans,
            "property",
            name,
            None,
            "unknown_count",
            sample(),
        ));
    }
}
fn cohort(
    rows: &mut Vec<Value>,
    plans: &[&str],
    name: &str,
    d: Value,
    value: Value,
    bounds: Value,
) {
    for (field, v) in [("value", value), ("bounds_with_undetermined", bounds)] {
        let mut r = row(plans, "cohort", name, None, field, v);
        r["denominator"] = d.clone();
        rows.push(r);
    }
}
fn outcome(
    rows: &mut Vec<Value>,
    plans: &[&str],
    kind: &str,
    end: Option<&str>,
    count: Value,
    rate: Value,
) {
    rows.push(row(plans, "outcome", kind, end, "expected_count", count));
    rows.push(row(plans, "outcome", kind, end, "rate", rate));
}
fn arrivals(n: u64) -> Value {
    json!({"kind":"arrivals","count":n})
}
fn registered() -> Value {
    json!({"kind":"property","property":"Registered","covers":null,"reason":"state_condition"})
}

pub fn record() -> Value {
    let p = &["Left", "Right"];
    let mut rows = Vec::new();
    for (name, n) in [
        ("Registered", "23/25"),
        ("Wrong", "1/50"),
        ("Pass", "23/25"),
    ] {
        property(&mut rows, p, name, exact(n), false);
    }
    for (name, n, d) in [
        ("WrongRate", "1/50", arrivals(1)),
        ("Conditional", "1/46", registered()),
        ("PassRate", "23/25", arrivals(1)),
    ] {
        cohort(&mut rows, p, name, d, exact(n), Value::Null);
    }
    for (kind, end, n) in [
        ("end", Some("posted"), "23/25"),
        ("end", Some("returned"), "2/25"),
        ("rejected", None, "0"),
        ("unfinished", None, "0"),
        ("deadlock", None, "0"),
    ] {
        outcome(&mut rows, p, kind, end, exact(n), exact(n));
    }
    json!(rows)
}
pub fn rule(sampled: bool) -> Value {
    let sample_n = |n: &str| json!({"mean":n,"min":n,"max":n,"samples":4,"reason":null});
    let quantity = |n: &str| if sampled { sample_n(n) } else { exact(n) };
    let mut per_plan = Vec::new();
    for (plan, n) in [("AsIs", "1"), ("ToBeB", "0")] {
        let p = &[plan];
        let mut rows = Vec::new();
        rows.push(row(
            p,
            "property",
            "Hit",
            None,
            "expected_count",
            quantity(n),
        ));
        if sampled {
            rows.push(row(
                p,
                "property",
                "Hit",
                None,
                "unknown_count",
                sample_n("0"),
            ));
        }
        let ratio = if sampled {
            json!({"point":n,"numerator":sample_n(n),"denominator":sample_n("1"),"samples":4,"reason":null})
        } else {
            exact(n)
        };
        cohort(&mut rows, p, "Rate", arrivals(1), ratio, Value::Null);
        for (kind, end, n) in [
            ("end", Some("done"), "1"),
            ("rejected", None, "0"),
            ("unfinished", None, "0"),
            ("deadlock", None, "0"),
        ] {
            let rate = if sampled {
                json!({"point":n,"samples":4,"reason":null})
            } else {
                exact(n)
            };
            outcome(&mut rows, p, kind, end, quantity(n), rate);
        }
        per_plan.push(rows);
    }
    let mut rows = per_plan.remove(0);
    for (left, right) in rows.iter_mut().zip(per_plan.remove(0)) {
        left["values"]
            .as_array_mut()
            .unwrap()
            .extend(right["values"].as_array().unwrap().iter().cloned());
    }
    json!(rows)
}
pub fn held() -> Value {
    let p = &["Left", "Right"];
    let mut rows = Vec::new();
    property(&mut rows, p, "Hit", undetermined("0", "3"), false);
    cohort(
        &mut rows,
        p,
        "Rate",
        arrivals(3),
        unknown("undetermined"),
        json!({"lower":"0","upper":"1"}),
    );
    for (kind, end, n, rate) in [
        ("end", Some("done"), "0", "0"),
        ("rejected", None, "0", "0"),
        ("unfinished", None, "0", "0"),
        ("deadlock", None, "0", "0"),
        ("cut_off", None, "3", "1"),
    ] {
        outcome(&mut rows, p, kind, end, exact(n), exact(rate));
    }
    json!(rows)
}
pub fn overflow() -> Value {
    let p = &["Left", "Right"];
    let mut rows = Vec::new();
    for name in ["Wrong", "Registered"] {
        property(&mut rows, p, name, undetermined("0", "1"), false);
    }
    property(&mut rows, p, "Spill", undetermined("1", "2"), false);
    cohort(
        &mut rows,
        p,
        "WrongPerArrival",
        arrivals(2),
        unknown("undetermined"),
        json!({"lower":"0","upper":"1/2"}),
    );
    cohort(
        &mut rows,
        p,
        "WrongPerRegistered",
        registered(),
        unknown("undetermined"),
        Value::Null,
    );
    cohort(
        &mut rows,
        p,
        "SpillPerArrival",
        arrivals(2),
        unknown("undetermined"),
        json!({"lower":"1/2","upper":"1"}),
    );
    for (kind, end, n, rate) in [
        ("end", Some("returned"), "0", "0"),
        ("end", Some("posted"), "0", "0"),
        ("overflow", Some("spill"), "1", "1/2"),
        ("rejected", None, "0", "0"),
        ("unfinished", None, "0", "0"),
        ("deadlock", None, "0", "0"),
        ("cut_off", None, "1", "1/2"),
    ] {
        outcome(&mut rows, p, kind, end, exact(n), exact(rate));
    }
    json!(rows)
}

pub fn horizon14() -> Value {
    let p = &["Check", "Fast"];
    let mut rows = Vec::new();
    property(&mut rows, p, "Wrong", undetermined("1/25", "26/25"), false);
    property(
        &mut rows,
        p,
        "Registered",
        undetermined("46/25", "71/25"),
        false,
    );
    cohort(
        &mut rows,
        p,
        "WrongPerArrival",
        arrivals(3),
        unknown("undetermined"),
        json!({"lower":"1/75","upper":"26/75"}),
    );
    cohort(
        &mut rows,
        p,
        "WrongPerRegistered",
        registered(),
        unknown("undetermined"),
        json!({"lower":"1/71","upper":"13/23"}),
    );
    for (kind, end, n, rate) in [
        ("end", Some("returned"), "4/25", "4/75"),
        ("end", Some("posted"), "46/25", "46/75"),
        ("rejected", None, "0", "0"),
        ("unfinished", None, "0", "0"),
        ("deadlock", None, "0", "0"),
        ("cut_off", None, "1", "1/3"),
    ] {
        outcome(&mut rows, p, kind, end, exact(n), exact(rate));
    }
    json!(rows)
}
