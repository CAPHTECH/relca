//! Additional R1 acceptance, after the fixed 76: command/attempt, not slot IDs.
//! Hand timeline per command: post[0,1), backoff[1,4), duplicate[4,5).
//! Two commands r/r2 on the same Ledger.post site-name have four real starts,
//! work4, lead10, intentional6. Case2 arrives20, so it ends30, not a new key
//! namespace. The unreachable `skipped` call shifts slots but starts zero times.
use relca_core::{compare, diff, lower, report, syntax};
use serde_json::{Value, json};

fn source(cases: usize, declared_compare: bool) -> String {
    let command = |name: &str| {
        format!(
            "{name}=Ledger.post(case.id) response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 3min exhausted end failed;"
        )
    };
    let body = format!("{}{}end done", command("r"), command("r2"));
    let arrivals = if cases == 2 { "[0min,20min]" } else { "[0min]" };
    let declaration = if declared_compare {
        "compare [From,To] cut C objectives { minimize metrics.overall.work }"
    } else {
        ""
    };
    format!(
        r#"model StableTaskIdentity version "0.2" {{
      type Ack=enum {{committed,duplicate}}
      observed case {{flag:bool=false}}
      actor Ledger {{capacity 1;state keys:set<CaseId>
        capability post(key:CaseId)->Ack {{commits;idempotency key key;writes keys;duration fixed 1min;
          atomic {{if key in keys {{result duplicate}} else {{keys += key;result committed}}}}
          cancel forbidden_after_start}}}}
      process Before {{{body}}}
      process After {{if flag == true {{skipped=Ledger.post(case.id)}} else {{}};{body}}}
      cut C {{include [Ledger]}}
      configuration From {{cut C;process Before}}
      configuration To {{cut C;process After}}
      arrivals fixed {arrivals}
      metrics {{window 40min;currency "JPY";rate Ledger per_minute 1;rate Ledger.post per_start 2;breakdown waits}}
      bounds {{cases {cases};live_tasks 2;clock 40min}}
      simulation replications 4 seed 7 confidence 0.5
      verify safety no_resource_overcommit
      {declaration}
    }}"#
    )
}

fn task_cells<'a>(
    rows: &'a Value,
    case: usize,
    command: &str,
    attempt: usize,
    cmp: bool,
) -> Vec<&'a Value> {
    let subject = format!("response:{command}:attempt{attempt}");
    let matches: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| {
            r["key"]["scope"] == "task"
                && r["key"]["case_id"] == case
                && r["key"]["subject"] == subject
                && r["key"]["metric"] == "starts"
        })
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "canonical Task row case={case} subject={subject}"
    );
    let row = matches[0];
    if cmp {
        row["values"].as_array().unwrap().iter().collect()
    } else {
        vec![&row["from"], &row["to"]]
    }
}

fn task_inventory(rows: &Value, cases: usize, cmp: bool) {
    for case in 1..=cases {
        for command in ["r", "r2"] {
            for attempt in 1..=2 {
                for cell in task_cells(rows, case, command, attempt, cmp) {
                    assert_eq!(cell["status"], "reported");
                    assert_eq!(cell["value"]["min"], "1");
                    assert_eq!(cell["value"]["max"], "1");
                }
            }
        }
    }
    let starts: Vec<_> = rows
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["key"]["scope"] == "task" && r["key"]["metric"] == "starts")
        .collect();
    assert_eq!(
        starts.len(),
        cases * 5,
        "r/r2 x two attempts plus the distinct skipped site per case"
    );
    assert!(starts.iter().all(|r| {
        !r["key"]["subject"]
            .as_str()
            .unwrap()
            .starts_with("@response:")
    }));
    for row in starts.iter().filter(|r| r["key"]["subject"] == "skipped") {
        let cells: Vec<_> = if cmp {
            row["values"].as_array().unwrap().iter().collect()
        } else {
            vec![&row["from"], &row["to"]]
        };
        assert_eq!(cells[0]["status"], "not_applicable");
        assert_eq!(cells[1]["status"], "reported");
        assert_eq!(cells[1]["value"]["min"], "0");
    }
}

