//! Closed declaration DTOs shared by static comparison and effective inputs.
//! These describe assumptions, not receipt histories or execution semantics.
use crate::{ast::Receipt, ir, probability::Rational};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Loss {
    None,
    Nondeterministic,
    Sequence {
        receipts: Vec<Receipt>,
    },
    Probability {
        probability: Rational,
        #[serde(deserialize_with = "basis")]
        basis: String,
        independence: Independence,
    },
}
fn basis<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    let value = String::deserialize(d)?;
    if value.trim().is_empty() {
        return Err(serde::de::Error::custom("empty response basis"));
    }
    Ok(value)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Independence {
    PerAttemptConditionalOnNormalFinish,
}
impl From<&ir::ResponseLoss> for Loss {
    fn from(loss: &ir::ResponseLoss) -> Self {
        match loss {
            ir::ResponseLoss::None => Self::None,
            ir::ResponseLoss::Nondet => Self::Nondeterministic,
            ir::ResponseLoss::Sequence(xs) => Self::Sequence {
                receipts: xs.clone(),
            },
            ir::ResponseLoss::Probability { probability, basis } => Self::Probability {
                probability: probability.clone(),
                basis: basis.clone(),
                independence: Independence::PerAttemptConditionalOnNormalFinish,
            },
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Control {
    pub capability: String,
    pub arguments: Vec<String>,
    pub loss: Loss,
    #[serde(deserialize_with = "nullable")]
    pub retry: Option<Retry>,
    #[serde(deserialize_with = "nullable")]
    pub idempotency: Option<String>,
    #[serde(deserialize_with = "nullable")]
    pub effect_scope: Option<EffectScope>,
}
fn nullable<'de, D: serde::Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::deserialize(d)
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EffectScope {
    Modeled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Retry {
    pub max_attempts: u64,
    pub when: When,
    pub backoff: u64,
    pub exhausted: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct When {
    pub receipt_unknown: bool,
    pub results: Vec<String>,
}
pub(crate) fn control(m: &ir::Model, r: &ir::ResponseAttempt) -> Control {
    let call = &r.command_call;
    let c = &m.capabilities[call.capability.0];
    Control {
        capability: format!("{}.{}", m.actors[c.actor.0].name, c.name),
        arguments: call
            .args
            .iter()
            .map(|b| {
                if *b == ir::CURRENT_CASE {
                    "case.id".into()
                } else {
                    m.site_name(*b)
                }
            })
            .collect(),
        loss: Loss::from(&r.loss),
        retry: r.exhausted.map(|e| Retry {
            max_attempts: r.max_attempts,
            when: When {
                receipt_unknown: r.retry_unknown,
                results: r
                    .retry_results
                    .iter()
                    .map(|v| m.types[c.return_type.0].members[v.0].clone())
                    .collect(),
            },
            backoff: r.backoff,
            exhausted: m.ends[e.0].clone(),
        }),
        idempotency: r.idempotency.clone(),
        effect_scope: r.effect_modeled.then_some(EffectScope::Modeled),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn declared_idempotency_key_is_the_guarded_insert_key() {
        let source = r#"model M version "0.2" { type Ack=enum {ok,duplicate}
          actor Ledger {capacity 1;state keys:set<CaseId>
            capability post(key:CaseId,other:CaseId)->Ack {commits;idempotency key key;writes keys;duration fixed 1min;
              atomic {if key in keys {result duplicate} else {keys += key;result ok}} cancel forbidden_after_start}}
          process P {ack=Ledger.post(case.id,case.id) response_loss none;end done}
          bounds {cases 1;live_tasks 1;clock 3min} verify safety no_resource_overcommit}"#;
        let mut model = crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap();
        assert_eq!(
            crate::reference::explore(&model).stop,
            crate::reference::StopReason::Complete
        );
        assert!(crate::engine::explore(&model).diagnostics.is_empty());
        for statement in &mut model.process.body {
            if let crate::ir::Stmt::Response(_, r) = &mut statement.kind {
                r.idempotency = Some("other".into());
            }
        }
        assert_eq!(
            crate::reference::explore(&model).stop,
            crate::reference::StopReason::SemanticAbort
        );
        assert!(
            crate::engine::explore(&model)
                .diagnostics
                .iter()
                .any(|d| d.severity == crate::diag::Severity::Error)
        );
    }
}
