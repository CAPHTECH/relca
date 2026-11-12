#[cfg(test)]
mod round2_tests {
    use super::*;
    use serde_json::json;
    fn isolated(stop: Stop) -> (Recovery, Value) {
        let mut host = json!(crate::report::simulate(&crate::r2_inputs::source("R01")));
        host["case_outcomes"][0]["terminals"] = json!([]);
        host["case_outcomes"][0]["range_stopped_count"] = json!(0);
        host["case_outcomes"][0]["deadlocked_count"] = json!(0);
        let mut recovery: Recovery =
            serde_json::from_value(host["response_recovery"].clone()).unwrap();
        recovery.paths[0].commands[0].stop_reason = stop;
        match stop {
            Stop::StateRange=>{
                host["state_range_violations"]=json!([{"case_id":1}]);
                host["case_outcomes"][0]["range_stopped_count"]=json!(1);
            }
            Stop::SemanticAbort=>host["diagnostics"].as_array_mut().unwrap().push(json!({"code":"U-SEM-0","severity":"error","message":"invalid_recovery_ir","span":null})),
            Stop::Deadlock=>{
                host["diagnostics"].as_array_mut().unwrap().push(json!({"code":"W-DEADLOCK-0","severity":"warning","message":"deadlock","span":null}));
                host["case_outcomes"][0]["deadlocked_count"]=json!(1);
            }
            _=>panic!("stop"),
        }
        recovery
            .validate_host(&host)
            .expect("positive host reaches the selected stop arm");
        (recovery, host)
    }
    #[test]
    fn round2_state_range_arm_checks_its_case_count() {
        let (r, mut host) = isolated(Stop::StateRange);
        host["case_outcomes"][0]["range_stopped_count"] = json!(0);
        assert!(r.validate_host(&host).is_err());
    }
    #[test]
    fn round2_semantic_abort_arm_requires_an_error() {
        let (r, mut host) = isolated(Stop::SemanticAbort);
        host["diagnostics"]
            .as_array_mut()
            .unwrap()
            .retain(|d| d["severity"] != "error");
        assert!(r.validate_host(&host).is_err());
    }
    #[test]
    fn round2_deadlock_arm_checks_its_case_count() {
        let (r, mut host) = isolated(Stop::Deadlock);
        host["case_outcomes"][0]["deadlocked_count"] = json!(0);
        assert!(r.validate_host(&host).is_err());
    }
}