fn stable_effects(differences: &Value) {
    assert_eq!(differences["status"], "derived");
    let rows = differences["rows"].as_array().unwrap();
    let unknown: Vec<_> = rows
        .iter()
        .filter(|r| r["viewpoint"] == "effects" && r["item"] == "u")
        .collect();
    assert_eq!(
        unknown.len(),
        5,
        "four command/attempt sites, plus the distinct skipped effect"
    );
    for command in ["r", "r2"] {
        for attempt in 1..=2 {
            let name = format!("response:{command}:attempt{attempt}");
            let r = unknown
                .iter()
                .find(|r| r["call"] == name)
                .expect("one matched effect site");
            assert_eq!(r["status"], "not_comparable");
            for owner in ["from", "to"] {
                assert_eq!(r[owner]["status"], "not_derived");
                assert_eq!(r[owner]["reason"], "response_control");
            }
        }
    }
    let changed_e: Vec<_> = rows
        .iter()
        .filter(|r| r["viewpoint"] == "effects" && r["item"] == "e")
        .collect();
    assert_eq!(
        changed_e.len(),
        1,
        "same original effect sites must not be false added/removed effects"
    );
    assert_eq!(changed_e[0]["call"], "skipped");
}

#[test]
fn standalone_children_have_four_real_starts_and_six_intentional_minutes_per_case() {
    for cases in [1, 2] {
        let s = source(cases, false);
        for name in ["From", "To"] {
            for sample in [false, true] {
                let value = if sample {
                    json!(report::simulate_with_selection(&s, Some(name), None))
                } else {
                    json!(report::check_configuration(&s, name))
                };
                assert_eq!(
                    value["verdict"],
                    if sample { "inconclusive" } else { "satisfied" },
                    "{}",
                    value["diagnostics"]
                );
                assert_eq!(
                    value["metrics"]["overall"]["work"]["min"],
                    (cases * 4).to_string()
                );
                for case in value["metrics"]["cases"].as_array().unwrap() {
                    let done = case["terminals"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .find(|t| t["end"] == "done")
                        .unwrap();
                    assert_eq!(done["lead_time"]["min"], "10");
                    assert_eq!(done["breakdown"]["in_service"]["min"], "4");
                    assert_eq!(done["breakdown"]["intentional"]["min"], "6");
                }
            }
        }
    }
}

#[test]
fn common_random_lists_actual_command_attempt_kernel_and_duration_key_names() {
    // All mass is at one minute/pass. N2 is a potential, not executed, draw.
    // The expected strings follow the pre-existing SHA256 task-site contract,
    // not a new RNG or a guess from internal slot numbers.
    let call = |name: &str| {
        format!(
            "{name}=A.run() response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed;"
        )
    };
    let body = format!("{}{}end done", call("r"), call("r2"));
    let s = format!(
        r#"model KeyNames version "0.2" {{type V=enum {{pass,fail}}
      observed case {{flag:bool=false}}
      kernel K {{ * -> {{1:pass}} }}
      actor A {{capacity 1;capability run()->V {{duration {{1min:1}};outcomes kernel K;cancel forbidden_after_start}}}}
      process Before {{{body}}}
      process After {{if flag == true {{skipped=A.run()}} else {{}};{body}}}
      cut C {{include [A]}}
      configuration From {{cut C;process Before}}
      configuration To {{cut C;process After}}
      bounds {{cases 1;live_tasks 2;clock 10min}}
      simulation replications 4 seed 7 confidence 0.5
      verify safety no_resource_overcommit
    }}"#
    );
    let value = json!(compare::compare(
        &s,
        &["From", "To"],
        compare::Analysis::Simulation
    ));
    assert_eq!(value["verdict"], "inconclusive", "{}", value["diagnostics"]);
    for (index, process) in ["Before", "After"].iter().enumerate() {
        for kind in ["kernel", "duration"] {
            let mut expected = Vec::new();
            if index == 1 {
                expected.push(format!("{kind}.{process}.skipped"));
            }
            for command in ["r", "r2"] {
                for attempt in 1..=2 {
                    expected.push(format!("{kind}.{process}.{command}.attempt{attempt}"));
                }
            }
            assert_eq!(
                value["common_random"]["independent"][index][format!("{kind}_sites")],
                json!(expected)
            );
        }
    }
}

