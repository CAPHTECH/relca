//! ADR0059 finite hand oracle, not production output or a replacement scheduler.
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
pub fn span(a: &str, b: &str) -> Value {
    json!({"min":a,"max":b,"reason":null})
}
pub fn absent(reason: &str) -> Value {
    json!({"min":null,"max":null,"reason":reason})
}
pub fn sample(mean: &str, min: &str, max: &str) -> Value {
    json!({"mean":mean,"min":min,"max":max,"samples":4,"reason":null})
}

/// Arrays of identified rows are matched by identity, not product ordering.
/// Every expected scalar object is closed, including null and its reason.
pub fn subset(want: &Value, actual: &Value, path: &str) -> Result<(), String> {
    match want {
        Value::Object(fields) => {
            let a = actual
                .as_object()
                .ok_or_else(|| format!("{path}: required object, got {actual}"))?;
            if fields.contains_key("min") || fields.contains_key("mean") {
                if want != actual {
                    return Err(format!("{path}: hand scalar {want}, got {actual}"));
                }
                return Ok(());
            }
            for (key, value) in fields {
                let child = a
                    .get(key)
                    .ok_or_else(|| format!("{path}/{key}: missing required key"))?;
                subset(value, child, &format!("{path}/{key}"))?;
            }
        }
        Value::Array(rows) => {
            let a = actual
                .as_array()
                .ok_or_else(|| format!("{path}: required array"))?;
            for (i, row) in rows.iter().enumerate() {
                let identity = [
                    "case_id", "binding", "actor", "person", "resource", "id", "attempt", "end",
                ]
                .into_iter()
                .find(|key| row.get(*key).is_some());
                let child = if let Some(key) = identity {
                    let matches: Vec<_> = a.iter().filter(|r| r.get(key) == row.get(key)).collect();
                    if matches.len() != 1 {
                        return Err(format!(
                            "{path}: unique {key}={}, found {}",
                            row[key],
                            matches.len()
                        ));
                    }
                    matches[0]
                } else {
                    a.get(i).ok_or_else(|| format!("{path}/{i}: missing"))?
                };
                subset(row, child, &format!("{path}/{i}"))?;
            }
            if rows.is_empty() && !a.is_empty() {
                return Err(format!("{path}: expected empty inventory"));
            }
        }
        _ if want != actual => return Err(format!("{path}: expected {want}, got {actual}")),
        _ => {}
    }
    Ok(())
}

pub fn causes(actor: u64, resource: u64, executor: u64, combined: u64, calendar: u64) -> Value {
    json!({"actor_capacity":scalar(actor),"resource":scalar(resource),"executor":scalar(executor),
        "combined":scalar(combined),"calendar":scalar(calendar),"condition":scalar(0),"unattributed":scalar(0)})
}
pub fn partition(service: u64, actor: u64, combined: u64, calendar: u64, condition: u64) -> Value {
    json!({"external":scalar(0),"in_service":scalar(service),"contest_route":scalar(0),
        "queue":{"actor_capacity":scalar(actor),"resource":scalar(0),"executor":scalar(0),
        "combined":scalar(combined),"calendar":scalar(calendar)},"condition":scalar(condition),
        "intentional":scalar(0),"unattributed":scalar(0)})
}
pub fn task(binding: &str, work: u64, wait: u64, parts: Value, predecessor: u64) -> Value {
    json!({"binding":binding,"work":scalar(work),"wait":scalar(wait),"starts":scalar(1),
        "predecessor_wait":scalar(predecessor),"wait_causes":parts})
}
pub fn case(id: u64, work: u64, lead: u64, task: Value, breakdown: Value) -> Value {
    json!({"case_id":id,"work":scalar(work),"tasks":[task],
        "terminals":[{"end":"done","lead_time":scalar(lead),"breakdown":breakdown}]})
}
pub fn terminal(id: u64, arrival: u64, at: u64, work: u64) -> Value {
    json!({"case_id":id,"arrival":arrival,"terminals":[{"end":"done","kind":"end",
        "completed_at":point(at),"elapsed":point(at-arrival),"work":point(work)}]})
}
pub fn actor(work: u64, capacity_time: u64, ordinary: u64, overtime: u64) -> Value {
    json!({"actor":"Desk","work":scalar(work),"scheduled_capacity_time":scalar(capacity_time),
        "scheduled_occupancy_time":scalar(ordinary),"overtime_occupancy_time":scalar(overtime)})
}

