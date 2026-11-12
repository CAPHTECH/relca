//! Specification arithmetic and selected JSON paths, independent of production.
#![allow(dead_code)]
use serde_json::{Value, json};

pub fn point(n: u64) -> Value {
    json!({"min":n,"max":n})
}
pub fn range(a: u64, b: u64) -> Value {
    json!({"min":a,"max":b})
}
pub fn scalar(n: impl ToString) -> Value {
    let n = n.to_string();
    json!({"min":n,"max":n,"reason":null})
}
fn metric_range(a: u64, b: u64) -> Value {
    json!({"min":a.to_string(),"max":b.to_string(),"reason":null})
}
fn terminal(end: &str, arrival: u64, lo: u64, hi: u64, work: u64) -> Value {
    json!({"kind":"end","end":end,"completed_at":range(lo,hi),
        "elapsed":range(lo-arrival,hi-arrival),"work":point(work),"result_at":null})
}
fn case(id: u64, arrival: u64, end: &str, lo: u64, hi: u64, work: u64) -> Value {
    json!({"case_id":id,"arrival":arrival,"terminals":[terminal(end,arrival,lo,hi,work)]})
}
fn done(arrivals: &[u64], ends: &[(u64, u64)], works: &[u64]) -> Value {
    json!({"verdict":"satisfied","bounds":{"truncated":false},
        "case_outcomes":arrivals.iter().enumerate().map(|(i,a)|case(i as u64+1,*a,"done",ends[i].0,ends[i].1,works[i])).collect::<Vec<_>>()})
}
fn task(binding: &str, wait: (u64, u64), work: u64) -> Value {
    json!({"binding":binding,"wait":metric_range(wait.0,wait.1),"work":scalar(work),"starts":scalar(1)})
}
fn metric_case(id: u64, tasks: Value) -> Value {
    json!({"case_id":id,"tasks":tasks})
}
fn probabilities(values: &[(&str, &str)]) -> Value {
    json!([{"id":"probability reached #1","cases":values.iter().enumerate().map(|(i,(a,b))|
        json!({"case_id":i+1,"probability":{"min":a,"max":b}})).collect::<Vec<_>>() }])
}
fn won(arrivals: &[u64], ends: &[u64], works: &[u64], winners: &[Option<bool>]) -> Value {
    let cases: Vec<_> = arrivals
        .iter()
        .enumerate()
        .map(|(i, a)| {
            let mut row = case(
                i as u64 + 1,
                *a,
                if winners[i] == Some(false) {
                    "lost"
                } else {
                    "won"
                },
                ends[i],
                ends[i],
                works[i],
            );
            if winners[i].is_none() {
                row["terminals"] = json!([
                    terminal("won", *a, ends[i], ends[i], works[i]),
                    terminal("lost", *a, ends[i], ends[i], works[i])
                ]);
            }
            row
        })
        .collect();
    let ps: Vec<_> = winners
        .iter()
        .map(|w| match w {
            Some(true) => ("1", "1"),
            Some(false) => ("0", "0"),
            None => ("0", "1"),
        })
        .collect();
    json!({"verdict":"satisfied","bounds":{"truncated":false},"case_outcomes":cases,"properties":probabilities(&ps)})
}

