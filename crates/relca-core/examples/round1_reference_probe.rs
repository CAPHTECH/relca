use relca_core::{lower, reference, syntax};
use serde_json::json;
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let config = std::env::args().nth(3);
    let model =
        lower::lower_with_selection(&syntax::parse(&source).unwrap(), config.as_deref(), None)
            .unwrap();
    let frames = std::env::args().nth(2).and_then(|s| s.parse().ok());
    let result = reference::explore_with_limits(
        &model,
        reference::Limits {
            frames: frames.unwrap_or(1_000_000),
            ..reference::Limits::default()
        },
    );
    println!(
        "{}",
        json!({"stop":format!("{:?}", result.stop), "frames":result.statistics.frames,
        "terminal_paths":result.statistics.terminal_paths,
        "probability_scheduler":result.outcome.probability_scheduler,
        "probability_policy_unresolved":result.outcome.probability_policy_unresolved,
        "case_outcomes":format!("{:?}",result.outcome.case_outcomes),"truncated":result.outcome.truncated,
        "case_properties":format!("{:?}",result.outcome.properties),
        "properties":result.outcome.properties.iter().map(|p|json!({"verdict":p.verdict.as_str(),"probability":p.probability})).collect::<Vec<_>>(),
        "terminals":result.outcome.terminals.iter().map(|t|json!({"end":model.ends[t.end.0],"elapsed":t.elapsed,"work":t.work})).collect::<Vec<_>>(),
        "diagnostics":result.outcome.diagnostics, "metrics":result.outcome.metrics})
    );
}
