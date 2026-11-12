//! ADR 0019 public-entry acceptance. All numbers come from intervals in calc.md,
//! not from a production collector. This file is also included by CLI preflight.
use relca_core::{compare, diff, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
use std::{fs, path::PathBuf};

pub const FIXTURES: [&str; 24] = [
    "met-cancel",
    "met-cancel.broken-drain",
    "met-compare",
    "met-compare.broken-longer-task",
    "met-diff",
    "met-diff.broken-free-start",
    "met-exclusions",
    "met-exclusions.broken-larger-queue",
    "met-fifo",
    "met-fifo.broken-shorter-second-step",
    "met-legacy",
    "met-legacy.broken-enabled",
    "met-missing-rate",
    "met-missing-rate.broken-free-rate",
    "met-parallel",
    "met-parallel.broken-one-slot",
    "met-person",
    "met-person.broken-shared-person",
    "met-positive-support",
    "met-positive-support.broken-positive-early-time",
    "met-sample",
    "met-sample.broken-short-clock",
    "met-serial",
    "met-serial.broken-longer-task",
];
pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}
pub fn source(name: &str) -> String {
    fs::read_to_string(root().join(name).join("model.relca")).unwrap()
}
pub fn strip_metrics(s: &str) -> String {
    s.lines()
        .filter(|l| !l.trim_start().starts_with("metrics {"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}
pub fn add(s: &str, clause: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!("{}\n{clause}\n{}", &s[..i], &s[i..])
}
pub fn run(s: &str, selection: Option<&str>, sim: bool) -> Value {
    match (selection, sim) {
        (Some(c), true) => serde_json::to_value(report::simulate_configuration(s, c)),
        (None, true) => serde_json::to_value(report::simulate(s)),
        (Some(c), false) => serde_json::to_value(report::check_configuration(s, c)),
        (None, false) => serde_json::to_value(report::check(s)),
    }
    .unwrap()
}
pub fn scalar(lo: &str, hi: &str) -> Value {
    json!({"min":lo,"max":hi,"reason":null})
}
pub fn sample(mean: &str, lo: &str, hi: &str, n: u64) -> Value {
    json!({"mean":mean,"min":lo,"max":hi,"samples":n,"reason":null})
}
pub fn absent(reason: &str, sim: bool) -> Value {
    if sim {
        json!({"mean":null,"min":null,"max":null,"samples":0,"reason":reason})
    } else {
        json!({"min":null,"max":null,"reason":reason})
    }
}
pub fn row<'a>(v: &'a Value, key: &str, id: &str) -> &'a Value {
    let rows = v.as_array().expect("required metric inventory");
    let found: Vec<_> = rows.iter().filter(|v| v[key] == id).collect();
    assert_eq!(found.len(), 1, "unique {key}={id}: {rows:?}");
    found[0]
}
pub fn eq(v: &Value, field: &str, lo: &str, hi: &str) {
    assert_eq!(v[field], scalar(lo, hi), "{field}: {v}");
}
fn constant(v: &Value, field: &str, x: &str, sim: bool) {
    assert_eq!(
        v[field],
        if sim {
            sample(x, x, x, 4)
        } else {
            scalar(x, x)
        },
        "{field}"
    );
}
fn count(v: &Value, x: u64, sim: bool) {
    let xstr = x.to_string();
    assert_eq!(
        v,
        &json!({"stats":if sim {sample(&xstr,&xstr,&xstr,4)}else{scalar(&xstr,&xstr)}, "observed_total":if sim {json!(4*x)}else{Value::Null}})
    );
}
pub fn case(v: &Value, id: u64) -> &Value {
    let cs = v["cases"]
        .as_array()
        .expect("all cases, not only completed");
    assert_eq!(cs[id as usize - 1]["case_id"], id);
    &cs[id as usize - 1]
}
pub fn terminal(c: &Value) -> &Value {
    row(&c["terminals"], "end", "done")
}
pub fn hand(name: &str, selection: Option<&str>, m: &Value) {
    if name == "met-legacy" {
        assert!(m.is_null());
        return;
    }
    assert!(m.is_object(), "{name}: metrics must be present");
    let sim = name.starts_with("met-sample") || name.starts_with("met-exclusions");
    let broken = name.contains(".broken-");
    assert_eq!(m["rules"], "business-metrics-p1-1");
    assert_eq!(m["currency"], "JPY");
    assert_eq!(
        m["window"],
        if name.starts_with("met-fifo") { 20 } else { 10 }
    );
    assert_eq!(m["time_basis"], "arrival_to_declared_end");
    assert_eq!(m["lead_time_scope"], "terminal_cases_only");
    assert_eq!(
        m["observation_scope"],
        "until_execution_stop_no_extra_drain"
    );
    assert_eq!(
        m["statistical_unit"],
        if sim {
            json!("replication")
        } else {
            Value::Null
        }
    );
    assert_eq!(
        m["estimate_note"],
        if sim {
            json!("点推定であり、全スケジュールの検証でも区間推定でもない")
        } else {
            Value::Null
        }
    );
    assert_eq!(
        m["status"],
        if name.starts_with("met-exclusions") || name == "met-sample.broken-short-clock" {
            "truncated"
        } else {
            "complete"
        }
    );
    assert_eq!(
        m["overall"]["business_value"],
        json!({"value":null,"reason":"not_configured"})
    );
    let o = &m["overall"];
    let n = if name.starts_with("met-fifo") {
        3
    } else if name.starts_with("met-exclusions") {
        4
    } else {
        1
    };
    assert_eq!(m["cases"].as_array().unwrap().len(), n);
    let (actor_ids, capability_ids, bindings): (&[&str], &[&str], &[&str]) =
        if name.starts_with("met-fifo") {
            (&["A", "B"], &["A.prep", "B.finish"], &["a", "b"])
        } else if name.starts_with("met-person") {
            (&["A", "B"], &["A.a", "B.b"], &["a", "b"])
        } else if name.starts_with("met-cancel") {
            (&["F", "S"], &["F.win", "S.work"], &["fast", "slow"])
        } else if name.starts_with("met-positive-support") {
            (&["A"], &["A.job"], &["job"])
        } else if sim {
            (&["A"], &["A.run"], &["job"])
        } else {
            (&["Office"], &["Office.short", "Office.long"], &["a", "b"])
        };
    for (field, key, ids) in [
        ("actors", "actor", actor_ids),
        ("capabilities", "capability", capability_ids),
    ] {
        let actual: Vec<_> = m[field]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r[key].as_str().unwrap())
            .collect();
        assert_eq!(actual, ids, "{name}: complete {field} inventory");
    }
    for c in m["cases"].as_array().unwrap() {
        let actual: Vec<_> = c["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["binding"].as_str().unwrap())
            .collect();
        assert_eq!(
            actual, bindings,
            "{name}: complete call-site inventory, including unstarted/rejected cases"
        );
        let mut ends: Vec<_> = c["terminals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| (t["kind"].as_str().unwrap(), t["end"].as_str()))
            .collect();
        ends.sort();
        let mut wanted = vec![("end", Some("done")), ("rejected", None)];
        if name.starts_with("met-cancel") {
            wanted.push(("end", Some("none_end")));
        }
        wanted.sort();
        assert_eq!(ends, wanted);
    }
    assert_eq!(
        m["people"].as_array().unwrap().len(),
        if name.starts_with("met-person") { 2 } else { 0 }
    );
    constant(
        o,
        "named_people_work",
        if name.starts_with("met-person") {
            "8"
        } else {
            "0"
        },
        sim,
    );
    count(&o["counts"]["offered"], n as u64, sim);
    count(&o["counts"]["deadlocked"], 0, sim);
    count(&o["counts"]["not_arrived"], 0, sim);
    if !sim {
        count(&o["counts"]["completed"], n as u64, false);
        count(&o["counts"]["received"], n as u64, false);
        count(&o["counts"]["unfinished"], 0, false);
        count(&o["counts"]["rejected"], 0, false);
    }
    if name.starts_with("met-sample") {
        let work = if broken {
            sample("2", "2", "2", 4)
        } else {
            sample("3", "2", "4", 4)
        };
        let cost = if broken {
            sample("17", "17", "17", 4)
        } else {
            sample("24", "17", "31", 4)
        };
        assert_eq!(o["work"], work);
        assert_eq!(o["observed_cost_total"], cost);
        assert_eq!(o["observed_cost_per_arrival"], cost);
        assert_eq!(
            terminal(case(m, 1))["lead_time"],
            if broken {
                sample("2", "2", "2", 2)
            } else {
                sample("3", "2", "4", 4)
            }
        );
        assert_eq!(
            terminal(case(m, 1))["cost_per_case"],
            if broken {
                sample("17", "17", "17", 2)
            } else {
                cost
            }
        );
        let a = row(&m["actors"], "actor", "A");
        assert_eq!(a["work"], work);
        assert_eq!(
            a["utilization"],
            if broken {
                sample("1/5", "1/5", "1/5", 4)
            } else {
                sample("3/10", "1/5", "2/5", 4)
            }
        );
        count(&o["counts"]["received"], 1, true);
        count(&o["counts"]["rejected"], 0, true);
        if broken {
            for k in ["completed", "unfinished"] {
                assert_eq!(
                    o["counts"][k],
                    json!({"stats":sample("1/2","0","1",4),"observed_total":2})
                );
            }
        } else {
            count(&o["counts"]["completed"], 1, true);
            count(&o["counts"]["unfinished"], 0, true);
        }
        constant(o, "peak_tasks", "1", true);
        constant(o, "start_cost", "3", true);
        constant(case(m, 1), "first_start_wait", "0", true);
        constant(
            row(&case(m, 1)["tasks"], "binding", "job"),
            "wait",
            "0",
            true,
        );
        return;
    }
    if name.starts_with("met-exclusions") {
        constant(o, "work", "3", true);
        constant(o, "peak_tasks", "1", true);
        constant(o, "observed_until", "3", true);
        constant(o, "observed_cost_total", "16", true);
        constant(o, "observed_cost_per_arrival", "4", true);
        constant(row(&m["actors"], "actor", "A"), "utilization", "3/10", true);
        constant(
            row(&m["capabilities"], "capability", "A.run"),
            "starts",
            "2",
            true,
        );
        for (k, x) in [
            ("received", if broken { 3 } else { 2 }),
            ("completed", 1),
            ("rejected", if broken { 1 } else { 2 }),
            ("unfinished", if broken { 2 } else { 1 }),
        ] {
            count(&o["counts"][k], x, true);
        }
        constant(terminal(case(m, 1)), "lead_time", "3", true);
        constant(terminal(case(m, 1)), "cost_per_case", "11", true);
        constant(case(m, 1), "first_start_wait", "0", true);
        let started = if broken { 2 } else { 3 };
        constant(
            case(m, started),
            "first_start_wait",
            if broken { "3" } else { "2" },
            true,
        );
        assert_eq!(
            terminal(case(m, started))["lead_time"],
            absent("no_samples", true)
        );
        let rejected = if broken { vec![4] } else { vec![2, 4] };
        for id in rejected {
            let c = case(m, id);
            let r = row(&c["terminals"], "kind", "rejected");
            assert_eq!(r["lead_time"], absent("rejected", true));
            constant(c, "observed_cost_total", "0", true);
        }
        if broken {
            assert_eq!(case(m, 3)["first_start_wait"], absent("unfinished", true));
        }
        return;
    }
    let (lead_lo, lead_hi, work_lo, work_hi, cost_lo, cost_hi, peak) = if name
        .starts_with("met-fifo")
    {
        (
            "8",
            "8",
            if broken { "21" } else { "24" },
            if broken { "21" } else { "24" },
            if broken { "69" } else { "78" },
            if broken { "69" } else { "78" },
            "2",
        )
    } else if name.starts_with("met-cancel") {
        if broken {
            ("7", "7", "10", "10", "39", "39", "2")
        } else {
            ("4", "5", "7", "8", "30", "33", "2")
        }
    } else if name.starts_with("met-positive-support") {
        (
            if broken { "1" } else { "2" },
            "4",
            if broken { "1" } else { "2" },
            "4",
            if broken { "10" } else { "17" },
            "31",
            "1",
        )
    } else if name.starts_with("met-person") {
        (
            if broken { "8" } else { "5" },
            if broken { "8" } else { "5" },
            "8",
            "8",
            "355",
            "355",
            if broken { "1" } else { "2" },
        )
    } else {
        let long = if name.ends_with("broken-longer-task") {
            6
        } else {
            5
        };
        let parallel = selection == Some("ToBeP") || name.starts_with("met-parallel") && !broken;
        let lead = match (parallel, long) {
            (true, 5) => "5",
            (true, 6) => "6",
            (false, 5) => "8",
            _ => "9",
        };
        let work = if long == 6 { "9" } else { "8" };
        let cost = if name.ends_with("broken-free-rate") || name.ends_with("broken-free-start") {
            "802"
        } else if long == 6 {
            "905"
        } else {
            "805"
        };
        (
            lead,
            lead,
            work,
            work,
            cost,
            cost,
            if parallel { "2" } else { "1" },
        )
    };
    eq(o, "work", work_lo, work_hi);
    eq(o, "peak_tasks", peak, peak);
    for capability in m["capabilities"].as_array().unwrap() {
        eq(
            capability,
            "starts",
            if n == 3 { "3" } else { "1" },
            if n == 3 { "3" } else { "1" },
        );
    }
    if name == "met-missing-rate" {
        for f in [
            "start_cost",
            "observed_cost_total",
            "observed_cost_per_arrival",
        ] {
            assert_eq!(o[f], absent("not_configured", false));
        }
        assert_eq!(
            terminal(case(m, 1))["cost_per_case"],
            absent("not_configured", false)
        );
        let c = row(&m["capabilities"], "capability", "Office.long");
        eq(c, "starts", "1", "1");
        assert_eq!(c["start_cost"], absent("not_configured", false));
    } else {
        eq(o, "observed_cost_total", cost_lo, cost_hi);
        eq(
            o,
            "observed_cost_per_arrival",
            if n == 3 {
                if broken { "23" } else { "26" }
            } else {
                cost_lo
            },
            if n == 3 {
                if broken { "23" } else { "26" }
            } else {
                cost_hi
            },
        );
        let per_lo = if n == 3 {
            if broken { "23" } else { "26" }
        } else {
            cost_lo
        };
        let per_hi = if n == 3 {
            if broken { "23" } else { "26" }
        } else {
            cost_hi
        };
        eq(
            row(&m["terminals"], "end", "done"),
            "cost_per_case",
            per_lo,
            per_hi,
        );
        for c in m["cases"].as_array().unwrap() {
            eq(terminal(c), "cost_per_case", per_lo, per_hi);
        }
    }
    if name.starts_with("met-fifo") {
        for id in 1..=3 {
            let i = id as usize - 1;
            let c = case(m, id);
            assert_eq!(c["arrival"], id - 1);
            let leads = if broken {
                ["7", "10", "13"]
            } else {
                ["8", "12", "16"]
            };
            eq(terminal(c), "lead_time", leads[i], leads[i]);
            let first = ["0", "2", "4"][i];
            eq(c, "first_start_wait", first, first);
            eq(row(&c["tasks"], "binding", "a"), "wait", first, first);
            let second = if broken { ["0", "1", "2"][i] } else { first };
            eq(row(&c["tasks"], "binding", "b"), "wait", second, second);
            eq(
                c,
                "work",
                if broken { "7" } else { "8" },
                if broken { "7" } else { "8" },
            );
        }
        eq(row(&m["actors"], "actor", "A"), "work", "9", "9");
        eq(
            row(&m["actors"], "actor", "A"),
            "utilization",
            "9/20",
            "9/20",
        );
        eq(
            row(&m["actors"], "actor", "B"),
            "work",
            if broken { "12" } else { "15" },
            if broken { "12" } else { "15" },
        );
        eq(
            row(&m["actors"], "actor", "B"),
            "utilization",
            if broken { "3/5" } else { "3/4" },
            if broken { "3/5" } else { "3/4" },
        );
    } else {
        let c = case(m, 1);
        eq(terminal(c), "lead_time", lead_lo, lead_hi);
        eq(c, "first_start_wait", "0", "0");
        for t in c["tasks"].as_array().unwrap() {
            let (lo, hi) = if name == "met-parallel.broken-one-slot"
                || name == "met-person.broken-shared-person"
            {
                ("0", if t["binding"] == "a" { "5" } else { "3" })
            } else {
                ("0", "0")
            };
            eq(t, "wait", lo, hi);
            eq(t, "starts", "1", "1");
        }
        if name.starts_with("met-person") {
            let p = row(&m["people"], "person", "p");
            let q = row(&m["people"], "person", "q");
            eq(
                p,
                "work",
                if broken { "8" } else { "3" },
                if broken { "8" } else { "3" },
            );
            eq(
                row(&m["actors"], "actor", "A"),
                "utilization",
                "3/10",
                "3/10",
            );
            eq(row(&m["actors"], "actor", "B"), "utilization", "1/2", "1/2");
            eq(
                q,
                "work",
                if broken { "0" } else { "5" },
                if broken { "0" } else { "5" },
            );
            eq(p, "peak_tasks", "1", "1");
            eq(
                q,
                "peak_tasks",
                if broken { "0" } else { "1" },
                if broken { "0" } else { "1" },
            );
        } else if name.starts_with("met-cancel") {
            let slow = row(&m["actors"], "actor", "S");
            eq(
                slow,
                "work",
                if broken { "7" } else { "4" },
                if broken { "7" } else { "5" },
            );
            eq(
                slow,
                "utilization",
                if broken { "7/10" } else { "2/5" },
                if broken { "7/10" } else { "1/2" },
            );
        } else if name.starts_with("met-positive-support") {
            eq(
                row(&m["actors"], "actor", "A"),
                "utilization",
                if broken { "1/10" } else { "1/5" },
                "2/5",
            );
        } else {
            let a = row(&m["actors"], "actor", "Office");
            let u = if name == "met-parallel.broken-one-slot" {
                "4/5"
            } else if work_lo == "9" {
                "9/20"
            } else {
                "2/5"
            };
            eq(a, "utilization", u, u);
        }
    }
}

#[test]
fn m_population_24_dirs_28_selected_24_exhaustive_4_simulation() {
    let mut names: Vec<_> = fs::read_dir(root())
        .unwrap()
        .map(|e| e.unwrap())
        .filter(|e| {
            e.file_type().unwrap().is_dir() && e.file_name().to_string_lossy().starts_with("met-")
        })
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(names, FIXTURES);
    let (mut selected, mut exhaustive, mut simulation) = (0, 0, 0);
    for name in FIXTURES {
        let s = source(name);
        let sim = name.starts_with("met-sample") || name.starts_with("met-exclusions");
        let choices: &[Option<&str>] =
            if name.starts_with("met-compare") || name.starts_with("met-diff") {
                &[Some("AsIs"), Some("ToBeP")]
            } else {
                &[None]
            };
        for &cfg in choices {
            selected += 1;
            if sim {
                simulation += 1;
            } else {
                exhaustive += 1;
            }
            let v = run(&s, cfg, sim);
            assert_eq!(
                v["verdict"],
                if sim { "inconclusive" } else { "satisfied" },
                "{name}/{cfg:?}: {v}"
            );
            hand(name, cfg, &v["metrics"]);
        }
    }
    assert_eq!((selected, exhaustive, simulation), (28, 24, 4));
}

pub struct Input {
    pub label: String,
    pub source: String,
    pub skeleton: String,
    pub code: Option<&'static str>,
}
pub fn static_inputs() -> Vec<Input> {
    let base = strip_metrics(&source("met-serial"));
    let fragments = [
        (
            "unknown-actor",
            "window 10min; currency \"JPY\"; rate Unknown per_minute 1",
            "E-NAME-0",
        ),
        (
            "unknown-capability",
            "window 10min; currency \"JPY\"; rate Office.unknown per_start 1",
            "E-NAME-0",
        ),
        (
            "unused-unknown",
            "window 10min; currency \"JPY\"; rate Unknown.unknown per_start 1",
            "E-NAME-0",
        ),
        (
            "duplicate-window",
            "window 10min; window 10min; currency \"JPY\"",
            "U-SEM-0",
        ),
        (
            "duplicate-currency",
            "window 10min; currency \"JPY\"; currency \"JPY\"",
            "U-SEM-0",
        ),
        (
            "duplicate-actor-rate",
            "window 10min; currency \"JPY\"; rate Office per_minute 1; rate Office per_minute 1",
            "U-SEM-0",
        ),
        (
            "duplicate-start-rate",
            "window 10min; currency \"JPY\"; rate Office.long per_start 1; rate Office.long per_start 1",
            "U-SEM-0",
        ),
        (
            "negative-one",
            "window 10min; currency \"JPY\"; rate Office per_minute -1",
            "E-SYN-0",
        ),
        (
            "negative-zero",
            "window 10min; currency \"JPY\"; rate Office per_minute -0",
            "E-SYN-0",
        ),
        (
            "negative-window",
            "window -1min; currency \"JPY\"",
            "E-SYN-0",
        ),
        ("no-window", "currency \"JPY\"", "E-SYN-0"),
        ("no-currency", "window 10min", "E-SYN-0"),
        ("empty", "", "E-SYN-0"),
        ("zero-window", "window 0min; currency \"JPY\"", "E-CON-0"),
        ("short-window", "window 9min; currency \"JPY\"", "E-CON-0"),
        ("empty-currency", "window 10min; currency \"\"", "E-CON-0"),
        (
            "blank-currency",
            "window 10min; currency \"   \"",
            "E-CON-0",
        ),
        ("hours", "window 10hour; currency \"JPY\"", "U-SYN-0"),
        ("days", "window 10day; currency \"JPY\"", "U-SYN-0"),
        ("seconds", "window 10s; currency \"JPY\"", "U-SYN-0"),
        (
            "fractional-min",
            "window 1.5min; currency \"JPY\"",
            "U-SYN-0",
        ),
        ("no-unit", "window 10; currency \"JPY\"", "E-SYN-0"),
        (
            "no-price",
            "window 10min; currency \"JPY\"; rate Office per_minute",
            "E-SYN-0",
        ),
        ("no-string", "window 10min; currency", "E-SYN-0"),
        (
            "decimal",
            "window 10min; currency \"JPY\"; rate Office per_minute 1.5",
            "U-SYN-0",
        ),
        (
            "exponent",
            "window 10min; currency \"JPY\"; rate Office per_minute 1e2",
            "U-SYN-0",
        ),
        (
            "fraction",
            "window 10min; currency \"JPY\"; rate Office per_minute 1/2",
            "U-SYN-0",
        ),
        (
            "expression",
            "window 10min; currency \"JPY\"; rate Office per_minute 1+2",
            "U-SYN-0",
        ),
        (
            "positive-sign",
            "window 10min; currency \"JPY\"; rate Office per_minute +1",
            "U-SYN-0",
        ),
        (
            "rate-overflow",
            "window 10min; currency \"JPY\"; rate Office per_minute 18446744073709551616",
            "U-SYN-0",
        ),
        (
            "window-overflow",
            "window 18446744073709551616min; currency \"JPY\"",
            "U-SYN-0",
        ),
        (
            "actor-per-start",
            "window 10min; currency \"JPY\"; rate Office per_start 1",
            "U-SYN-0",
        ),
        (
            "cap-per-minute",
            "window 10min; currency \"JPY\"; rate Office.long per_minute 1",
            "U-SYN-0",
        ),
        (
            "three-path",
            "window 10min; currency \"JPY\"; rate Office.long.x per_start 1",
            "U-SYN-0",
        ),
        (
            "unknown-item",
            "window 10min; currency \"JPY\"; mystery 1",
            "U-SYN-0",
        ),
        (
            "unknown-method",
            "window 10min; currency \"JPY\"; rate Office per_hour 1",
            "U-SYN-0",
        ),
    ];
    let mut inputs: Vec<_> = fragments
        .into_iter()
        .map(|(label, fragment, code)| Input {
            label: label.into(),
            source: add(&base, &format!("metrics {{ {fragment} }}")),
            skeleton: base.clone(),
            code: Some(code),
        })
        .collect();
    let block = "metrics { window 10min; currency \"JPY\" }";
    inputs.push(Input {
        label: "duplicate-block".into(),
        source: add(&base, &format!("{block}\n{block}")),
        skeleton: base.clone(),
        code: Some("U-SEM-0"),
    });
    inputs.push(Input {
        label: "missing-brace".into(),
        source: add(&base, "metrics { window 10min; currency \"JPY\""),
        skeleton: base.clone(),
        code: Some("E-SYN-0"),
    });
    for (label, at) in [
        ("actor-scope", "actor Office {"),
        ("capability-scope", "capability short() -> T {"),
        ("process-scope", "process Serial {"),
        ("bounds-scope", "bounds {"),
    ] {
        inputs.push(Input {
            label: label.into(),
            source: base.replacen(at, &format!("{at} {block}"), 1),
            skeleton: base.clone(),
            code: Some("U-SYN-0"),
        });
    }
    for (label, fragment) in [
        ("no-rates", "window 10min; currency \"JPY\""),
        (
            "zero-rate",
            "rate Office per_minute 0; currency \"JPY\"; window 10min",
        ),
        (
            "newlines",
            "currency \"JPY\"\nwindow 10min\nrate Office.short per_start 0",
        ),
        (
            "max-rate",
            "window 10min; currency \"JPY\"; rate Office per_minute 18446744073709551615",
        ),
    ] {
        inputs.push(Input {
            label: label.into(),
            source: add(&base, &format!("metrics {{ {fragment} }}")),
            skeleton: base.clone(),
            code: None,
        });
    }
    for word in [
        "metrics",
        "window",
        "currency",
        "rate",
        "per_minute",
        "per_start",
    ] {
        let s = base
            .replace("Office", word)
            .replace("a =", &format!("{word} ="))
            .replace("{ pass }", &format!("{{ {word} }}"));
        inputs.push(Input {
            label: format!("identifier-{word}"),
            source: s.clone(),
            skeleton: s,
            code: None,
        });
        let s = base
            .replace(
                "  actor Office {",
                &format!("  people [{word}]\n  actor Office {{ executors [{word}]"),
            )
            .replace("capacity 2", "capacity 1");
        inputs.push(Input {
            label: format!("identifier-person-{word}"),
            source: s.clone(),
            skeleton: s,
            code: None,
        });
    }
    let forward = "metrics {window 10min;currency \"JPY\";rate Office per_minute 100;rate Office.short per_start 2;rate Office.long per_start 3}";
    inputs.push(Input {
        label: "forward-rate".into(),
        source: base.replacen("  type T", &format!("  {forward}\n  type T"), 1),
        skeleton: base.clone(),
        code: None,
    });
    inputs
}

fn static_failure(v: &Value, code: &str, s: &str) {
    assert_eq!(
        v["verdict"],
        if code.starts_with('E') {
            "invalid"
        } else {
            "unsupported"
        },
        "{v}"
    );
    assert!(v.get("metrics").is_none());
    assert_eq!(v["properties"], json!([]));
    assert_eq!(v["terminals"], json!([]));
    assert_eq!(v["agency"], Value::Null);
    for k in ["cases", "live_tasks", "clock", "truncated"] {
        assert!(v["bounds"][k].is_null());
    }
    let d = v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == code)
        .unwrap_or_else(|| panic!("missing {code}: {v}"));
    assert_eq!(d["severity"], "error");
    let start = d["span"]["start"].as_u64().unwrap() as usize;
    let end = d["span"]["end"].as_u64().unwrap() as usize;
    assert!(start <= end && end <= s.len() && s.is_char_boundary(start) && s.is_char_boundary(end));
}

