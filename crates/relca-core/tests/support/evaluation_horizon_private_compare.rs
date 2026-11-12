//! Implementation linkage (inside compare.rs):
//! #[cfg(test)]
//! #[path = "../tests/support/evaluation_horizon_private_compare.rs"]
//! mod evaluation_horizon_private_compare;
//! No public DSL path currently makes selected plans disagree on horizon.
//! Currently linked through evaluation_review_private_compare::horizon_inventory,
//! which already has a cfg(test) seam in compare.rs. Do not link it twice.
use super::*;

fn selected(
    horizon: Option<u64>,
    name: &str,
    analysis: Analysis,
) -> (Plan, lower::SelectedConfiguration) {
    let h = horizon.map_or_else(String::new, |n| format!("horizon {n}min;"));
    let s = format!(
        r#"model HorizonInventory version "0.2" {{
        type T = enum {{ ok }}
        actor A {{ capacity 1; capability run() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}
        process P {{ x = A.run(); end done }}
        process Q {{ x = A.run(); end done }}
        cut C {{ include [A] }}
        configuration Left {{ cut C; process P }}
        configuration Right {{ cut C; process Q }}
        scenario Day {{ arrivals fixed [0min]; {h} bounds {{ cases 1; live_tasks 1; clock 10min }}; simulation replications 2 seed 7 confidence 0.5 }}
        verify reachable end.done
    }}"#
    );
    let (report, metadata) = match analysis {
        Analysis::Exhaustive => {
            let (r, m) = report::check_selected_scenario_using(
                &s,
                name,
                Some("Day"),
                crate::engine::explore,
            );
            (SelectedReport::Exhaustive(r), m)
        }
        Analysis::Simulation => {
            let (r, m) = report::simulate_selected_scenario(&s, name, Some("Day"));
            (SelectedReport::Simulation(r), m)
        }
    };
    let metadata = metadata.expect("legal selected configuration");
    assert_eq!(metadata.model.horizon, horizon);
    assert!(
        report.view(&metadata.model).is_ok(),
        "each nested report separately matches its own IR"
    );
    (
        Plan {
            configuration: name.into(),
            report,
        },
        metadata,
    )
}

fn assert_horizons(analysis: Analysis, pairs: &[(Option<u64>, Option<u64>)]) {
    for &(left, right) in pairs {
        let (a, ma) = selected(left, "Left", analysis);
        let (b, mb) = selected(right, "Right", analysis);
        let out = tables(&[a, b], &[ma, mb], analysis);
        if left == right {
            assert!(out.is_ok(), "matching horizon positive: {left:?}");
        } else {
            assert!(
                matches!(out, Err("report inventories differ")),
                "different effective horizons must fail closed: {left:?} vs {right:?}"
            );
        }
    }
}

#[test]
fn matching_effective_horizons_are_positive_in_both_modes() {
    for analysis in [Analysis::Exhaustive, Analysis::Simulation] {
        assert_horizons(analysis, &[(None, None), (Some(5), Some(5))]);
    }
}
#[test]
fn effective_horizon_presence_mismatch_exhaustive() {
    assert_horizons(Analysis::Exhaustive, &[(None, Some(5)), (Some(5), None)]);
}
#[test]
fn effective_horizon_presence_mismatch_simulation() {
    assert_horizons(Analysis::Simulation, &[(None, Some(5)), (Some(5), None)]);
}
#[test]
fn effective_horizon_value_mismatch_exhaustive() {
    assert_horizons(Analysis::Exhaustive, &[(Some(5), Some(6))]);
}
#[test]
fn effective_horizon_value_mismatch_simulation() {
    assert_horizons(Analysis::Simulation, &[(Some(5), Some(6))]);
}