pub fn expected(name: &str) -> Value {
    let z = || causes(0, 0, 0, 0, 0);
    let simple = |at: u64, wait: u64, service: u64| {
        json!({"verdict":"satisfied",
        "case_outcomes":[terminal(1,0,at,service)],"bounds":{"truncated":false},
        "metrics":{"cases":[case(1,service,at,task("r",service,wait,causes(0,0,0,0,wait),0),partition(service,0,0,wait,0))]}})
    };
    match name {
        "open" => {
            let mut v = simple(7, 5, 2);
            v["metrics"]["actors"] = json!([actor(2, 2, 2, 0)]);
            v["metrics"]["actors"][0]["queue"] = json!({"wait_total":{"stats":scalar(5)},"length_max":scalar(1),"length_time_average":{"stats":scalar("5/7")}});
            v
        }
        "open-later" => simple(8, 6, 2),
        "resource" | "resource-later" => {
            let late = name == "resource-later";
            let wait = if late { 5 } else { 4 };
            let mut v = simple(wait + 2, wait, 2);
            v["metrics"]["resources"] = json!([{"resource":"Terminal","occupancy_time":scalar(2),"scheduled_capacity_time":scalar(2),"scheduled_occupancy_time":scalar(2),"overtime_occupancy_time":scalar(0)}]);
            v
        }
        "continue" | "continue-earlier-close" | "explicit-rate" | "free-overtime" => {
            let early = name == "continue-earlier-close";
            let (a, c, regular, extra) = if early { (1, 3, 7, 3) } else { (2, 2, 8, 2) };
            let mut v = json!({"verdict":"satisfied","bounds":{"truncated":false},
                "case_outcomes":[terminal(1,0,5,5),terminal(2,1,11,5)],
                "metrics":{"cases":[
                    case(1,5,5,task("r",5,0,z(),0),partition(5,0,0,0,0)),
                    case(2,5,10,task("r",5,5,causes(a,0,0,c,1),0),partition(5,a,c,1,0))],
                "actors":[actor(10,regular,regular,extra)],"overall":{"work":scalar(10)}}});
            let cost = match name {
                "explicit-rate" => scalar(22),
                "free-overtime" => scalar(16),
                _ => absent("not_configured"),
            };
            v["metrics"]["overall"]["observed_cost_total"] = cost.clone();
            v["metrics"]["actors"][0]["occupancy_cost"] = cost;
            v["metrics"]["actors"][0]["utilization"] = scalar("1/2");
            v["metrics"]["actors"][0]["scheduled_utilization"] = scalar(1);
            v["metrics"]["actors"][0]["tariff_regular_work"] = scalar(regular);
            v["metrics"]["actors"][0]["tariff_overtime_work"] = scalar(extra);
            v
        }
        "drop" | "drop-zero" | "drop-zero-missing-rate" => {
            let zero = name != "drop";
            let mut v = json!({"verdict":"satisfied","bounds":{"truncated":false},
                "case_outcomes":[terminal(1,0,3,3),terminal(2,0,3,3)],
                "metrics":{"cases":[case(1,3,3,task("r",3,0,z(),0),partition(3,0,0,0,0)),case(2,3,3,task("r",3,0,z(),0),partition(3,0,0,0,0))],
                    "actors":[actor(6,if zero {2} else {4},if zero {2} else {4},if zero {4} else {2})]}});
            let cost = if name == "drop-zero-missing-rate" {
                absent("not_configured")
            } else {
                scalar(if zero { 16 } else { 12 })
            };
            v["metrics"]["overall"] = json!({"observed_cost_total":cost});
            v["metrics"]["actors"][0]["tariff_overtime_work"] = scalar(if zero { 4 } else { 0 });
            v["metrics"]["actors"][0]["tariff_regular_work"] = scalar(if zero { 2 } else { 6 });
            v
        }
        "shift" | "shift-later" => {
            let late = name == "shift-later";
            let (end, wait) = if late { (11, 3) } else { (8, 0) };
            json!({"verdict":"satisfied","case_outcomes":[terminal(1,0,5,5),terminal(2,3,end,5)],
                "metrics":{"cases":[case(1,5,5,task("r",5,0,z(),0),partition(5,0,0,0,0)),
                    case(2,5,end-3,task("r",5,wait,causes(0,0,0,if late {2} else {0},if late {1} else {0}),0),partition(5,0,if late {2} else {0},if late {1} else {0},0))],
                "actors":[actor(10,end*2,10,0)],
                "people":[{"person":"p","work":scalar(5),"peak_tasks":scalar(1),"overtime_work":scalar(2)},
                    {"person":"q","work":scalar(5),"peak_tasks":scalar(1),"overtime_work":scalar(0)}]}})
        }
        "condition" | "condition-later" | "condition-open" | "condition-open-later" => {
            let signal = if name == "condition-later" { 5 } else { 4 };
            let wait = match name {
                "condition-open" => 2,
                "condition-open-later" => 3,
                _ => 0,
            };
            let end = signal + wait + 2;
            json!({"verdict":"satisfied","bounds":{"truncated":false},
                "case_outcomes":[terminal(1,0,signal,signal),terminal(2,0,end,2)],
                "metrics":{"cases":[case(1,signal,signal,task("s",signal,0,z(),0),partition(signal,0,0,0,0)),
                    case(2,2,end,task("r",2,wait,causes(0,0,0,0,wait),signal),partition(2,0,0,wait,signal))]}})
        }
        "condition-true" => json!({"verdict":"satisfied","case_outcomes":[terminal(1,0,2,2)],
            "metrics":{"cases":[case(1,2,2,task("r",2,0,z(),0),partition(2,0,0,0,0))]}}),
        "condition-sim" => {
            json!({"verdict":"inconclusive","simulation":{"completed_replications":4},
            "case_outcomes":[{"case_id":1,"deadlocked_count":0,"unfinished_count":0,"terminals":[{"end":"done","count":4}]},
                {"case_id":2,"deadlocked_count":0,"unfinished_count":0,"terminals":[{"end":"done","count":4}]}],
            "metrics":{"cases":[{"case_id":1},{"case_id":2,"terminals":[{"end":"done","lead_time":sample("6","6","6"),
                "breakdown":{"condition":sample("4","4","4"),"unattributed":sample("0","0","0")}}],
                "tasks":[{"binding":"r","wait":sample("0","0","0"),"wait_causes":{"condition":sample("0","0","0")}}]}]}})
        }
        "condition-deadlock" => {
            json!({"verdict":"violated","case_outcomes":[{"case_id":1,"terminals":[]}],
            "metrics":{"cases":[{"case_id":1,"work":scalar(0),"terminals":[{"end":"done","lead_time":absent("no_terminal")}]}]}})
        }
        "condition-race" => {
            json!({"verdict":"violated","case_outcomes":[terminal(1,0,5,5),terminal(2,0,5,1)]})
        }
        "horizon" => json!({"verdict":"inconclusive","bounds":{"truncated":false},
            "case_outcomes":[{"case_id":1,"arrival":0,"terminals":[{"kind":"cut_off","end":null,"completed_at":null,"elapsed":null,"work":point(1)}]}],
            "metrics":{"cases":[{"case_id":1,"dwell":{"until":scalar(6),"bound":"lower","breakdown":partition(1,0,0,5,0)}}]}}),
        "clock" => json!({"verdict":"inconclusive","bounds":{"truncated":true},
            "case_outcomes":[{"case_id":1,"terminals":[]}],"metrics":{"actors":[{"actor":"Desk","scheduled_capacity_time":absent("truncated"),"overtime_occupancy_time":absent("truncated")}]} }),
        "ack" | "ack-later" => {
            let later = name == "ack-later";
            let mut v = json!({"verdict":"satisfied","case_outcomes":[{"case_id":1,"arrival":0,"terminals":[{"end":"done","completed_at":range(2,if later {3} else {2}),"elapsed":range(2,if later {3} else {2}),"work":range(3,if later {4} else {3})}]}],
                "metrics":{"actors":[{"actor":"B","scheduled_capacity_time":scalar(1),"scheduled_occupancy_time":scalar(1),"overtime_occupancy_time":if later {span("1","2")} else {scalar(1)}}]}});
            v["metrics"]["cases"] = json!([{"case_id":1,"tasks":[{"binding":"a","work":scalar(1)}, {"binding":"b","work":if later {span("2","3")} else {scalar(2)}}]}]);
            v
        }
        "arrivals" | "arrivals-burst" => {
            let burst = name == "arrivals-burst";
            let arrival = if burst {
                sample("1/2", "0", "1")
            } else {
                sample("2", "0", "4")
            };
            let lead = if burst {
                sample("11/2", "5", "6")
            } else {
                sample("9/2", "3", "6")
            };
            let wait = if burst {
                sample("5/2", "2", "3")
            } else {
                sample("3/2", "0", "3")
            };
            json!({"verdict":"satisfied","mode":"simulation","bounds":{"truncated":false},
                "simulation":{"attempted_replications":4,"completed_replications":4,"truncated_replications":0},
                "case_outcomes":[{"case_id":1,"arrival":null,"arrival_time":sample("0","0","0"),"unfinished_count":0,"deadlocked_count":0},
                    {"case_id":2,"arrival":null,"arrival_time":arrival,"terminals":[{"end":"done","count":4}],"unfinished_count":0,"deadlocked_count":0}],
                "properties":[{"id":"probability reached #1","cases":[
                    {"case_id":1,"arrival":null,"arrival_time":sample("0","0","0")},
                    {"case_id":2,"arrival":null,"arrival_time":arrival,"hits":if burst {0} else {2},"unknown":0,"trials":4}]}],
                "metrics":{"overall":{"counts":{"offered":{"stats":sample("2","2","2"),"observed_total":8},
                    "received":{"stats":sample("2","2","2"),"observed_total":8},
                    "completed":{"stats":sample("2","2","2"),"observed_total":8}}},
                    "cases":[{"case_id":1,"arrival":null,"arrival_time":sample("0","0","0")},
                    {"case_id":2,"arrival":null,"arrival_time":arrival,"work":sample("3","3","3"),
                    "terminals":[{"end":"done","lead_time":lead}],"tasks":[{"binding":"r","wait":wait,"work":sample("3","3","3")}]}]}})
        }
        "arrivals-calendar" => {
            let mut v = expected("arrivals");
            v["properties"][0]["cases"][1]["hits"] = json!(0);
            v["metrics"]["cases"][1]["terminals"][0]["lead_time"] = sample("9", "7", "11");
            v["metrics"]["cases"][1]["tasks"][0]["wait"] = sample("6", "4", "8");
            v["metrics"]["cases"][1]["tasks"][0]["wait_causes"] = json!({"calendar":sample("3","1","5"),"actor_capacity":sample("3","3","3"),"condition":sample("0","0","0")});
            v
        }
        "fixed-burst" | "fixed-spaced" => {
            let spaced = name == "fixed-spaced";
            json!({"verdict":"satisfied","case_outcomes":[terminal(1,0,3,3),terminal(2,if spaced {4} else {0},if spaced {7} else {6},3)],
                "metrics":{"cases":[{"case_id":2,"arrival":if spaced {4} else {0},"tasks":[{"binding":"r","wait":scalar(if spaced {0} else {3})}],"terminals":[{"end":"done","lead_time":scalar(if spaced {3} else {6})}]}]}})
        }
        "retry" | "retry-later" => json!({"verdict":"satisfied","bounds":{"truncated":false},
            "case_outcomes":[{"case_id":1,"terminals":[{"end":"duplicate","completed_at":point(if name=="retry" {5} else {6}),"work":point(2)}]}],
            "response_receipts":[{"attempt":1,"receipt":"unknown","at":point(1)},
                {"attempt":2,"receipt":"delivered","at":point(if name=="retry" {5} else {6})}]}),
        _ => panic!("no runtime hand oracle {name}"),
    }
}