#[test]
fn m_static_matrix_all_public_report_entries_and_utf8_spans() {
    for input in static_inputs() {
        let s = input
            .source
            .replace("cut C {", "cut C { question \"日本語\";");
        let s = add(&s, "simulation replications 4 seed 0 confidence 0.5");
        for sim in [false, true] {
            let v = run(&s, None, sim);
            if let Some(code) = input.code {
                static_failure(&v, code, &s);
            } else {
                assert!(
                    !["invalid", "unsupported"]
                        .iter()
                        .any(|x| v["verdict"] == *x),
                    "{}: {v}",
                    input.label
                );
                if input.label == "no-rates" {
                    assert_eq!(
                        v["metrics"]["overall"]["observed_cost_total"],
                        absent("not_configured", sim)
                    );
                    for kind in ["actors", "capabilities"] {
                        for rate in v["metrics"]["rates"][kind].as_array().unwrap() {
                            assert_eq!(rate["required"], true);
                            assert_eq!(rate["reason"], "not_configured");
                        }
                    }
                }
            }
        }
        let configured = add(
            &s,
            "configuration A { cut C; process Serial }\nconfiguration B { cut C; process Serial }",
        );
        if let Some(code) = input.code {
            for cfg in ["A", "B"] {
                for sim in [false, true] {
                    static_failure(&run(&configured, Some(cfg), sim), code, &configured);
                }
            }
            let c = serde_json::to_value(compare::compare(
                &configured,
                &["A", "B"],
                compare::Analysis::Exhaustive,
            ))
            .unwrap();
            let cs = serde_json::to_value(compare::compare(
                &configured,
                &["A", "B"],
                compare::Analysis::Simulation,
            ))
            .unwrap();
            let d = serde_json::to_value(diff::diff(&configured, "A", "B")).unwrap();
            for v in [&c, &cs, &d] {
                assert_eq!(
                    v["verdict"],
                    if code.starts_with('E') {
                        "invalid"
                    } else {
                        "unsupported"
                    }
                );
                assert!(v.get("metrics").is_none());
                assert!(v["tables"].get("metrics").is_none());
            }
        }
    }
}