pub fn expected(name: &str) -> Value {
    match name {
        "start" | "label" | "explain-start" => {
            let mut v = done(&[0, 0, 1], &[(6, 6), (2, 2), (4, 4)], &[2, 2, 2]);
            v["metrics"] = json!({"cases":[metric_case(1,json!([task("r",(4,4),2)])),
                metric_case(2,json!([task("r",(0,0),2)])),metric_case(3,json!([task("r",(1,1),2)]))],
                "actors":[{"actor":if name=="label"{"Desk"}else{"A"},"work":scalar(6),
                    "queue":{"wait_total":{"stats":scalar(5)},"length_max":scalar(2),"length_time_average":{"stats":scalar("5/6")}}}]});
            v["properties"] = probabilities(&[("0", "0"), ("1", "1"), ("0", "0")]);
            if name.starts_with("explain") {
                v["verdict"] = json!("violated");
            }
            v
        }
        "start-tie" => done(&[0, 0, 1], &[(4, 6), (2, 2), (4, 6)], &[2, 2, 2]),
        "equal" | "equal-explicit" => {
            let mut v = done(&[0, 0], &[(2, 4), (2, 4)], &[2, 2]);
            v["properties"] = probabilities(&[("0", "1"), ("0", "1")]);
            v
        }
        "equal-fifo" | "unused" | "scenario" => {
            let mut v = done(&[0, 0], &[(2, 2), (4, 4)], &[2, 2]);
            v["properties"] = probabilities(&[("1", "1"), ("0", "0")]);
            v
        }
        "max-rank" => done(&[0, 0], &[(4, 4), (2, 2)], &[2, 2]),
        "call-order" | "call-order-reverse" => {
            let mut v = done(&[0], &[(5, 5)], &[5]);
            let reverse = name.ends_with("reverse");
            v["metrics"] = json!({"cases":[metric_case(1,json!([
                task("short",if reverse{(3,3)}else{(0,0)},2),task("long",if reverse{(0,0)}else{(2,2)},3)]))]});
            v
        }
        "people" | "people-environment" => {
            let env = name.ends_with("environment");
            let mut v = done(
                &[0, 2],
                if env {
                    &[(6, 9), (7, 9)]
                } else {
                    &[(9, 9), (7, 7)]
                },
                &[3, 3],
            );
            v["metrics"] =
                json!({"people":[{"person":"p","work":scalar(5),"peak_tasks":scalar(1)}]});
            v
        }
        "calendar" | "calendar-later" => {
            let later = u64::from(name.ends_with("later"));
            let mut v = done(
                &[0, 1],
                &[(8 + later, 8 + later), (6 + later, 6 + later)],
                &[2, 2],
            );
            v["metrics"] = json!({"cases":[metric_case(1,json!([task("r",(6+later,6+later),2)])),
                metric_case(2,json!([task("r",(3+later,3+later),2)]))]});
            v
        }
        "external"
        | "external-fifo"
        | "external-equal"
        | "external-equal-fifo"
        | "explain-external" => {
            let equal = name == "external-equal";
            let fifo = matches!(name, "external-fifo" | "external-equal-fifo");
            let ends = if equal {
                [(4, 4), (8, 12), (8, 12)]
            } else if fifo {
                [(4, 4), (8, 8), (12, 12)]
            } else {
                [(4, 4), (12, 12), (8, 8)]
            };
            let admitted = if equal {
                [(0, 0), (4, 8), (4, 8)]
            } else if fifo {
                [(0, 0), (4, 4), (8, 8)]
            } else {
                [(0, 0), (8, 8), (4, 4)]
            };
            let mut v = done(&[0, 1, 2], &ends, &[4, 4, 4]);
            for (i, (a, b)) in admitted.into_iter().enumerate() {
                v["case_outcomes"][i]["admitted_at"] = range(a, b);
            }
            v["admission"] = json!({"rejected_count":point(0),"external_wait_max":point(2)});
            v["metrics"] = json!({"cases":admitted.iter().enumerate().map(|(i,(a,b))|
                json!({"case_id":i+1,"external_wait":metric_range(a-i as u64,b-i as u64)})).collect::<Vec<_>>()});
            if name.starts_with("explain") {
                v["verdict"] = json!("violated");
            }
            v
        }
        "initial" => {
            let mut v = done(&[0, 0], &[(2, 2), (4, 4)], &[2, 2]);
            v["case_outcomes"][0]["admitted_at"] = point(0);
            v["case_outcomes"][1]["admitted_at"] = point(2);
            v
        }
        "conflict" | "explain-conflict" => {
            let mut v = won(&[0, 0], &[2, 2], &[2, 2], &[Some(false), Some(true)]);
            if name.starts_with("explain") {
                v["verdict"] = json!("violated");
            }
            v
        }
        "conflict-no-resolve" | "conflict-equal" => won(&[0, 0], &[2, 2], &[2, 2], &[None, None]),
        "conflict-id" => won(&[0, 0], &[2, 2], &[2, 2], &[Some(true), Some(false)]),
        "future" => won(&[0, 0], &[3, 4], &[3, 3], &[Some(true), Some(false)]),
        "future-tied" => won(&[0, 0], &[3, 3], &[3, 2], &[Some(false), Some(true)]),
        "stale" => won(
            &[0, 1, 4],
            &[5, 3, 5],
            &[5, 2, 1],
            &[Some(false), Some(true), Some(true)],
        ),
        "unrelated" => json!({"verdict":"violated","bounds":{"truncated":false},
            "case_outcomes":[case(1,0,"done",3,3,3),case(2,0,"done",3,3,2)]}),
        "ack" => json!({"verdict":"satisfied","bounds":{"truncated":false},
            "case_outcomes":[{"case_id":1,"arrival":0,"terminals":[{"end":"done","completed_at":point(2),"work":point(3),"result_at":point(1)}]}],
            "properties":probabilities(&[("0","1")])}),
        "actor-hol" | "resource-hol" | "independent-hol" => {
            json!({"verdict":"violated","bounds":{"truncated":false},
            "case_outcomes":[{"case_id":1,"terminals":[]}],"metrics":{"overall":{"work":scalar(u64::from(name=="independent-hol"))}}})
        }
        "unattributed" | "physical" => {
            let physical = name == "physical";
            let mut small = task("small", if physical { (2, 3) } else { (2, 2) }, 1);
            // When both need R, big precedes small and occupies it [4,5).
            if physical {
                small = task("small", (3, 3), 1);
            }
            small["wait_causes"] = json!({"actor_capacity":scalar(0),"resource":scalar(if physical{3}else{0}),
                "executor":scalar(0),"combined":scalar(0),"unattributed":scalar(if physical{0}else{2})});
            let mut v = done(
                &[0, 2],
                if physical {
                    &[(4, 4), (6, 6)]
                } else {
                    &[(4, 4), (5, 5)]
                },
                &[4, 2],
            );
            v["metrics"] = json!({"cases":[metric_case(2,json!([task("big",(2,2),1),small]))]});
            v
        }
        "clock" => json!({"verdict":"inconclusive","bounds":{"truncated":true},
            "case_outcomes":[{"case_id":1,"admitted_at":point(0),"terminals":[terminal("done",0,4,4,4)]},
                {"case_id":2,"admitted_at":null,"terminals":[]},{"case_id":3,"admitted_at":point(4),"terminals":[]}],
            "admission":{"rejected_count":point(0)}}),
        "horizon" => json!({"verdict":"inconclusive","bounds":{"truncated":false},
            "case_outcomes":[{"case_id":1,"terminals":[terminal("done",0,4,4,4)]},
                {"case_id":2,"admitted_at":null,"terminals":[{"kind":"cut_off","end":null,"completed_at":null,"elapsed":null,"work":point(0)}]},
                {"case_id":3,"admitted_at":point(4),"terminals":[{"kind":"cut_off","end":null,"completed_at":null,"elapsed":null,"work":point(2)}]}]}),
        "retry" => json!({"verdict":"satisfied","bounds":{"truncated":false},
            "case_outcomes":[case(1,0,"duplicate",4,4,2),case(2,0,"duplicate",2,2,2)],
            "properties":retry_properties()}),
        "retry-unknown" => json!({"verdict":"inconclusive","bounds":{"truncated":false},
            "case_outcomes":[unknown_case(1,4),unknown_case(2,2)],"properties":retry_properties()}),
        "equal-sim" | "external-equal-sim" => json!({"verdict":"unsupported"}),
        _ => panic!("missing hand oracle {name}"),
    }
}
fn unknown_case(id: u64, at: u64) -> Value {
    json!({"case_id":id,"arrival":0,"terminals":[{"kind":"receipt_unknown","end":null,"completed_at":null,
        "elapsed":null,"work":null,"observed_until":point(at),"observed_elapsed":point(at),"observed_work":point(2)}]})
}
fn retry_properties() -> Value {
    let mut properties = probabilities(&[("1", "1"), ("1", "1")]);
    properties
        .as_array_mut()
        .unwrap()
        .push(json!({"id":"contract Once #1 at_most_once","verdict":"satisfied"}));
    properties
}

