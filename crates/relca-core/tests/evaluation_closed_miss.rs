#[path = "support/evaluation_closed_miss_acceptance.rs"]
mod acceptance;
use relca_core::compare::{self, Analysis};
use serde_json::{Value, json};

fn run(source: &str, plans: &[&str], sampled: bool) -> Value {
    json!(compare::compare(
        source,
        plans,
        if sampled {
            Analysis::Simulation
        } else {
            Analysis::Exhaustive
        }
    ))
}