pub fn boundary_inputs() -> Vec<Input> {
    let mut out = Vec::new();
    let single = source("met-sample")
        .replace("duration {2min:0.5, 4min:0.5}", "duration fixed 5min")
        .replace("clock 10min", "clock 3min")
        .replace("per_minute 7", "per_minute 2")
        .replace("per_start 3", "per_start 5");
    out.push(("clock-prefix", single));
    let zero = source("met-sample")
        .replace("duration {2min:0.5, 4min:0.5}", "duration fixed 1min")
        .replace("capacity 1", "capacity 0");
    out.push(("zero-capacity", zero));
    out.push((
        "no-task-end",
        source("met-sample").replace("job = A.run(); ", ""),
    ));
    out.push((
        "zero-duration",
        source("met-sample").replace("duration {2min:0.5, 4min:0.5}", "duration fixed 0min"),
    ));
    out.push((
        "three-reps",
        source("met-sample").replace("replications 4", "replications 3"),
    ));
    out.push((
        "mixed-cut",
        source("met-positive-support").replace("clock 10min", "clock 3min"),
    ));
    let people = source("met-person")
        .replace("executors [p]", "executors [p, q]")
        .replace(
            "capacity 1; executors [p, q]",
            "capacity 2; executors [p, q]",
        )
        .replace("fork f { a = A.a(); b = B.b() }; join all f;", "a = A.a();");
    out.push(("history-identity", people));
    let peak = source("met-sample")
        .replace("duration {2min:0.5, 4min:0.5}", "duration fixed 3min")
        .replace("capacity 1", "capacity 2")
        .replace("cases 1", "cases 2");
    out.push(("global-peak", add(&peak, "arrivals fixed [0min, 1min]")));
    let big = source("met-sample")
        .replace("duration {2min:0.5, 4min:0.5}", "duration fixed 2min")
        .replace("per_minute 7", "per_minute 18446744073709551615")
        .replace("per_start 3", "per_start 0");
    out.push(("big-cost", big));
    let cancel = source("met-cancel")
        .replace(
            "actor F { capacity 1; capability win()",
            "actor A { capacity 1; capability win()",
        )
        .replace(
            "  actor S { capacity 1; capability work()",
            "  actor S { capacity 1; capability unused()",
        );
    // Build the shared-Actor pair without changing any cancellation contract.
    let cancel=cancel.replace("result nondet { pass }; cancel forbidden_after_start } }", "result nondet { pass }; cancel forbidden_after_start } capability work() -> T { duration fixed 7min; result nondet { ok }; cancel cooperative ack_within 2min } }").replace("F.win()","A.win()").replace("S.work()","A.work()").replace("[F, S]","[A, S]").replace("rate F per_minute 2; rate S per_minute 3; rate F.win per_start 5; rate S.work per_start 7","rate A per_minute 2; rate A.win per_start 5; rate A.work per_start 7");
    out.push(("unstarted-cancel", cancel));
    let correlation="model Correlation version \"0.2\" {
 type T = enum { left, right }
 actor Gate { capacity 1; capability choose() -> T {duration fixed 1min; result nondet {left,right}} }
 actor A {capacity 1; capability run() -> T {duration fixed 3min; result nondet {left}}}
 actor B {capacity 1; capability run() -> T {duration fixed 5min; result nondet {right}}}
 process P {g=Gate.choose(); if g==left {a=A.run()} else {b=B.run()}; end done}
 cut C {include [Gate,A,B]}
 bounds {cases 1; live_tasks 1; clock 10min}
 metrics {window 10min; currency \"JPY\"; rate Gate per_minute 0; rate A per_minute 2; rate B per_minute 3; rate Gate.choose per_start 0; rate A.run per_start 0; rate B.run per_start 0}
 verify reachable end.done
}".to_owned();
    out.push(("correlation", correlation));
    let unused = add(
        &source("met-serial"),
        "actor Z {capacity 1; capability z() -> T {duration fixed 1min; result nondet {pass}}}",
    )
    .replace("include [Office]", "include [Office, Z]");
    out.push(("unused-rate", unused.clone()));
    out.push((
        "dead-branch-rate",
        unused.replace(
            "end done }",
            "if a == pass { end done } else { z = Z.z(); end done } }",
        ),
    ));
    out.push(("unselected-process-rate",add(&unused,"process Other { z=Z.z(); end done }\nconfiguration Selected {cut C; process Serial}\nconfiguration OtherPlan {cut C; process Other}")));
    let abort = source("met-sample")
        .replace(
            "process P { job = A.run(); end done }",
            "process P { fork f { job = A.run(); other = A.run() }; join all f; end done }",
        )
        .replace("live_tasks 1", "live_tasks 2");
    out.push(("abort", abort));
    let late=source("sim-keys")
        .replace("process P { a = A.c(); if a == first { end yes } else { end no } }", "process P { a = A.c(); if a == first { b = B.c(); end yes } else { end no } }")
        .replace("  bounds", "  actor B {capacity 1; capability c() -> T {duration fixed 1min; result nondet {first,second}}}\n  contract C { invariant A.c.finish -> y == first }\n  bounds");
    out.push(("abort-late",add(&late,"metrics {window 10min; currency \"JPY\"; rate A per_minute 2; rate B per_minute 2; rate A.c per_start 5; rate B.c per_start 5}")));
    out.into_iter()
        .map(|(label, s)| Input {
            label: label.into(),
            skeleton: strip_metrics(&s),
            source: s,
            code: None,
        })
        .collect()
}