pub fn validate(name: &str, v: &Value) -> Result<(), String> {
    if name == "condition-race-sim" {
        return diagnostic(v, "U-SEM-0", "unresolved");
    }
    let want = expected(name);
    subset(&want, v, name)?;
    if let Some(cases) = want["case_outcomes"].as_array() {
        let actual = v["case_outcomes"]
            .as_array()
            .ok_or("case outcomes required")?;
        if actual.len() != cases.len() {
            return Err(format!("{name}: closed case inventory"));
        }
        if let Some(metrics) = v.pointer("/metrics/cases").and_then(Value::as_array)
            && metrics.len() != cases.len()
        {
            return Err(format!("{name}: metric cohort differs"));
        }
    }
    if name.starts_with("retry") {
        let rows = v["properties"]
            .as_array()
            .ok_or("retry properties required")?;
        let once: Vec<_> = rows
            .iter()
            .filter(|p| {
                p["id"]
                    .as_str()
                    .is_some_and(|s| s.ends_with(" at_most_once"))
            })
            .collect();
        if once.len() != 1 || once[0]["verdict"] != "satisfied" {
            return Err("retry must retain exactly one satisfied once contract".into());
        }
    }
    if !matches!(
        name,
        "clock" | "horizon" | "condition-deadlock" | "condition-race"
    ) {
        for d in v["diagnostics"].as_array().into_iter().flatten() {
            if d["code"]
                .as_str()
                .is_some_and(|c| c.starts_with("E-") || c.starts_with("U-") || c == "W-DEADLOCK-0")
            {
                return Err(format!("{name}: unexpected refusal/deadlock {d}"));
            }
        }
    }
    if matches!(name, "condition-deadlock" | "condition-race") {
        let ds = v["diagnostics"].as_array().ok_or("diagnostics required")?;
        if !ds.iter().any(|d| d["code"] == "W-DEADLOCK-0") {
            return Err("missing genuine deadlock".into());
        }
    }
    if let Some(cases) = v.pointer("/metrics/cases").and_then(Value::as_array) {
        for c in cases {
            for terminal in c["terminals"].as_array().into_iter().flatten() {
                if let Some(lead) = terminal["lead_time"]["min"]
                    .as_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    && let Some(parts) = terminal["breakdown"].as_object()
                {
                    let mut total = Some(0u64);
                    for (key, value) in parts {
                        let quantities: Vec<_> = if key == "queue" {
                            value.as_object().unwrap().values().collect()
                        } else {
                            vec![value]
                        };
                        for q in quantities {
                            total = total.and_then(|n| {
                                q["min"].as_str()?.parse::<u64>().ok()?.checked_add(n)
                            });
                        }
                    }
                    if total != Some(lead) {
                        return Err(format!("{name}: case partition sum != lead time"));
                    }
                    let zero = if v["mode"] == "simulation" {
                        let n = v["simulation"]["replications"]
                            .as_u64()
                            .ok_or("simulation replication count required")?;
                        json!({"mean":"0","min":"0","max":"0","samples":n,"reason":null})
                    } else {
                        scalar(0)
                    };
                    if terminal["breakdown"]["unattributed"] != zero {
                        return Err(format!("{name}: unattributed must be zero"));
                    }
                }
            }
            if let Some(tasks) = c["tasks"].as_array() {
                for t in tasks {
                    if let Some(parts) = t["wait_causes"].as_object()
                        && let Some(wait) = t["wait"]["min"]
                            .as_str()
                            .and_then(|s| s.parse::<u64>().ok())
                    {
                        let sum: Option<u64> = parts
                            .values()
                            .map(|x| x["min"].as_str()?.parse::<u64>().ok())
                            .sum();
                        if sum != Some(wait) {
                            return Err(format!("{name}: Task cause sum != wait"));
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn diagnostic(v: &Value, code: &str, reason: &str) -> Result<(), String> {
    let expected = if code.starts_with("E-") {
        "invalid"
    } else {
        "unsupported"
    };
    if v["verdict"] != expected {
        return Err(format!("expected {expected}/{code}, got {}", v["verdict"]));
    }
    let rows = v["diagnostics"].as_array().ok_or("required diagnostics")?;
    if !rows
        .iter()
        .any(|d| d["code"] == code && d["message"].as_str().is_some_and(|m| m.contains(reason)))
    {
        return Err(format!("required {code}/{reason}; observed {rows:?}"));
    }
    Ok(())
}

/// Independent assembled witness: validates this oracle, not a production decoder.
pub fn mutation_controls() -> Vec<(&'static str, String, Value)> {
    let mut overcommit = expected("drop");
    overcommit["verdict"] = json!("violated");
    let mut free = expected("continue");
    free["metrics"]["overall"]["observed_cost_total"] = scalar(16);
    let mut arrival = expected("arrivals");
    arrival["case_outcomes"][1]["arrival"] = json!(0);
    let mut property_arrival = expected("arrivals");
    property_arrival["properties"][0]["cases"][1]["arrival"] = json!(0);
    let mut property_sample = expected("arrivals");
    property_sample["properties"][0]["cases"][1]
        .as_object_mut()
        .unwrap()
        .remove("arrival_time");
    let mut property_null = expected("arrivals");
    property_null["properties"][0]["cases"][0]
        .as_object_mut()
        .unwrap()
        .remove("arrival");
    let mut property_wrong_sample = expected("arrivals");
    property_wrong_sample["properties"][0]["cases"][1]["arrival_time"] = sample("0", "0", "0");
    let mut intentional = expected("open");
    intentional["metrics"]["cases"][0]["terminals"][0]["breakdown"]["intentional"] = scalar(5);
    intentional["metrics"]["cases"][0]["terminals"][0]["breakdown"]["queue"]["calendar"] =
        scalar(0);
    let mut condition = expected("condition-open");
    condition["metrics"]["cases"][1]["tasks"][0]["wait"] = scalar(6);
    vec![
        (
            "drop",
            "schedule decrease became overcommit".into(),
            overcommit,
        ),
        ("continue", "unset overtime rate became zero".into(), free),
        ("arrivals", "first sampled arrival leaked".into(), arrival),
        (
            "arrivals",
            "first support arrival leaked into property".into(),
            property_arrival,
        ),
        (
            "arrivals",
            "property arrival sample missing".into(),
            property_sample,
        ),
        (
            "arrivals",
            "property arrival null key missing".into(),
            property_null,
        ),
        (
            "arrivals",
            "property sample replaced by first support".into(),
            property_wrong_sample,
        ),
        ("open", "calendar became intentional".into(), intentional),
        (
            "condition-open",
            "condition double-counted in Task wait".into(),
            condition,
        ),
    ]
}