/// Object subsets are selected paths; identity arrays reject duplicate rows.
/// Terminal and offered-case inventories are closed, never silently subsetted.
pub fn subset(want: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match want {
        Value::Object(fields) => {
            let a = actual
                .as_object()
                .ok_or_else(|| format!("{path}: expected object, got {actual}"))?;
            for (key, value) in fields {
                let child = a
                    .get(key)
                    .ok_or_else(|| format!("{path}/{key}: missing key"))?;
                subset(value, child, &format!("{path}/{key}"))?;
            }
        }
        Value::Array(rows) => {
            let a = actual
                .as_array()
                .ok_or_else(|| format!("{path}: expected array"))?;
            if (path.ends_with("/terminals") || path.ends_with("/case_outcomes"))
                && a.len() != rows.len()
            {
                return Err(format!(
                    "{path}: expected {} rows, got {}",
                    rows.len(),
                    a.len()
                ));
            }
            for (i, row) in rows.iter().enumerate() {
                let identity = ["case_id", "binding", "actor", "person", "id", "end"]
                    .into_iter()
                    .find(|k| row.get(*k).is_some());
                let child = if let Some(key) = identity {
                    let found: Vec<_> = a.iter().filter(|r| r.get(key) == row.get(key)).collect();
                    if found.len() != 1 {
                        return Err(format!("{path}: nonunique {key}={}", row[key]));
                    }
                    found[0]
                } else {
                    a.get(i).ok_or_else(|| format!("{path}/{i}: missing row"))?
                };
                subset(row, child, &format!("{path}/{i}"))?;
            }
        }
        _ if want != actual => return Err(format!("{path}: hand {want}, observed {actual}")),
        _ => {}
    }
    Ok(())
}