#[test]
fn m_public_boundaries_prefix_deadlock_no_task_history_bigint_and_correlation() {
    for input in boundary_inputs() {
        let cfg = if input.label == "unselected-process-rate" {
            Some("Selected")
        } else {
            None
        };
        let sim = matches!(
            input.label.as_str(),
            "clock-prefix" | "zero-duration" | "three-reps" | "abort" | "abort-late"
        );
        let v = run(&input.source, cfg, sim);
        let m = &v["metrics"];
        assert!(m.is_object(), "{}: {v}", input.label);
        let o = &m["overall"];
        match input.label.as_str() {
            "clock-prefix" => {
                constant(o, "observed_until", "3", true);
                constant(o, "work", "3", true);
                constant(o, "observed_cost_total", "11", true);
                assert_eq!(
                    terminal(case(m, 1))["lead_time"],
                    absent("no_samples", true)
                );
            }
            "zero-capacity" => {
                eq(o, "work", "0", "0");
                eq(o, "observed_cost_total", "0", "0");
                count(&o["counts"]["deadlocked"], 1, false);
                assert_eq!(
                    row(&m["actors"], "actor", "A")["utilization"],
                    absent("zero_capacity", false)
                );
                assert_eq!(case(m, 1)["first_start_wait"], absent("deadlock", false));
            }
            "no-task-end" => {
                eq(terminal(case(m, 1)), "lead_time", "0", "0");
                eq(o, "work", "0", "0");
                eq(o, "peak_tasks", "0", "0");
                eq(o, "observed_cost_total", "0", "0");
                assert_eq!(
                    case(m, 1)["first_start_wait"],
                    absent("no_task_started", false)
                );
            }
            "zero-duration" => {
                constant(o, "work", "0", true);
                constant(o, "peak_tasks", "1", true);
                constant(o, "observed_cost_total", "3", true);
            }
            "three-reps" => {
                assert_eq!(o["work"], sample("8/3", "2", "4", 3));
                assert_eq!(o["observed_cost_total"], sample("65/3", "17", "31", 3));
            }
            "mixed-cut" => {
                assert_eq!(m["status"], "truncated");
                assert_all_truncated(m);
                assert!(!v["terminals"].as_array().unwrap().is_empty());
            }
            "history-identity" => {
                eq(o, "work", "3", "3");
                eq(o, "named_people_work", "3", "3");
                for id in ["p", "q"] {
                    eq(row(&m["people"], "person", id), "work", "0", "3");
                }
            }
            "global-peak" => {
                eq(o, "work", "6", "6");
                eq(o, "peak_tasks", "2", "2");
                eq(
                    row(&m["actors"], "actor", "A"),
                    "utilization",
                    "3/10",
                    "3/10",
                );
            }
            "big-cost" => eq(
                o,
                "observed_cost_total",
                "36893488147419103230",
                "36893488147419103230",
            ),
            "unstarted-cancel" => {
                eq(terminal(case(m, 1)), "lead_time", "3", "10");
                eq(o, "work", "3", "10");
                eq(o, "peak_tasks", "1", "1");
                eq(o, "observed_cost_total", "11", "32");
                eq(
                    row(&m["capabilities"], "capability", "A.work"),
                    "starts",
                    "0",
                    "1",
                );
            }
            "correlation" => {
                eq(o, "work", "4", "6");
                eq(o, "observed_cost_total", "6", "15");
            }
            "unused-rate" | "unselected-process-rate" => eq(o, "observed_cost_total", "805", "805"),
            "dead-branch-rate" => {
                assert_eq!(o["observed_cost_total"], absent("not_configured", false))
            }
            "abort" => {
                assert_eq!(v["verdict"], "unsupported");
                assert_eq!(m["status"], "aborted");
                assert_all_reason(m, "aborted");
            }
            "abort-late" => {
                assert_eq!(v["verdict"], "unsupported");
                assert_eq!(v["simulation"]["attempted_replications"], 3);
                assert_eq!(v["simulation"]["completed_replications"], 2);
                assert_eq!(m["status"], "aborted");
                assert_all_reason(m, "aborted");
            }
            _ => unreachable!(),
        }
    }
}
pub fn assert_all_reason(v: &Value, reason: &str) {
    if let Some(o) = v.as_object() {
        if o.contains_key("min") && o.contains_key("max") {
            assert!(v["min"].is_null() && v["max"].is_null());
            assert_eq!(v["reason"], reason);
            if o.contains_key("samples") {
                assert_eq!(v["samples"], 0);
                assert!(v["mean"].is_null());
            }
        } else {
            for x in o.values() {
                assert_all_reason(x, reason);
            }
        }
    } else if let Some(a) = v.as_array() {
        for x in a {
            assert_all_reason(x, reason);
        }
    }
}
pub fn assert_all_truncated(v: &Value) {
    assert_all_reason(v, "truncated");
}

