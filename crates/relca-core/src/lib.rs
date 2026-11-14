//! Relca core: typed business IR and analysis results.
//!
//! Finite-model analysis with explicit unsupported and inconclusive outcomes.
//! Entry points and result contracts are implemented in their respective modules.

pub mod actor_table;
pub mod agency;
pub mod ast;
pub mod compare;
mod comparison_projection;
pub mod conformance;
pub mod contest;
pub mod contract_scope;
mod degradation;
pub mod diag;
pub mod diff;
pub mod engine;
pub mod evaluation;
mod evidence_basis;
#[cfg(test)]
#[path = "../tests/support/r2_round2_inputs.rs"]
mod r2_round2_inputs;
#[cfg(test)]
pub(crate) mod evaluation_population {
    pub(crate) fn fixture(name: &str) -> bool {
        include!("../tests/support/evaluation_names.rs").contains(&name)
    }
}
#[cfg(test)]
pub(crate) mod evaluation_spy {
    use std::cell::Cell;
    thread_local! {
        static ACTIVE: Cell<bool> = const { Cell::new(false) };
        // lower, engine explore, reference explore, compare, engine simulate,
        // reference simulate, explain's trace explorer (not ordinary explore).
        static COUNTS: Cell<[usize;7]> = const { Cell::new([0;7]) };
    }
    pub(crate) fn hit(index: usize) {
        if ACTIVE.get() {
            COUNTS.update(|mut n| {
                n[index] += 1;
                n
            });
        }
    }
    pub(crate) fn observe<T>(f: impl FnOnce() -> T) -> (T, [usize; 7]) {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                ACTIVE.set(false)
            }
        }
        assert!(!ACTIVE.get());
        COUNTS.set([0; 7]);
        ACTIVE.set(true);
        let reset = Reset;
        let result = f();
        let counts = COUNTS.get();
        drop(reset);
        (result, counts)
    }
}
#[cfg(test)]
#[path = "../tests/support/evaluation_review_private_spy.rs"]
mod evaluation_review_private_spy;
pub mod explain;
#[cfg(test)]
mod integer_tests;
pub mod ir;
pub mod ledger;
#[cfg(test)]
#[path = "../../relca-cli/tests/support/ledger_shape.rs"]
pub(crate) mod ledger_test_shape;
pub mod lower;
pub mod metrics;
pub mod mutation;
pub mod outcome;
mod perf;
pub mod probability;
pub mod quality;
#[cfg(test)]
mod quality_impl_tests;
#[cfg(test)]
#[path = "../tests/support/quality_population.rs"]
pub(crate) mod quality_population;
#[cfg(test)]
#[path = "../tests/support/quality_private_contract.rs"]
mod quality_private_contract;
#[cfg(test)]
mod quantile_impl_tests;
#[cfg(test)]
#[path = "../tests/support/quantile_population.rs"]
pub(crate) mod quantile_population;
#[cfg(test)]
#[path = "../tests/support/quantile_private_contract.rs"]
mod quantile_private_contract;
#[cfg(test)]
mod quantile_test_support;
#[cfg(test)]
#[path = "../tests/support/r2_inputs.rs"]
mod r2_inputs;
#[cfg(test)]
#[path = "../tests/support/r2_private.rs"]
mod r2_private;
#[cfg(test)]
mod r2_test_adapter;
pub mod recovery;
pub mod reference;
pub mod report;
pub mod response_inputs;
pub mod responsibility;
#[cfg(test)]
#[path = "../tests/support/scenario_population.rs"]
pub(crate) mod scenario_population;
#[cfg(test)]
#[path = "../tests/support/scenario_private_replay.rs"]
mod scenario_private_replay;
pub mod sensitivity;
pub mod simulation;
#[cfg(test)]
#[path = "../tests/support/state_invariant_private_ir.rs"]
mod state_invariant_private_ir;
#[cfg(test)]
#[path = "../tests/support/state_invariant_private_replay.rs"]
mod state_invariant_private_replay;
#[cfg(test)]
mod state_invariant_tests;
pub mod syntax;
pub mod uncertainty;
#[cfg(test)]
pub(crate) mod state_invariant_population {
    include!("../tests/support/state_invariant_population.rs");
    pub(crate) fn fixture(name: &str) -> bool {
        assert!(
            !name.starts_with("sinv-") || NAMES.contains(&name),
            "unclassified state invariant fixture {name}"
        );
        NAMES.contains(&name)
    }
}

use serde::{Deserialize, Serialize};

/// Result classes kept distinct by design (fail closed).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    Satisfied,
    Violated,
    Inconclusive,
    Invalid,
    Unsupported,
    /// The property holds only because it never became active (design v0.2 §7.2).
    Vacuous,
}

impl Verdict {
    /// Process exit code for the CLI. Only `Satisfied` is 0.
    pub fn exit_code(self) -> i32 {
        match self {
            Verdict::Satisfied => 0,
            Verdict::Violated => 1,
            Verdict::Inconclusive => 2,
            Verdict::Invalid => 3,
            Verdict::Unsupported => 4,
            Verdict::Vacuous => 5,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Satisfied => "satisfied",
            Verdict::Violated => "violated",
            Verdict::Inconclusive => "inconclusive",
            Verdict::Invalid => "invalid",
            Verdict::Unsupported => "unsupported",
            Verdict::Vacuous => "vacuous",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_satisfied_exits_zero() {
        for v in [
            Verdict::Violated,
            Verdict::Inconclusive,
            Verdict::Invalid,
            Verdict::Unsupported,
            Verdict::Vacuous,
        ] {
            assert_ne!(v.exit_code(), 0, "{} must not exit 0", v.as_str());
        }
        assert_eq!(Verdict::Satisfied.exit_code(), 0);
    }

    #[test]
    fn exit_codes_are_distinct() {
        let all = [
            Verdict::Satisfied,
            Verdict::Violated,
            Verdict::Inconclusive,
            Verdict::Invalid,
            Verdict::Unsupported,
            Verdict::Vacuous,
        ];
        let mut codes: Vec<i32> = all.iter().map(|v| v.exit_code()).collect();
        codes.sort();
        codes.dedup();
        assert_eq!(codes.len(), all.len());
    }
}
mod completion;
pub mod obligation;

#[cfg(test)]
mod caseset_impl_tests;
#[cfg(test)]
#[path = "../tests/support/caseset_ir_internal.rs"]
mod caseset_ir_internal;
#[cfg(test)]
#[path = "../tests/support/caseset_projection_internal.rs"]
mod caseset_projection_internal;
#[cfg(test)]
#[path = "../tests/support/conflict_ir_internal.rs"]
mod conflict_ir_internal;
#[cfg(test)]
#[path = "../tests/support/p2_cleanup_shared.rs"]
pub(crate) mod p2_cleanup_shared;
#[cfg(test)]
mod q6_acceptance;
