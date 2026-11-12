//! Fixed private seam. Adapter must call the actual work_profiles::record once
//! per case; never replace a missing table with a synthetic Err in the adapter.
#![allow(dead_code)]
use super::private_types::*;
type Mutation = (&'static str, fn(&mut Model, usize));

pub fn check_record(
    mut record: impl FnMut(&Model, &str) -> Result<Option<serde_json::Value>, String>,
    source: &str,
    model: &Model,
) {
    let good = record(model, source)
        .expect("valid record")
        .expect("complete table");
    assert_eq!(good["profiles"].as_array().unwrap().len(), 1);
    let cap = model
        .capabilities
        .iter()
        .position(|c| c.name == "run")
        .unwrap();
    let mutations: &[Mutation] = &[
        ("actor", |m, c| {
            m.capabilities[c].actor = ActorId(usize::MAX)
        }),
        ("qualified", |m, c| {
            m.capabilities[c].qualified_executors = Some(vec![PersonId(usize::MAX)])
        }),
        ("permission", |m, c| {
            m.capabilities[c].permission = Some(vec![PersonId(usize::MAX)])
        }),
        ("profile", |m, c| {
            m.capabilities[c].performance.as_mut().unwrap().profile = usize::MAX
        }),
        ("row-person", |m, _| {
            m.performances[0].rows[0].executor = Some(PersonId(usize::MAX))
        }),
        ("row-kernel", |m, _| {
            m.performances[0].rows[0].kernel = usize::MAX
        }),
        ("source-executors", |m, c| {
            m.actors[m.capabilities[c].actor.0].executors = Some(vec![PersonId(0)])
        }),
    ];
    let mut failed = Vec::new();
    for (name, mutate) in mutations {
        let mut m = model.clone();
        mutate(&mut m, cap);
        match record(&m, source) {
            Err(e) if !e.is_empty() => (),
            other => failed.push(format!("{name}: no silent omission, got {other:?}")),
        }
    }
    if record(model, "not a model").is_ok() {
        failed.push("parse failure is not absence".into());
    }
    assert!(failed.is_empty(), "{}", failed.join("\n"));
}