#[test]
fn m_opt_in_keeps_legacy_wire_and_sampling_keys() {
    let s = source("met-legacy.broken-enabled");
    let bare = strip_metrics(&s);
    let mut before = run(&bare, None, false);
    let mut after = run(&s, None, false);
    assert!(before.get("metrics").is_none());
    assert!(after.as_object_mut().unwrap().remove("metrics").is_some());
    for v in [&mut before, &mut after] {
        v["model"]["sha256"] = Value::Null;
        v["assumptions"]
            .as_array_mut()
            .unwrap()
            .retain(|a| !a["field"].as_str().unwrap().starts_with("metrics."));
    }
    assert_eq!(
        ledger_shape::original_report(&before),
        ledger_shape::original_report(&after)
    );
}

#[test]
fn m_comparison_raw_reports_and_metric_tables_are_ordered_not_ranked() {
    for name in [
        "met-compare",
        "met-compare.broken-longer-task",
        "met-diff",
        "met-diff.broken-free-start",
    ] {
        let s = source(name);
        let c = serde_json::to_value(compare::compare(
            &s,
            &["ToBeP", "AsIs"],
            compare::Analysis::Exhaustive,
        ))
        .unwrap();
        for (i, cfg) in ["ToBeP", "AsIs"].into_iter().enumerate() {
            assert_eq!(c["plans"][i]["report"], run(&s, Some(cfg), false));
        }
        let rows = c["tables"]["metrics"].as_array().expect("metrics table");
        assert!(!rows.is_empty());
        for row in rows {
            let vs = row["values"].as_array().unwrap();
            assert_eq!(vs.len(), 2);
            assert_eq!(vs[0]["configuration"], "ToBeP");
            assert_eq!(vs[1]["configuration"], "AsIs");
            for key in ["delta", "rank", "verdict"] {
                assert!(row.get(key).is_none());
            }
        }
        assert!(
            c["checked"]
                .as_array()
                .unwrap()
                .contains(&json!("metric_tables"))
        );
        assert!(
            !c["unchecked"]
                .as_array()
                .unwrap()
                .contains(&json!("metrics"))
        );
        let d = serde_json::to_value(diff::diff(&s, "AsIs", "ToBeP")).unwrap();
        for (side, cfg) in [("from", "AsIs"), ("to", "ToBeP")] {
            assert_eq!(d[side]["report"], run(&s, Some(cfg), false));
        }
        assert!(
            !d["metrics"]
                .as_array()
                .expect("diff metrics rows")
                .is_empty()
        );
        assert!(
            d["differences"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["kind"] != "actor" && r["kind"] != "relation")
        );
    }
}

