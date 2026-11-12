//! Value-level companion for pre-C2 hand Agency equality (not hash comparison).
//! Remove exactly one canonical new member, preserving all old values/order.
#![allow(dead_code)]
use serde_json::Value;
pub fn agency(a: &Value) -> Value {
    let mut a = a.clone();
    if let Some(o) = a.as_object_mut()
        && let Some(v) = o.remove("viewpoints")
    {
        assert!(v["rules"] == "agency-viewpoints-p1-1" || v["rules"] == "agency-viewpoints-p1-2");
        if v["rules"] == "agency-viewpoints-p1-2" {
            let tags = o.get_mut("unchecked").unwrap().as_array_mut().unwrap();
            tags.retain(|x| x != "responsibility_people_undeclared");
            if v["effects"]["status"] == "derived" {
                let at = tags.iter().position(|x| x == "fallback_path").unwrap() + 1;
                tags.insert(at, serde_json::json!("executors"));
            }
        }
        assert_eq!(
            v["scope"],
            "selected_process_lexical_calls_and_declared_contest_routes"
        );
    }
    a
}
