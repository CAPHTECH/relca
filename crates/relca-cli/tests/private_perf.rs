//! The opt-in side channel must not alter stdout, stderr or exit status.
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{collections::BTreeMap, path::PathBuf, process::Command};

#[test]
fn private_stage_measurement_is_wire_neutral_and_reports_actual_work() {
    let tmp = test_tmp::root();
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let dir = tmp.join(format!("private-perf-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let input = dir.join("model.relca");
    std::fs::write(&input, r#"model M version "0.2" {
        type Ack=enum {committed,duplicate}
        actor L {capacity 1;state keys:set<CaseId>;capability post(key:CaseId)->Ack {
          commits;idempotency key key;writes keys;duration fixed 1min;
          atomic {if key in keys {result duplicate}else{keys+=key;result committed}}}}
        process P {r=L.post(case.id) response_loss sequence [unknown,delivered]
          retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed;end done}
        bounds {cases 2;live_tasks 2;clock 5min} arrivals fixed [0min,0min]
        metrics {window 5min;currency "JPY"}
        verify probability reached end.done >= 1
        }"#).unwrap();
    let next = std::cell::Cell::new(0);
    let run = |sink: Option<&std::path::Path>| {
        next.set(next.get() + 1);
        let cli_tmp = dir.join(format!("cli-{}", next.get()));
        std::fs::create_dir_all(&cli_tmp).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_relca"));
        command
            .arg("check")
            .arg(&input)
            .env("TMPDIR", cli_tmp)
            .env_remove("RELCA_PRIVATE_PERF_LOG");
        if let Some(sink) = sink {
            command.env("RELCA_PRIVATE_PERF_LOG", sink);
        }
        command.output().unwrap()
    };
    let plain = run(None);
    let sink = dir.join("stages.jsonl");
    let measured = run(Some(&sink));
    assert_eq!(plain.status.code(), measured.status.code());
    assert_eq!(plain.stdout, measured.stdout);
    assert_eq!(plain.stderr, measured.stderr);
    let unavailable = run(Some(&dir.join("missing-parent/log")));
    assert_eq!(plain.status.code(), unavailable.status.code());
    assert_eq!(plain.stdout, unavailable.stdout);
    assert_eq!(plain.stderr, unavailable.stderr);

    // In particular, never send private JSONL through the CLI's public pipes.
    // No reader opens the FIFO: the pre-open type check must reject it without
    // waiting for a peer. Every invocation still gets its own temporary dir.
    let mut refused = vec![dir.clone()];
    #[cfg(unix)]
    {
        refused.extend([PathBuf::from("/dev/stdout"), PathBuf::from("/dev/stderr")]);
        let fifo = dir.join("sink.fifo");
        assert!(
            Command::new("mkfifo")
                .arg(&fifo)
                .status()
                .unwrap()
                .success()
        );
        refused.push(fifo);
        let alias = dir.join("stdout-link");
        std::os::unix::fs::symlink("/dev/stdout", &alias).unwrap();
        refused.push(alias);
        let regular_alias = dir.join("regular-link");
        std::os::unix::fs::symlink(&sink, &regular_alias).unwrap();
        refused.push(regular_alias);
    }
    let saved = std::fs::read(&sink).unwrap();
    for path in refused {
        let output = run(Some(&path));
        assert_eq!(plain.status.code(), output.status.code(), "{path:?}");
        assert_eq!(plain.stdout, output.stdout, "{path:?}");
        assert_eq!(plain.stderr, output.stderr, "{path:?}");
        assert_eq!(saved, std::fs::read(&sink).unwrap());
    }
    let appended = run(Some(&sink));
    assert_eq!(plain.status.code(), appended.status.code());
    assert_eq!(plain.stdout, appended.stdout);
    assert_eq!(plain.stderr, appended.stderr);
    let extended = std::fs::read(&sink).unwrap();
    assert!(extended.starts_with(&saved));
    assert!(extended.len() > saved.len());

    let mut calls = BTreeMap::<String, u64>::new();
    let mut counters = BTreeMap::<(String, String), u64>::new();
    for line in std::str::from_utf8(&saved).unwrap().lines() {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        let stage = row["stage"].as_str().unwrap();
        *calls.entry(stage.into()).or_default() += row["sample"]["calls"].as_u64().unwrap();
        for (key, value) in row["sample"]["counters"].as_object().unwrap() {
            *counters.entry((stage.into(), key.clone())).or_default() += value.as_u64().unwrap();
        }
    }
    for stage in [
        "engine.graph",
        "engine.solve",
        "engine.metric_snapshot",
        "agency.derive",
        "agency.response",
        "reference.total",
        "seal",
    ] {
        assert!(calls[stage] > 0, "missing {stage}");
    }
    assert_eq!(calls["engine.graph"], 1);
    assert_eq!(calls["reference.total"], 1);
    assert!(counters[&("engine.graph".into(), "states".into())] > 0);
    assert!(counters[&("engine.graph".into(), "edges".into())] > 0);
    assert!(counters[&("reference.total".into(), "frames".into())] > 0);
    assert!(!calls.contains_key("reference.metric_snapshot"));
}
