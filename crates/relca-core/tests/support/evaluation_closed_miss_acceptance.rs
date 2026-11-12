//! Hand oracle: ../evaluation_closed_miss.md. Shared by core and real CLI.
#[path = "quantile_review_inputs.rs"]
mod input;
use serde_json::json;

fn source(desk: bool, positive: bool, quantiles: bool) -> String {
    let mut s = if desk { input::DESK } else { input::CLOSED }.to_owned();
    if positive {
        s = if desk {
            input::edit(
                &s,
                "reached(Desk.flag == true)",
                "reached(Desk.seen == true)",
            )
        } else {
            input::edit(&s, "Ledger.n == 2", "Ledger.n == 1")
        };
    }
    s = input::edit(&s, "replications 1", "replications 4");
    if !quantiles {
        let lines: Vec<_> = s
            .lines()
            .filter(|l| !l.trim_start().starts_with("metric quantile."))
            .collect();
        assert_eq!(lines.len() + 2, s.lines().count());
        s = lines.join("\n");
    }
    let end = s.rfind('}').unwrap();
    let property = if desk { "Flag" } else { "Two" };
    // Simulation compare requires distinct process names for operation keys.
    // Duplicate the existing body without changing the execution topology.
    let body = &s[s.find("  process P").unwrap()..s.find("  cut C").unwrap()];
    let second_process = input::edit(body, "process P", "process Q");
    s.insert_str(
        end,
        &format!(
            "\n{second_process}\nconfiguration Left {{ cut C; process P }}\nconfiguration Right {{ cut C; process Q }}\nmetric cohort.Rate = expected(count(case.{property})) / arrivals\ncompare [Left,Right] cut C hard {{ cohort.Rate <= 0.75 }} objectives {{ minimize cohort.Rate }}\n"
        ),
    );
    s
}

