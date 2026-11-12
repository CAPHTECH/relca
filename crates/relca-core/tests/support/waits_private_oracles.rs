//! Internal-seam acceptance. Invoke
//! `run` separately from engine and reference unit tests, adapting these plain
//! histories to each collector/reducer without sharing a cause calculator.
//! These assertions are NOT a standalone implementation of waiting semantics.
#![allow(dead_code)]
use serde_json::{Value, json};

pub struct Probe {
    pub name: &'static str,
    pub input: Value,
    pub expected: Value,
}
fn s(x: &str) -> Value {
    json!({"min":x,"max":x,"reason":null})
}
fn range(a: &str, b: &str) -> Value {
    json!({"min":a,"max":b,"reason":null})
}
fn sample(mean: &str, min: &str, max: &str) -> Value {
    json!({"mean":mean,"min":min,"max":max,"samples":2,"reason":null})
}
fn n(reason: &str) -> Value {
    json!({"min":null,"max":null,"reason":reason})
}

/// Outputs use a small normalized observation view, not a proposed public DTO.
/// The adapter selects these named quantities from the ACTUAL collector result.
pub fn probes() -> Vec<Probe> {
    let mut out = Vec::new();
    for (ready, start, unattributed, service) in [(2, 2, "2", "1"), (1, 1, "1", "2")] {
        out.push(Probe {
            name: "P01_gap",
            input: json!({"kind":"history","arrival":0,"admitted":0,"end":3,"T":3,
            "tasks":[{"binding":"a","actor":"A","ready":ready,"start":start,"stop":3}],"waits":[]}),
            expected: json!({"unattributed":s(unattributed),"in_service":s(service)}),
        });
    }
    for (stop, total, avg) in [(2, "2", "2/5"), (3, "3", "3/5")] {
        out.push(Probe { name:"P02_cancelled_before_start",input:json!({"kind":"history","arrival":0,"admitted":0,"T":5,
            "tasks":[{"binding":"a","actor":"A","ready":0,"start":null,"stop":stop,"status":"cancelled_before_start"}]}),
            expected:json!({"wait_total":s(total),"length_max":s("1"),"length_time_average":s(avg),
                "wait_causes":{"actor_capacity":n("cancelled_before_start"),"resource":n("cancelled_before_start"),
                    "executor":n("cancelled_before_start"),"combined":n("cancelled_before_start"),"unattributed":n("cancelled_before_start")}}) });
    }
    for (start, actor, unknown) in [(2, "0", "2"), (1, "1", "1")] {
        out.push(Probe {
            name: "P03_zero_measure_and_half_open",
            input: json!({"kind":"history","T":3,"capacity":1,
            "tasks":[{"binding":"blocker","actor":"A","ready":start,"start":start,"stop":2},
                {"binding":"target","actor":"A","ready":1,"start":3,"stop":4}]}),
            expected: json!({"target_actor_capacity":s(actor),"target_unattributed":s(unknown)}),
        });
    }
    let correlation = json!([
        {"A_total":0,"B_total":3,"overall":3,"T":8},
        {"A_total":5,"B_total":0,"overall":5,"T":8}]);
    out.push(Probe {name:"P04_correlated_leaf_extrema",input:json!({"kind":"exhaustive_reduce","records":correlation}),
        expected:json!({"A_wait_total":range("0","5"),"B_wait_total":range("0","3"),"overall":range("3","5")})});
    let mixed = json!([{"end":3,"cut_off":false,"service":3},{"end":null,"cut_off":true,"T":3,"service":3}]);
    out.push(Probe {
        name: "P05_dwell_late_presence",
        input: json!({"kind":"exhaustive_reduce","records":mixed}),
        expected: json!({"dwell":{"until":s("3"),"in_service":s("3"),"bound":"lower"}}),
    });
    out.push(Probe{name:"P05_no_cutoff_control",input:json!({"kind":"exhaustive_reduce","records":[{"end":3,"cut_off":false,"service":3},{"end":3,"cut_off":false,"service":3}]}),
        expected:json!({"dwell":null})});
    out.push(Probe {
        name: "P05_sample_conditional_population",
        input: json!({"kind":"sample_reduce","records":mixed}),
        expected: json!({"dwell":{"until":{"mean":"3","min":"3","max":"3","samples":1,"reason":null},
            "in_service":{"mean":"3","min":"3","max":"3","samples":1,"reason":null},"bound":"lower"}}),
    });
    for stopped in [false, true] {
        let mut total = json!({"stats":range("2","3")});
        let mut average = json!({"stats":range("1/2","1")});
        if stopped {
            total["bound"] = json!("lower");
            average["bound"] = json!("lower");
        }
        out.push(Probe{name:"P06_actual_horizon_OR_not_declaration",input:json!({"kind":"exhaustive_reduce","horizon":10,
            "records":[{"total":2,"T":4,"horizon_stopped":false},{"total":3,"T":3,"horizon_stopped":stopped}]}),
            expected:json!({"wait_total":total,"length_time_average":average,"length_max_has_bound":false})});
    }
    for (total, mean, max) in [(6, "5/4", "2"), (3, "3/4", "1")] {
        out.push(Probe{name:"P07_mean_of_ratios_not_ratio_of_sums",input:json!({"kind":"sample_reduce","records":[{"total":2,"T":4},{"total":total,"T":3}]}),
            expected:json!({"length_time_average":sample(mean,"1/2",max)})});
    }
    out.push(Probe{name:"P08_global_invalidation_removes_dwell_and_lower",input:json!({"kind":"exhaustive_reduce","coverage":"truncated","records":mixed}),
        expected:json!({"terminal_in_service":n("truncated"),"wait_total":{"stats":n("truncated")},"dwell":null,
            "join_wait":n("truncated"),"predecessor_wait":n("truncated"),"overall":n("truncated")})});
    out
}

