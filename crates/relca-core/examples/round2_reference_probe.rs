use relca_core::{lower, reference, syntax};
fn main() {
    let source = std::fs::read_to_string(std::env::args().nth(1).unwrap()).unwrap();
    let mut model = lower::lower(&syntax::parse(&source).unwrap()).unwrap();
    if std::env::args().nth(3).as_deref() == Some("same-target") {
        model.capabilities[1].writes[0].variable = model.capabilities[0].writes[0].variable;
    }
    let frames = std::env::args()
        .nth(2)
        .and_then(|s| s.parse().ok())
        .unwrap_or(1_000_000);
    let r = reference::explore_with_limits(
        &model,
        reference::Limits {
            frames,
            ..reference::Limits::default()
        },
    );
    println!(
        "{}",
        serde_json::json!({"stop":format!("{:?}",r.stop),"frames":r.statistics.frames,"paths":r.statistics.terminal_paths,"properties":format!("{:?}",r.outcome.properties),"ranges":r.outcome.state_range_violations,"quality":r.outcome.quality,"metrics":r.outcome.metrics,"diagnostics":r.outcome.diagnostics,"terminals":format!("{:?}",r.outcome.terminals)})
    );
}