fn check(desk: bool, positive: bool, sampled: bool) {
    let (verdict, status) = if !positive {
        ("satisfied", "admissible")
    } else if sampled {
        ("inconclusive", "undetermined")
    } else {
        ("violated", "excluded")
    };
    let point = if positive { "1" } else { "1/2" };
    let (lo, hi) = if sampled {
        if positive {
            ("1/2", "1")
        } else {
            ("1/4", "3/4")
        }
    } else {
        (point, point)
    };
    for quantiles in [true, false] {
        let s = source(desk, positive, quantiles);
        for plans in [["Left", "Right"], ["Right", "Left"]] {
            let s = if plans[0] == "Right" {
                input::edit(&s, "compare [Left,Right]", "compare [Right,Left]")
            } else {
                s.clone()
            };
            let v = super::run(&s, &plans, sampled);
            let evaluation = &v["evaluation"];
            assert!(
                evaluation.is_object(),
                "compare must evaluate: {}",
                v["diagnostics"]
            );
            assert_eq!(evaluation["hard"].as_array().unwrap().len(), 1);
            let hard = &evaluation["hard"][0];
            assert_eq!(hard["constraint"], "cohort.Rate <= 3/4");
            assert_eq!(hard["cohort"], "Rate");
            assert_eq!(hard["op"], "<=");
            assert_eq!(hard["bound"], "3/4");
            let cells = hard["values"].as_array().unwrap();
            assert_eq!(cells.len(), 2);
            for (c, plan) in cells.iter().zip(plans) {
                // First semantic assertion is the actual hard verdict. A fault
                // must fail here, not merely in an upstream B6/quantile row.
                assert_eq!(c["verdict"], verdict, "D06 hard verdict: {c}");
                let mut expected = json!({
                    "configuration":plan,"verdict":verdict,
                    "basis":if sampled {"statistical"} else {"exact"},
                    "interval":{"lower":lo,"upper":hi},
                    "reason":null,"source_reason":null
                });
                if sampled {
                    expected["point"] = json!(point);
                }
                assert_eq!(*c, expected, "D06 hard endpoints and provenance");
            }
            assert_eq!(v["diagnostics"], json!([]));
            // Existing implicit no_lost_update (and Ledger range) safety
            // checks stay inconclusive under sampling, independently of hard.
            let verify = if sampled { "inconclusive" } else { "satisfied" };
            assert_eq!(v["verdict"], if sampled { "inconclusive" } else { verdict });
            for (p, plan) in evaluation["plans"].as_array().unwrap().iter().zip(plans) {
                assert_eq!(p["configuration"], plan);
                assert_eq!(p["verify_verdict"], verify);
                assert_eq!(p["hard_verdict"], verdict);
                assert_eq!(p["status"], if sampled { "undetermined" } else { status });
            }
            assert_eq!(evaluation["objectives"].as_array().unwrap().len(), 1);
            let objective = &evaluation["objectives"][0];
            assert_eq!(objective["objective"], "minimize cohort.Rate");
            assert_eq!(objective["direction"], "minimize");
            let values = objective["values"].as_array().unwrap();
            assert_eq!(values.len(), 2);
            for (c, plan) in values.iter().zip(plans) {
                assert_eq!(
                    *c,
                    if sampled {
                        json!({"configuration":plan,"point":point,"reason":null})
                    } else {
                        json!({"configuration":plan,"lower":point,"upper":point,"basis":"exact","reason":null})
                    },
                    "objective transports cohort value, not statistical hard interval"
                );
            }
            if sampled {
                assert_eq!(
                    evaluation["statistics"],
                    json!({
                        "interval_method":"clopper-pearson-per-case-sum",
                        "multiplicity":{"method":"bonferroni","m":4,"alpha":"1/16"},
                        "confidence":"1/2"
                    })
                );
            } else {
                assert!(evaluation.get("statistics").is_none());
            }
            for p in v["plans"].as_array().unwrap() {
                let q = &p["report"]["quality"];
                assert_eq!(q["properties"].as_array().unwrap().len(), 1);
                let arrival = if desk { 5 } else { 2 };
                let cases = if sampled {
                    json!([
                        {"case_id":1,"arrival":0,"hits":if positive {4} else {0},"unknown":0,"trials":4},
                        {"case_id":2,"arrival":arrival,"hits":4,"unknown":0,"trials":4}
                    ])
                } else {
                    json!([
                        {"case_id":1,"arrival":0,"min":if positive {"1"} else {"0"},"max":if positive {"1"} else {"0"},"reason":null},
                        {"case_id":2,"arrival":arrival,"min":"1","max":"1","reason":null}
                    ])
                };
                assert_eq!(q["properties"][0]["cases"], cases);
                assert_eq!(q["cohort"].as_array().unwrap().len(), 1);
                assert_eq!(q["cohort"][0]["name"], "Rate");
                let count = if positive { "2" } else { "1" };
                assert_eq!(
                    q["cohort"][0]["value"],
                    if sampled {
                        json!({"point":point,"reason":null,"samples":4,
                            "numerator":{"mean":count,"min":count,"max":count,"samples":4,"reason":null},
                            "denominator":{"mean":"2","min":"2","max":"2","samples":4,"reason":null}})
                    } else {
                        json!({"lower":point,"upper":point,"basis":"exact","reason":null})
                    }
                );
            }
        }
    }
}

macro_rules! acceptance {
    ($name:ident, $desk:expr, $positive:expr, $sampled:expr) => {
        #[test]
        fn $name() {
            check($desk, $positive, $sampled);
        }
    };
}
acceptance!(d06_ledger_closed_miss_exhaustive, false, false, false);
acceptance!(d06_ledger_closed_miss_simulation, false, false, true);
acceptance!(d06_ledger_positive_exhaustive, false, true, false);
acceptance!(d06_ledger_positive_simulation, false, true, true);
acceptance!(d06_desk_closed_miss_exhaustive, true, false, false);
acceptance!(d06_desk_closed_miss_simulation, true, false, true);
acceptance!(d06_desk_positive_exhaustive, true, true, false);
acceptance!(d06_desk_positive_simulation, true, true, true);