pub fn run(mut observe: impl FnMut(&str, &Value) -> Value) {
    for probe in probes() {
        let observed = observe(probe.name, &probe.input);
        assert_eq!(
            observed, probe.expected,
            "{}: collector/reducer hand oracle",
            probe.name
        );
        if let Some(records) = probe.input["records"].as_array() {
            let mut reversed = probe.input.clone();
            reversed["records"] = Value::Array(records.iter().rev().cloned().collect());
            assert_eq!(
                observe(probe.name, &reversed),
                probe.expected,
                "{}: template/input order must not matter",
                probe.name
            );
        }
    }
}

/// Review additions deliberately use a separate callback: the existing P01-P08
/// adapters only understand two Tasks in one case. Do not truncate these rows.
/// See waits_review_calc.md for interval arithmetic and the linkage obligation.
pub fn review_probes() -> Vec<Probe> {
    let mut out = vec![Probe {
        name: "P09_mixed_known_unknown",
        input: json!({"kind":"review_history","T":4,
            "actors":[{"name":"A","capacity":1},{"name":"B","capacity":1}],
            "cases":[
                {"arrival":0,"admitted":0,"end":4,"tasks":[
                    {"binding":"x","actor":"A","ready":1,"start":3,"stop":4},
                    {"binding":"y","actor":"B","ready":1,"start":3,"stop":4}]},
                {"arrival":0,"admitted":0,"end":3,"tasks":[
                    {"binding":"blocker","actor":"A","ready":0,"start":0,"stop":3}]}]}),
        expected: json!({"case_combined":s("2"),"case_unattributed":s("1"),"case_in_service":s("1"),
            "x_actor_capacity":s("2"),"x_unattributed":s("0"),"y_actor_capacity":s("0"),"y_unattributed":s("2")}),
    }];
    for seam in ["envelope_finish", "envelope_merge", "engine_merge"] {
        for bad_first in [false, true] {
            out.push(Probe {
                name: "P10_null_wait_numeric_causes_rejected",
                input: json!({"kind":"review_malformed_point","seam":seam,"bad_first":bad_first,
                    "base_fixture":"wait-person","task":0,"wait":n("unfinished"),
                    "wait_causes":{"actor_capacity":s("0"),"resource":s("0"),"executor":s("0"),"combined":s("0"),"unattributed":s("0")}}),
                expected: json!({"rejected":true}),
            });
        }
    }
    out.push(Probe {
        name: "P11_negative_predecessor_rejected",
        input: json!({"kind":"review_history","T":4,
            "actors":[{"name":"A","capacity":1}],
            "cases":[{"arrival":0,"admitted":2,"end":4,"tasks":[
                {"binding":"target","actor":"A","ready":1,"start":3,"stop":4}]}]}),
        expected: json!({"rejected":true}),
    });
    for (begin, executor, unknown) in [(2, "2", "2"), (0, "4", "0")] {
        out.push(Probe {
            name: "P12_partial_executor_candidates",
            input: json!({"kind":"review_history","T":5,"people":["p","q"],
                "actors":[{"name":"A","capacity":2,"executors":["p","q"]},
                    {"name":"B","capacity":1,"executors":["p"]},
                    {"name":"C","capacity":1,"executors":["q"]}],
                "cases":[{"arrival":0,"admitted":0,"end":5,"tasks":[
                    {"binding":"target","actor":"A","ready":0,"start":4,"stop":5,"executor":"p"},
                    {"binding":"blocker_p","actor":"B","ready":0,"start":0,"stop":4,"executor":"p"},
                    {"binding":"blocker_q","actor":"C","ready":begin,"start":begin,"stop":4,"executor":"q"}]}]}),
            expected: json!({"target_wait":s("4"),"target_wait_causes":{
                "actor_capacity":s("0"),"resource":s("0"),"executor":s(executor),"combined":s("0"),"unattributed":s(unknown)},
                "A_wait_total":s("4"),"A_length_max":s("1"),"A_length_time_average":s("4/5")}),
        });
    }
    out
}