pub fn ranks(name: &str) -> Vec<u64> {
    match name {
        "start" | "label" | "explain-start" => vec![0, 10, 20],
        "start-tie" => vec![0, 10, 0],
        "equal" | "equal-explicit" | "equal-fifo" | "equal-sim" | "scenario" | "conflict-equal"
        | "conflict-id" => vec![5, 5],
        "calendar" | "calendar-later" => vec![0, 10],
        "external-equal" | "external-equal-fifo" | "external-equal-sim" => vec![0, 1, 1],
        "external" | "external-fifo" | "clock" | "horizon" | "explain-external" => vec![0, 1, 9],
        "call-order" | "call-order-reverse" => vec![7],
        "ack" | "actor-hol" | "resource-hol" | "independent-hol" => vec![9],
        "stale" => vec![9, 5, 0],
        "max-rank" => vec![0, u64::MAX],
        _ => vec![0, 9],
    }
}
pub fn policy(name: &str) -> Value {
    let source = super::input::source(name);
    let starts = if source.contains("policy observable priority descending") {
        let tie = if source.contains("tie fifo tie_by case_id then call_order") {
            "fifo_case_id_call_order"
        } else if source.contains("policy observable priority descending tie fifo") {
            "fifo_case_id"
        } else {
            "nondet"
        };
        let actors: Vec<&str> = match name {
            "people" => vec!["A", "B"],
            "people-environment" => vec!["A"],
            "label" => vec!["Desk"],
            "actor-hol" => vec!["A"],
            "resource-hol" | "independent-hol" => vec!["A", "B"],
            _ => vec!["A"],
        };
        json!({"direction":"descending","tie":tie,"actors":actors,"assign_first_idle":source.contains("assign first_idle"),"scope":"policy_queued_only"})
    } else {
        Value::Null
    };
    let external = if source.contains("wait_external priority descending") {
        json!({"direction":"descending","tie":if source.contains("wait_external priority descending tie fifo"){"fifo_case_id"}else{"nondet"},"scope":"held_readmission_only"})
    } else {
        Value::Null
    };
    let resolution = if source.contains(" resolve priority descending") {
        let caps: Vec<&str> = match name {
            "future" | "future-tied" | "unrelated" => vec!["A.low", "A.high"],
            "stale" => vec!["A.high", "A.middle", "A.low"],
            _ => vec!["A.claim"],
        };
        json!(caps.iter().map(|cap|json!({"capability":cap,"direction":"descending","tie":if name=="conflict-id"{"case_id"}else{"nondet"},"scope":"due_checked_intersection_only"})).collect::<Vec<_>>())
    } else {
        Value::Null
    };
    json!({"rules":"priority-queues-v1","case_ranks":ranks(name),"known_at":"receipt","immutable":true,
        "starts":starts,"external":external,"version_resolution":resolution})
}