pub fn comparison_inputs() -> Vec<Input> {
    let sample = source("met-sample").replace("include [A]", "include [A, B]");
    let sample = add(
        &sample,
        "actor B {capacity 1; capability run() -> T {duration fixed 2min; result nondet {pass}}}\nprocess Q {job=B.run();end done}\nconfiguration AsIs {cut C;process P}\nconfiguration ToBeP {cut C;process Q}",
    );
    let sample = sample.replace(
        "rate A.run per_start 3",
        "rate A.run per_start 3; rate B per_minute 7; rate B.run per_start 3",
    );
    let fifo = source("met-fifo");
    let fifo = add(
        &fifo,
        "process Q {a=A.prep();b=B.finish();end done}\nconfiguration AsIs {cut C;process P}\nconfiguration ToBeP {cut C;process Q}",
    );
    let union=fifo.replace("process Q {a=A.prep();b=B.finish();end done}","process Q {a=A.prep();b=B.fast();end done}")
        .replace("capability finish() -> T", "capability fast() -> T {duration fixed 4min; result nondet {pass}}; capability finish() -> T")
        .replace("rate B.finish per_start 4", "rate B.finish per_start 4; rate B.fast per_start 4");
    [
        ("compare-simulation", sample),
        ("diff-new-route", fifo),
        ("diff-union", union),
    ]
    .into_iter()
    .map(|(label, s)| Input {
        label: label.into(),
        skeleton: strip_metrics(&s),
        source: s,
        code: None,
    })
    .collect()
}

