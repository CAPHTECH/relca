#[path = "support/population_oracles.rs"]
mod oracle;
use serde_json::Value;

#[test]
fn all_46_selected_inline_sources_calibrate_the_independent_ir_relation() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = std::process::Command::new("python3")
        .args([
            "-B",
            "crates/relca-cli/tests/read_write_inline_inputs.py",
            "--json",
        ])
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let inputs: Vec<Value> = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(inputs.len(), 46);
    let mut positive = 0;
    for input in inputs {
        let ast = relca_core::syntax::parse(input["source"].as_str().unwrap()).unwrap();
        let mut model =
            relca_core::lower::lower_with_selection(&ast, input["configuration"].as_str(), None)
                .unwrap();
        if std::env::var("RELCA_INLINE_IR_FAULT").as_deref() == Ok("producer")
            && input["rows"] != serde_json::json!([])
        {
            let index = model
                .capabilities
                .iter()
                .position(|c| {
                    input["rows"].as_array().unwrap().iter().any(|r| {
                        r["value"]
                            .as_str()
                            .unwrap()
                            .ends_with(&format!(" -> {}.{}", model.actors[c.actor.0].name, c.name))
                    })
                })
                .unwrap();
            eprintln!("fault applied inline independent IR producer");
            model.capabilities[index].name.push_str("_fault");
        }
        let rows = serde_json::json!(oracle::review::h::rows(&model));
        assert_eq!(
            rows, input["rows"],
            "independent IR versus hand source {}",
            input["name"]
        );
        if rows != serde_json::json!([]) {
            positive += 1;
            // Corrupt the IR producer name, then run the real independent
            // relation comparator against the same hand expectation.
            let index = model
                .capabilities
                .iter()
                .position(|c| {
                    input["rows"].as_array().unwrap().iter().any(|r| {
                        r["value"]
                            .as_str()
                            .unwrap()
                            .ends_with(&format!(" -> {}.{}", model.actors[c.actor.0].name, c.name))
                    })
                })
                .unwrap();
            model.capabilities[index].name.push_str("_fault");
            assert_ne!(
                serde_json::json!(oracle::review::h::rows(&model)),
                input["rows"]
            );
        }
    }
    assert_eq!(positive, 23);
}
