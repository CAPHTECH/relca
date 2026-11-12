//! I13: the planned family is frozen before sampling. It is not reduced after a
//! refused/incomplete point and never changes engine draw keys or replication n.
use super::input::{FailureKind, InputError};
use crate::probability::Rational;
#[cfg(test)]
#[path = "../../tests/support/sensitivity_oracle.rs"]
mod hand;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuiteMultiplicity {
    confidence: Rational,
    rows: u64,
}

impl SuiteMultiplicity {
    /// Each input count is all probability-query × case rows of one planned
    /// model point, not its completed replications or a pooled case sample size.
    pub fn new(confidence: Rational, planned_rows: &[u64]) -> Result<Self, InputError> {
        if confidence <= Rational::zero() || confidence >= Rational::one() {
            return Err(InputError::new(
                FailureKind::ManifestInvalid,
                "suite confidence must be strictly between 0 and 1",
            ));
        }
        let rows = planned_rows
            .iter()
            .try_fold(0u64, |n, r| n.checked_add(*r))
            .ok_or_else(|| {
                InputError::new(
                    FailureKind::CapacityLimit,
                    "planned probability-row count overflow",
                )
            })?;
        if planned_rows.is_empty() || rows == 0 {
            return Err(InputError::new(
                FailureKind::TargetInvalid,
                "no planned probability-query case rows",
            ));
        }
        Ok(Self { confidence, rows })
    }
    pub fn confidence(&self) -> &Rational {
        &self.confidence
    }
    pub fn rows(&self) -> u64 {
        self.rows
    }
    pub(crate) fn validate_child(
        &self,
        rows: u64,
        confidence: &Rational,
    ) -> Result<(), InputError> {
        if rows > self.rows || confidence != &self.confidence {
            return Err(InputError::new(
                FailureKind::ContractMismatch,
                "point rows/confidence disagree with the preallocated suite family",
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Verdict, report};
    use serde_json::{Value, json};

    fn confidence() -> Rational {
        serde_json::from_value(json!("7/8")).unwrap()
    }
    fn run(source: &str, scope: &SuiteMultiplicity) -> Value {
        json!(report::simulate_with_multiplicity(
            source, None, None, scope
        ))
    }
    #[test]
    fn allocation_changes_both_cp_and_verdict_not_the_observations() {
        let local = json!(report::simulate(hand::ZERO));
        hand::zero_statistics(&local, 1, "1/16", "1/2", "satisfied").unwrap();
        let scope = SuiteMultiplicity::new(confidence(), &[1, 1]).unwrap();
        let suite = run(hand::ZERO, &scope);
        hand::zero_statistics(&suite, 2, "1/32", "607705/1048576", "inconclusive").unwrap();
        assert_eq!(suite["simulation"]["multiplicity"]["scope"], "suite");
        for field in [
            "replications",
            "seed",
            "attempted_replications",
            "completed_replications",
            "random_method",
        ] {
            assert_eq!(suite["simulation"][field], local["simulation"][field]);
        }
        for field in ["hits", "unknown", "trials"] {
            assert_eq!(
                hand::property(&suite, hand::QUERY)["cases"][0][field],
                hand::property(&local, hand::QUERY)["cases"][0][field]
            );
        }
        assert_eq!(scope.rows(), 2);
        assert_eq!(scope.confidence(), &confidence());
    }
    #[test]
    fn cases_are_rows_not_additional_independent_replications() {
        let scope = SuiteMultiplicity::new(confidence(), &[2, 2]).unwrap();
        let (value, calls) = crate::evaluation_spy::observe(|| {
            run(
                include_str!("../../tests/data/sensitivity/zero-two-cases.relca"),
                &scope,
            )
        });
        assert_eq!(calls[4], 1);
        hand::zero_statistics(&value, 4, "1/64", "677849/1048576", "inconclusive").unwrap();
        let cases = hand::property(&value, hand::QUERY)["cases"]
            .as_array()
            .unwrap();
        assert_eq!(cases.len(), 2);
        for case in cases {
            assert_eq!(case["trials"], 4);
            assert_eq!(case["hits"], 0);
        }
    }
    #[test]
    fn late_abort_keeps_the_original_family_and_withdraws_intervals() {
        let scope = SuiteMultiplicity::new(confidence(), &[2, 2]).unwrap();
        let value = run(hand::ABORT, &scope);
        assert_eq!(value["verdict"], "unsupported");
        assert_eq!(value["simulation"]["attempted_replications"], 3);
        assert_eq!(value["simulation"]["completed_replications"], 2);
        assert_eq!(value["simulation"]["multiplicity"]["rows"], 4);
        assert_eq!(value["simulation"]["multiplicity"]["alpha_after"], "1/64");
        for p in value["properties"].as_array().unwrap() {
            for c in p["cases"].as_array().unwrap() {
                assert!(c.get("interval").is_none());
            }
        }
        assert_eq!(scope.rows(), 4);
    }
    #[test]
    fn invalid_or_underallocated_context_refuses_before_sampling() {
        assert!(SuiteMultiplicity::new(confidence(), &[]).is_err());
        assert!(SuiteMultiplicity::new(confidence(), &[0]).is_err());
        assert_eq!(
            SuiteMultiplicity::new(confidence(), &[u64::MAX, 1])
                .unwrap_err()
                .kind,
            FailureKind::CapacityLimit
        );
        let scope = SuiteMultiplicity::new(confidence(), &[1]).unwrap();
        let (value, calls) = crate::evaluation_spy::observe(|| {
            run(
                include_str!("../../tests/data/sensitivity/zero-two-cases.relca"),
                &scope,
            )
        });
        assert_eq!(calls[4], 0);
        assert_eq!(value["verdict"], "invalid");
        assert_eq!(value["simulation"]["attempted_replications"], 0);
        assert_eq!(Verdict::Invalid.exit_code(), 3);
        assert!(value["diagnostics"].as_array().unwrap().iter().any(|d| {
            d["code"] == "E-CMP-0"
                && d["message"]
                    .as_str()
                    .unwrap()
                    .starts_with("sensitivity_contract_mismatch")
        }));
        let scope =
            SuiteMultiplicity::new(serde_json::from_value(json!("1/2")).unwrap(), &[1]).unwrap();
        let (value, calls) = crate::evaluation_spy::observe(|| run(hand::ZERO, &scope));
        assert_eq!(calls[4], 0);
        assert_eq!(value["verdict"], "invalid");
        assert_eq!(value["simulation"]["attempted_replications"], 0);
        assert!(
            !value["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "U-LEDGER-0"),
            "{}",
            value["diagnostics"]
        );
        assert_eq!(value["uncertainty"]["estimation"]["reason"], "no_analysis");
    }
}