/// Callback must call the real collector/reducer, select fields only, and
/// explicitly normalize a production rejection to {rejected:true}. A callback
/// panic (including unsupported input/unwrap) is NOT accepted as a rejection.
/// On valid controls it must return {rejected:false}, never an empty result.
pub fn run_review(mut observe: impl FnMut(&str, &Value) -> Value) {
    for probe in review_probes() {
        assert_eq!(
            observe(probe.name, &probe.input),
            probe.expected,
            "{}: review hand oracle",
            probe.name
        );
        if probe.input["kind"] == "review_malformed_point" {
            let mut control = probe.input.clone();
            control["wait"] = s("0");
            assert_eq!(
                observe(probe.name, &control),
                json!({"rejected":false}),
                "{}: consistent numeric point must be accepted",
                probe.name
            );
        } else if probe.name.starts_with("P11") {
            let mut control = probe.input.clone();
            control["cases"][0]["admitted"] = json!(1);
            assert_eq!(
                observe(probe.name, &control),
                json!({"rejected":false}),
                "ready == admitted is valid predecessor0"
            );
        }
    }
}

/// Survivor F21: separate callback inventory so the existing P01-P12 adapters
/// cannot silently pretend to consume a new History shape. See the hand ledger.
/// Connect this to BOTH independent collectors.
pub fn mutation_probes() -> Vec<Probe> {
    [3, 4]
        .into_iter()
        .map(|begin| Probe {
            name: "P13_wait_interval_not_total",
            input: json!({"kind":"mutation_history","T":7,
            "actors":[{"name":"A","capacity":1}],
            "cases":[{"arrival":0,"admitted":0,"end":7,"waits":[[begin,5]],
                "tasks":[{"binding":"a","actor":"A","ready":0,"start":0,"stop":4},
                    {"binding":"b","actor":"A","ready":6,"start":6,"stop":7}]}]}),
            expected: json!({"in_service":s("5"),"intentional":s("1"),"unattributed":s("1"),
            "external":s("0"),"contest_route":s("0"),"queue":{
                "actor_capacity":s("0"),"resource":s("0"),"executor":s("0"),"combined":s("0")}}),
        })
        .collect()
}

pub fn run_mutation(mut observe: impl FnMut(&str, &Value) -> Value) {
    for probe in mutation_probes() {
        assert_eq!(
            observe(probe.name, &probe.input),
            probe.expected,
            "{}: survivor interval hand oracle",
            probe.name
        );
    }
}
