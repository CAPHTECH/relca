//! Independent typed leaf oracle for current mutation Reports (ADR 0034).
#![allow(dead_code)]
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

pub fn leaves(v: &Value, path: &str, out: &mut BTreeMap<String, Value>) {
    match v {
        Value::Object(o) => {
            for (k, v) in o {
                leaves(v, &format!("{path}/{k}"), out);
            }
        }
        Value::Array(a) if a.iter().all(Value::is_string) => {
            let set: BTreeSet<_> = a.iter().map(|x| x.as_str().unwrap()).collect();
            out.insert(path.into(), json!(set));
        }
        Value::Array(a) => {
            for v in a {
                let id = ["call", "capability", "effect", "actor", "person", "end"]
                    .into_iter()
                    .filter_map(|k| v.get(k).map(|x| format!("{k}={x}")))
                    .collect::<Vec<_>>()
                    .join(";");
                assert!(
                    !id.is_empty(),
                    "no numeric wildcard identities in viewpoints"
                );
                leaves(v, &format!("{path}/{id}"), out);
            }
        }
        _ => {
            out.insert(path.into(), v.clone());
        }
    }
}

pub fn changes(before: &Value, after: &Value) -> Vec<Value> {
    let mut b = BTreeMap::new();
    let mut a = BTreeMap::new();
    if !before.is_null() {
        leaves(before, "/viewpoints", &mut b);
    }
    if !after.is_null() {
        leaves(after, "/viewpoints", &mut a);
    }
    let keys: BTreeSet<_> = b.keys().chain(a.keys()).collect();
    keys.into_iter()
        .filter_map(|k| {
            let bv = b.get(k).unwrap_or(&Value::Null);
            let av = a.get(k).unwrap_or(&Value::Null);
            (bv != av).then(|| {
                json!({"check":"agency","key":k,"field":"value",
            "before":bv,"after":av,"direction":"changed"})
            })
        })
        .collect()
}