pub fn validate(name: &str, v: &Value) -> Result<(), String> {
    subset(&expected(name), v, "")?;
    if name.ends_with("-sim") {
        return diagnostic(v, "U-SEM-0", "unresolved simulation nondeterminism");
    }
    let actual = v
        .get("priority_policy")
        .ok_or("/priority_policy: missing")?;
    if *actual != policy(name) {
        return Err(format!(
            "/priority_policy: expected {}, got {actual}",
            policy(name)
        ));
    }
    let rows = v
        .pointer("/ledger/derivation/components")
        .and_then(Value::as_array)
        .ok_or("/ledger/derivation/components: missing")?;
    let hits: Vec<_> = rows
        .iter()
        .filter(|r| r["name"] == "/priority_policy/rules")
        .collect();
    if hits.len() != 1
        || *hits[0]
            != json!({"name":"/priority_policy/rules","at":"/priority_policy/rules","version":"priority-queues-v1"})
    {
        return Err("priority local rule version/link mismatch".into());
    }
    if matches!(name, "retry" | "retry-unknown") {
        let receipts = v["response_receipts"]
            .as_array()
            .ok_or("response_receipts missing")?;
        if receipts.len() != 4 {
            return Err("two receipts per original case required".into());
        }
        for (case, first, last) in [(1, 3, 4), (2, 1, 2)] {
            let mut command = None;
            for (attempt, at) in [(1, first), (2, last)] {
                let rows: Vec<_> = receipts
                    .iter()
                    .filter(|r| r["case_id"] == case && r["attempt"] == attempt)
                    .collect();
                if rows.len() != 1 {
                    return Err("nonunique original case/attempt receipt".into());
                }
                let delivered = attempt == 2 && name == "retry";
                subset(
                    &json!({"at":point(at),"receipt":if delivered{"delivered"}else{"unknown"},
                    "observed_result":if delivered{Some("rejected_duplicate")}else{None}}),
                    rows[0],
                    "/response_receipts",
                )?;
                let actual = rows[0]["command"]
                    .as_str()
                    .ok_or("logical command missing")?;
                if command.is_some_and(|old| old != actual) {
                    return Err("retry changed original command identity".into());
                }
                command = Some(actual);
            }
        }
    }
    Ok(())
}
pub fn diagnostic(v: &Value, code: &str, reason: &str) -> Result<(), String> {
    let ds = v["diagnostics"].as_array().ok_or("diagnostics missing")?;
    if !ds
        .iter()
        .any(|d| d["code"] == code && d["message"].as_str().is_some_and(|m| m.contains(reason)))
    {
        return Err(format!(
            "required {code}/{reason}, got {}",
            v["diagnostics"]
        ));
    }
    Ok(())
}
pub fn witness(name: &str) -> Value {
    let mut v = expected(name);
    v["priority_policy"] = policy(name);
    v["ledger"] = json!({"derivation":{"components":[{"name":"/priority_policy/rules","at":"/priority_policy/rules","version":"priority-queues-v1"}]}});
    v
}
pub fn faults() -> Vec<(&'static str, &'static str, Value)> {
    let mut fifo = witness("equal");
    fifo["case_outcomes"][0]["terminals"][0]["completed_at"] = point(2);
    let mut initial = witness("initial");
    initial["case_outcomes"][0]["admitted_at"] = point(2);
    let mut stale = witness("stale");
    stale["case_outcomes"][0]["terminals"][0]["end"] = json!("won");
    let mut future = witness("future");
    future["case_outcomes"][0]["terminals"][0]["completed_at"] = point(4);
    let mut physical = witness("unattributed");
    physical["metrics"]["cases"][0]["tasks"][1]["wait_causes"]["unattributed"] = scalar(0);
    physical["metrics"]["cases"][0]["tasks"][1]["wait_causes"]["actor_capacity"] = scalar(2);
    let mut missing = witness("start");
    missing.as_object_mut().unwrap().remove("priority_policy");
    let mut wrong = witness("start");
    wrong["priority_policy"]["case_ranks"][2] = json!(0);
    let mut link = witness("start");
    link["ledger"]["derivation"]["components"][0]["version"] = json!("resource-time-v1");
    let mut absent = witness("unused");
    absent["priority_policy"]
        .as_object_mut()
        .unwrap()
        .remove("external");
    vec![
        ("equal", "omitted tie inferred FIFO", fifo),
        ("initial", "initial admission sorted by priority", initial),
        ("stale", "stale cleared for high rank", stale),
        ("future", "wait for future high finish", future),
        (
            "unattributed",
            "priority wait reported as physical obstruction",
            physical,
        ),
        ("start", "missing priority wire", missing),
        ("start", "wrong rank table", wrong),
        ("start", "wrong local rule version", link),
        ("unused", "missing versus null scope", absent),
    ]
}
pub fn exit(name: &str) -> i32 {
    match expected(name)["verdict"].as_str().unwrap() {
        "satisfied" => 0,
        "violated" => 1,
        "inconclusive" => 2,
        "invalid" => 3,
        "unsupported" => 4,
        _ => 5,
    }
}
