//! Local-only manifest CLI. The core owns bounded source capture and all gates.
use relca_core::{
    Verdict,
    sensitivity::{input::Mode, run, wire::PointStatus},
};

pub fn run(args: &[String]) -> Verdict {
    let parsed = super::flags(args, &["--mode"], super::SIMULATE).and_then(|(path, flags)| {
        let mode = match *flags.get("--mode")? {
            "exhaustive" => Mode::Exhaustive,
            "simulation" => Mode::Simulation,
            _ => return None,
        };
        Some((path, mode))
    });
    let result = match parsed {
        Some((path, mode)) => run::run(std::path::Path::new(path), mode),
        None => run::argument_failure(
            Mode::Exhaustive,
            "usage: relca sensitivity <manifest> --mode exhaustive|simulation",
        ),
    };
    for point in &result.points {
        if point.status == PointStatus::Evaluated
            && let Some(report) = &point.report
        {
            for d in report.diagnostics() {
                super::print_diagnostic(format_args!("point {}: ", point.id), d);
            }
        }
    }
    for d in &result.diagnostics {
        super::print_diagnostic(format_args!("sensitivity: "), d);
    }
    println!(
        "{}",
        serde_json::to_string(&result).expect("sensitivity report serializes")
    );
    result.verdict
}
