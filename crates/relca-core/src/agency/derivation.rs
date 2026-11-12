//! A missing derivation is a state, never an empty derived table.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum AgencyDerivation {
    Derived(Box<Agency>),
    NotDerived(Box<NotDerivedAgency>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotDerivedStatus {
    NotDerived,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LimitReason {
    DerivationLimit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotDerivedAgency {
    pub status: NotDerivedStatus,
    pub reason: LimitReason,
    pub limit: DerivationLimit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub degradation: Option<Degradation>,
    pub unchecked: Vec<String>,
}

impl From<Agency> for AgencyDerivation {
    fn from(a: Agency) -> Self {
        Self::Derived(Box::new(a))
    }
}
impl AgencyDerivation {
    pub fn as_derived(&self) -> Option<&Agency> {
        match self {
            Self::Derived(a) => Some(a),
            Self::NotDerived(_) => None,
        }
    }
    pub fn as_derived_mut(&mut self) -> Option<&mut Agency> {
        match self {
            Self::Derived(a) => Some(a),
            Self::NotDerived(_) => None,
        }
    }
    pub fn unchecked(&self) -> &[String] {
        match self {
            Self::Derived(a) => &a.unchecked,
            Self::NotDerived(a) => &a.unchecked,
        }
    }
    pub fn unchecked_mut(&mut self) -> &mut Vec<String> {
        match self {
            Self::Derived(a) => &mut a.unchecked,
            Self::NotDerived(a) => &mut a.unchecked,
        }
    }
    pub fn degradation(&self) -> Option<&Degradation> {
        match self {
            Self::Derived(a) => a.degradation.as_ref(),
            Self::NotDerived(a) => a.degradation.as_ref(),
        }
    }
    pub fn limit(&self) -> Option<&DerivationLimit> {
        match self {
            Self::NotDerived(a) => Some(&a.limit),
            Self::Derived(_) => None,
        }
    }
}

pub(crate) fn derive_for_model(
    model: &ir::Model,
) -> std::result::Result<(AgencyDerivation, Vec<diag::Diagnostic>), diag::Diagnostic> {
    let _timer = crate::perf::span("agency.derive");
    crate::actor_table::validate(model)
        .map_err(|e| Error::InvalidIr(e.message.into()).diagnostic())?;
    let degradation = degradation_for_model(model).map_err(Error::diagnostic)?;
    match derive_main(model, DEFAULT, [DEFAULT; 3], DEFAULT, degradation.clone()) {
        Ok((agency, warnings)) => Ok((agency.into(), warnings)),
        Err(Error::InvalidIr(reason)) => Err(Error::InvalidIr(reason).diagnostic()),
        Err(Error::Limit(limit)) => {
            let warning = diag::Diagnostic {
                code: diag::W_AGY_4.into(),
                severity: diag::Severity::Warning,
                message: limit.to_string(),
                span: limit.span,
            };
            let unchecked = vec![
                format!(
                    "agency: process {}: not_derived (derivation_limit)",
                    model.process.name
                ),
                format!(
                    "responsibility_default_candidates: process {}: not_derived (derivation_limit)",
                    model.process.name
                ),
            ];
            Ok((
                AgencyDerivation::NotDerived(Box::new(NotDerivedAgency {
                    status: NotDerivedStatus::NotDerived,
                    reason: LimitReason::DerivationLimit,
                    limit: *limit,
                    degradation,
                    unchecked,
                })),
                vec![warning],
            ))
        }
    }
}

#[cfg(test)]
// The frozen private acceptance loads this file in its own private namespace.
#[allow(clippy::duplicate_mod)]
#[path = "../../tests/support/agency_limit_inputs.rs"]
pub(crate) mod test_input;

#[cfg(test)]
mod tests {
    use super::test_input as input;
    use super::*;

    #[test]
    fn unavailable_agency_is_a_closed_state_and_invalid_ir_is_still_an_error() {
        let m = crate::lower::lower(&crate::syntax::parse(&input::work()).unwrap()).unwrap();
        assert!(m.agency.as_derived().is_none());
        let wire = serde_json::to_value(&m.agency).unwrap();
        assert_eq!(
            serde_json::from_value::<AgencyDerivation>(wire.clone()).unwrap(),
            m.agency
        );
        for key in ["origin", "refusal", "lineage"] {
            let mut forged = wire.clone();
            forged[key] = serde_json::json!([]);
            assert!(serde_json::from_value::<AgencyDerivation>(forged).is_err());
        }
        let mut bad = m;
        bad.process.body.push(bad.process.body[0].clone());
        assert!(
            derive_for_model(&bad)
                .unwrap_err()
                .message
                .starts_with("invalid static IR for Agency:")
        );
    }

    #[test]
    fn contest_validation_compares_the_unavailable_record_instead_of_empty_tables() {
        let mut m = crate::lower::lower(&crate::syntax::parse(&input::contest()).unwrap()).unwrap();
        assert!(m.agency.limit().is_some());
        crate::responsibility::validate(&m).unwrap();
        let AgencyDerivation::NotDerived(a) = &mut m.agency else {
            panic!();
        };
        *a.limit.requested.as_mut().unwrap() += 1;
        assert!(crate::responsibility::validate(&m).is_err());
    }

    #[test]
    fn via_effect_path_survives_but_unknown_refusal_targets_are_not_fabricated() {
        let s = input::work().replace(
            "actor Intake { capacity 1",
            "actor Intake { via model \"doc@v1\"; capacity 1",
        );
        let m = crate::lower::lower(&crate::syntax::parse(&s).unwrap()).unwrap();
        assert!(m.agency.as_derived().is_none());
        let paths = via_model_paths(&m).unwrap();
        assert!(!paths.truncated);
        assert!(
            paths
                .paths
                .iter()
                .any(|p| matches!(&p.target, ViaTarget::E {call} if call == "p"))
        );
        assert!(
            paths
                .paths
                .iter()
                .all(|p| !matches!(p.target, ViaTarget::U { .. }))
        );
        assert!(
            paths
                .unchecked
                .iter()
                .any(|u| u == "refusal_targets_not_derived_derivation_limit")
        );
        assert_eq!(
            paths.paths,
            vec![ViaPath {
                actor: "Intake".into(),
                model: "doc@v1".into(),
                start_call: "s".into(),
                calls: vec![
                    ViaCall {
                        call: "s".into(),
                        capability: "Intake.start".into()
                    },
                    ViaCall {
                        call: "j11".into(),
                        capability: "Judge.check".into()
                    },
                    ViaCall {
                        call: "p".into(),
                        capability: "Ledger.post".into()
                    }
                ],
                target: ViaTarget::E { call: "p".into() },
            }]
        );
    }

    #[test]
    fn r1_degradation_does_not_panic_before_rejecting_invalid_call_indices() {
        let base =
            crate::lower::lower(&crate::syntax::parse(&input::degradation()).unwrap()).unwrap();
        for fault in 0..7 {
            let mut m = base.clone();
            let ir::Stmt::Call(c) = &mut m.process.body[0].kind else {
                panic!()
            };
            match fault {
                0 => c.capability = ir::CapabilityId(usize::MAX),
                1 => c.binding = ir::BindingId(usize::MAX),
                2 => m.capabilities[c.capability.0].actor = ir::ActorId(usize::MAX),
                3 => m.process.degradations[0].producer = ir::CapabilityId(usize::MAX),
                4 => m.process.degradations[0].condition.binding = ir::BindingId(usize::MAX),
                5 => m.capabilities[c.capability.0]
                    .reads
                    .push(ir::ActorId(usize::MAX)),
                _ => {
                    m.capabilities[c.capability.0].qualified_executors =
                        Some(vec![ir::PersonId(usize::MAX)])
                }
            }
            let result = std::panic::catch_unwind(|| derive(&m));
            assert!(
                result.is_ok(),
                "invalid static IR must not panic: fault {fault}"
            );
            assert_eq!(result.unwrap().unwrap_err().code, diag::U_SEM_0);
            let result = std::panic::catch_unwind(|| derive_for_model(&m));
            assert!(
                result.is_ok(),
                "lowering boundary must not panic: fault {fault}"
            );
            assert!(
                result
                    .unwrap()
                    .unwrap_err()
                    .message
                    .starts_with("invalid static IR for Agency:")
            );
        }
    }
}
