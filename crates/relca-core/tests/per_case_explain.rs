//! Configuration-free explain and independent arrivals replay acceptance.
use relca_core::{explain, report};
use serde_json::{Value, json};
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
mod ledger_shape;
#[path = "support/percase_acceptance.rs"]
mod p;
fn run(src: &str) -> Value {
    serde_json::to_value(explain::explain(src)).unwrap()
}

#[test]
fn f13_finish_values_case_ids_locations_joint_events_and_replay() {
    for (name, value) in [
        ("pcase-explain", "routine"),
        ("pcase-explain.broken-invariant", "difficult"),
    ] {
        let src = p::source(name);
        assert!(!src.contains("configuration"));
        let v = run(&src);
        assert_eq!(v["verdict"], "violated");
        assert_eq!(v["explanation"]["status"], "complete");
        let cs = v["explanation"]["counterexamples"].as_array().unwrap();
        assert_eq!(cs.len(), 1, "shortest per property, not per case");
        let c = &cs[0];
        assert_eq!(c["replay"]["status"], "verified");
        let case = c["case_id"].as_u64().unwrap();
        assert!([1, 2].contains(&case));
        let finish = c["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["kind"] == "finish" && s["case_id"] == case && s["capability"] == "read")
            .unwrap();
        let span = &finish["location"]["span"];
        assert_eq!(
            &src[span["start"].as_u64().unwrap() as usize..span["end"].as_u64().unwrap() as usize],
            "a = Reader.read()"
        );
        let latent = finish["values"]["latent"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "difficulty")
            .unwrap();
        assert_eq!(latent["value"], value);
        assert_eq!(latent["visibility"], "latent");
        let observed = finish["values"]["observed"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["name"] == "obs")
            .unwrap();
        assert_eq!(observed["value"], value);
        let init = c["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| {
                s["kind"] == "initialize"
                    && s["events"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .filter(|e| e["kind"] == "initialize")
                        .count()
                        == 2
            })
            .unwrap();
        assert!(init["case_id"].is_null());
        let ids: Vec<_> = init["events"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|e| e["kind"] == "initialize")
            .map(|e| e["case_id"].clone())
            .collect();
        assert_eq!(ids, json!([1, 2]).as_array().unwrap().clone());
        let mut actual = v.clone();
        actual.as_object_mut().unwrap().remove("explanation");
        assert_eq!(
            ledger_shape::original_report(&actual),
            ledger_shape::original_report(&serde_json::to_value(report::check(&src)).unwrap())
        );
        p::assumptions(&v, &["Invoice"], true);
    }
}

#[test]
fn replay_disambiguates_equal_local_names_by_declaration_not_bare_name() {
    let src = p::source("pcase-explain")
        .replace(
            "  process Flow",
            "  actor Other { instance per_case; latent difficulty: bool = true }\n  process Flow",
        )
        .replace("include [Invoice,Reader]", "include [Invoice,Reader,Other]");
    let v = run(&src);
    assert_eq!(v["verdict"], "violated");
    assert_eq!(
        v["explanation"]["counterexamples"][0]["replay"]["status"],
        "verified"
    );
    let steps = v["explanation"]["counterexamples"][0]["steps"]
        .as_array()
        .unwrap();
    let finish = steps
        .iter()
        .find(|s| s["kind"] == "finish" && s["actor"] == "Reader")
        .unwrap();
    let values: Vec<_> = finish["values"]["latent"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|v| v["name"] == "difficulty")
        .collect();
    assert_eq!(values.len(), 2);
    assert_ne!(values[0]["location"], values[1]["location"]);
    assert_eq!(values[0]["value"], "routine");
    assert_eq!(values[1]["value"], "true");
}