#[test]
fn compare_matches_dead_call_shift_by_case_command_and_attempt_without_changing_children() {
    let s = source(2, false);
    for mode in [compare::Analysis::Exhaustive, compare::Analysis::Simulation] {
        let expected: Vec<_> = ["From", "To"]
            .iter()
            .map(|name| match mode {
                compare::Analysis::Exhaustive => {
                    json!(report::check_configuration(&s, name).metrics)
                }
                compare::Analysis::Simulation => {
                    json!(report::simulate_with_selection(&s, Some(name), None).metrics)
                }
            })
            .collect();
        let value = json!(compare::compare(&s, &["From", "To"], mode));
        assert_eq!(
            value["verdict"],
            if mode == compare::Analysis::Exhaustive {
                "satisfied"
            } else {
                "inconclusive"
            },
            "root={} children={}",
            value["diagnostics"],
            json!([
                value["plans"][0]["report"]["diagnostics"],
                value["plans"][1]["report"]["diagnostics"]
            ])
        );
        for (i, expected) in expected.iter().enumerate() {
            assert_eq!(
                &value["plans"][i]["report"]["metrics"], expected,
                "raw child metrics are not normalized in place"
            );
        }
        assert_ne!(
            value["plans"][0]["report"]["metrics"]["cases"][0]["tasks"][0]["binding"],
            value["plans"][1]["report"]["metrics"]["cases"][0]["tasks"][1]["binding"]
        );
        task_inventory(&value["tables"]["metrics"], 2, true);
        assert!(value["tables"].get("waiting").is_none());
    }
}

#[test]
fn diff_matches_task_and_partial_agency_sites_without_changing_raw_children() {
    let s = source(1, false);
    let from = report::check_configuration(&s, "From");
    let to = report::check_configuration(&s, "To");
    let value = json!(diff::diff(&s, "From", "To"));
    assert_eq!(value["verdict"], "satisfied", "{}", value["diagnostics"]);
    assert_eq!(value["from"]["report"]["metrics"], json!(from.metrics));
    assert_eq!(value["to"]["report"]["metrics"], json!(to.metrics));
    assert_eq!(value["from"]["report"]["agency"], json!(from.agency));
    assert_eq!(value["to"]["report"]["agency"], json!(to.agency));
    task_inventory(&value["metrics"], 1, false);
    stable_effects(&value["agency"]["differences"]);
    let typed: diff::DiffReport = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(json!(typed), value);
}

#[test]
fn declared_compare_agency_pairs_do_not_split_renumbered_effect_sites() {
    let s = source(1, true);
    let value = json!(compare::compare(
        &s,
        &["From", "To"],
        compare::Analysis::Exhaustive
    ));
    assert_eq!(value["verdict"], "satisfied", "{}", value["diagnostics"]);
    stable_effects(&value["tables"]["agency"][0]["differences"]);
}

#[test]
fn genuinely_duplicate_command_names_remain_static_errors_not_coalesced_sites() {
    let s = source(1, false).replace("r2=Ledger.post", "r=Ledger.post");
    let ast = syntax::parse(&s).unwrap();
    for name in ["From", "To"] {
        let diagnostics = lower::lower_configuration(&ast, name)
            .err()
            .expect("duplicate binding must be rejected");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.message.contains("duplicate or shadowed binding"))
        );
    }
}