#[test]
fn m_comparison_simulation_and_arrivals_union_preserve_raw_values() {
    for input in comparison_inputs() {
        let sim = input.label == "compare-simulation";
        let mode = if sim {
            compare::Analysis::Simulation
        } else {
            compare::Analysis::Exhaustive
        };
        for names in [["ToBeP", "AsIs"], ["AsIs", "ToBeP"]] {
            let c = serde_json::to_value(compare::compare(&input.source, &names, mode)).unwrap();
            for (i, cfg) in names.into_iter().enumerate() {
                let raw = run(&input.source, Some(cfg), sim);
                assert_eq!(c["plans"][i]["report"], raw);
                let o = &raw["metrics"]["overall"];
                if sim {
                    assert_eq!(
                        o["observed_cost_total"],
                        if cfg == "AsIs" {
                            sample("24", "17", "31", 4)
                        } else {
                            sample("17", "17", "17", 4)
                        }
                    );
                    assert_eq!(
                        terminal(case(&raw["metrics"], 1))["lead_time"],
                        if cfg == "AsIs" {
                            sample("3", "2", "4", 4)
                        } else {
                            sample("2", "2", "2", 4)
                        }
                    );
                } else {
                    eq(
                        o,
                        "observed_cost_total",
                        if cfg == "ToBeP" && input.label == "diff-union" {
                            "69"
                        } else {
                            "78"
                        },
                        if cfg == "ToBeP" && input.label == "diff-union" {
                            "69"
                        } else {
                            "78"
                        },
                    );
                }
            }
            assert!(
                !c["tables"]["metrics"]
                    .as_array()
                    .expect("cross-mode metric table")
                    .is_empty()
            );
        }
        if !sim {
            let d = serde_json::to_value(diff::diff(&input.source, "AsIs", "ToBeP")).unwrap();
            assert_eq!(d["terminals"], json!([]));
            let rows = d["metrics"].as_array().expect("arrivals metric table");
            assert!(!rows.is_empty());
            if input.label == "diff-union" {
                let missing: Vec<_> = rows
                    .iter()
                    .filter(|r| {
                        r["from"]["status"] == "not_applicable"
                            || r["to"]["status"] == "not_applicable"
                    })
                    .collect();
                assert!(!missing.is_empty());
                for row in missing {
                    for side in ["from", "to"] {
                        if row[side]["status"] == "not_applicable" {
                            assert!(row[side]["value"].is_null());
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn m_no_task_and_deadlock_simulation_do_not_become_rejected_zero_leads() {
    for input in boundary_inputs()
        .into_iter()
        .filter(|i| ["no-task-end", "zero-capacity"].contains(&i.label.as_str()))
    {
        let v = run(&input.source, None, true);
        let m = &v["metrics"];
        assert!(m.is_object());
        constant(&m["overall"], "work", "0", true);
        constant(&m["overall"], "observed_cost_total", "0", true);
        if input.label == "no-task-end" {
            constant(terminal(case(m, 1)), "lead_time", "0", true);
            assert_eq!(
                case(m, 1)["first_start_wait"],
                absent("no_task_started", true)
            );
        } else {
            assert_eq!(
                terminal(case(m, 1))["lead_time"],
                absent("no_samples", true)
            );
            count(&m["overall"]["counts"]["deadlocked"], 1, true);
        }
    }
}

#[test]
fn m_report_metric_number_roundtrip_above_probability_range_and_closed_nested_dto() {
    let v = run(&source("met-serial"), None, false);
    assert!(v["metrics"].is_object());
    for n in ["8", "805", "8/3", "36893488147419103230"] {
        let mut x = v.clone();
        x["metrics"]["overall"]["work"] = scalar(n, n);
        let typed: report::Report =
            serde_json::from_value(x.clone()).expect("MetricNumber is not probability::Rational");
        assert_eq!(serde_json::to_value(typed).unwrap(), x);
    }
    fn objects(v: &Value, path: String, out: &mut Vec<String>) {
        if let Some(o) = v.as_object() {
            out.push(path.clone());
            for (k, x) in o {
                objects(x, format!("{path}/{k}"), out);
            }
        } else if let Some(a) = v.as_array() {
            for (i, x) in a.iter().enumerate() {
                objects(x, format!("{path}/{i}"), out);
            }
        }
    }
    let mut paths = Vec::new();
    objects(&v["metrics"], "/metrics".into(), &mut paths);
    for path in paths {
        let mut x = v.clone();
        x.pointer_mut(&path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown_metric_field".into(), Value::Null);
        assert!(
            serde_json::from_value::<report::Report>(x).is_err(),
            "new DTO must deny unknown in {path}"
        );
    }
}
