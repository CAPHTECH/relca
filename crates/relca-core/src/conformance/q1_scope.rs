//! Q1 model support is not evidence that the log schema records Q1 history.
use crate::{ast, diag, ir};
use std::collections::BTreeSet;

pub(super) struct Feature {
    pub item: String,
    pub actor: String,
    pub capability: String,
    pub process: Option<String>,
    pub span: diag::Span,
}

pub(super) fn declarations(model: &ast::Model) -> Vec<Feature> {
    fn call(c: &ast::Call, process: &str, rows: &mut Vec<Feature>) {
        for (name, span) in [
            ("response_loss", c.response_loss.as_ref().map(|s| s.span)),
            ("retry", c.retry.as_ref().map(|r| r.span)),
        ] {
            if let Some(span) = span {
                rows.push(Feature {
                    item: format!("q1:process:{process}:{}:{name}", c.binding.value),
                    actor: c.actor.value.clone(),
                    capability: c.capability.value.clone(),
                    process: Some(process.into()),
                    span,
                });
            }
        }
    }
    fn block(b: &ast::Block, p: &str, rows: &mut Vec<Feature>) {
        for s in b {
            match &s.value {
                ast::Stmt::Repeat { .. } | ast::Stmt::Again | ast::Stmt::Leave => {
                    rows.push(Feature {
                        item: format!("loop:process:{p}:repeat"),
                        actor: String::new(),
                        capability: String::new(),
                        process: Some(p.into()),
                        span: s.span,
                    })
                }
                ast::Stmt::Call(c) => call(c, p, rows),
                ast::Stmt::Fork { branches, .. } => {
                    for c in branches {
                        call(c, p, rows)
                    }
                }
                ast::Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    block(then_body, p, rows);
                    block(else_body, p, rows)
                }
                ast::Stmt::Wait(_)
                | ast::Stmt::WaitUntil(_)
                | ast::Stmt::End(_)
                | ast::Stmt::ResolveEffect { .. }
                | ast::Stmt::StopReceiptUnknown => {}
            }
        }
    }
    let mut rows = Vec::new();
    for item in &model.items {
        match item {
            ast::Item::Actor(a) => {
                for item in &a.items {
                    if let ast::ActorItem::Capability(c) = item {
                        for item in &c.items {
                            let feature = match &item.value {
                                ast::CapItem::ReceiptQuery { .. } => {
                                    Some("recovery_feature_combination")
                                }
                                ast::CapItem::QualifiedExecutors(_)
                                | ast::CapItem::OutcomesPerformance(_) => {
                                    Some("q6_feature_combination")
                                }
                                ast::CapItem::Idempotency(_) => Some("idempotency"),
                                ast::CapItem::EffectScopeModeled => Some("effect_scope"),
                                ast::CapItem::VersionCheck { .. }
                                | ast::CapItem::Atomic { .. }
                                | ast::CapItem::Uses(_)
                                | ast::CapItem::Requires { .. }
                                | ast::CapItem::Start(_)
                                | ast::CapItem::OutcomesKernel(_)
                                | ast::CapItem::KernelReadsLatent(_)
                                | ast::CapItem::Writes(_)
                                | ast::CapItem::Finish(_)
                                | ast::CapItem::Reads(_)
                                | ast::CapItem::Commits
                                | ast::CapItem::DurationFixed(_)
                                | ast::CapItem::DurationDistribution(_)
                                | ast::CapItem::ResultNondet(_)
                                | ast::CapItem::Cancel(_) => None,
                            };
                            if let Some(name) = feature {
                                rows.push(Feature {
                                    item: format!(
                                        "q1:capability:{}.{}:{name}",
                                        a.name.value, c.name.value
                                    ),
                                    actor: a.name.value.clone(),
                                    capability: c.name.value.clone(),
                                    process: None,
                                    span: item.span,
                                });
                            }
                        }
                    }
                }
            }
            ast::Item::Process(p) => block(&p.body, &p.name.value, &mut rows),
            _ => {} // Other declarations are checked by lower and the IR feature ledger.
        }
    }
    rows
}

pub(super) fn selected(
    rows: Vec<Feature>,
    m: &ir::Model,
) -> (Vec<Feature>, BTreeSet<ir::CapabilityId>) {
    let mut selected = Vec::new();
    let mut capabilities = BTreeSet::new();
    for f in rows {
        if f.process.as_ref().is_some_and(|p| *p != m.process.name) {
            continue;
        }
        if let Some((i, c)) = m
            .capabilities
            .iter()
            .enumerate()
            .find(|(_, c)| c.name == f.capability && m.actors[c.actor.0].name == f.actor)
        {
            capabilities.insert(ir::CapabilityId(i));
            if crate::responsibility::included(m, c.actor) {
                selected.push(f);
            }
        }
    }
    for (call, r) in m
        .response_attempts()
        .into_iter()
        .filter(|(_, r)| r.attempt == 1)
    {
        let c = &m.capabilities[call.capability.0];
        capabilities.insert(call.capability);
        selected.push(Feature {
            item: format!(
                "q1:ir:{}:{}:response_attempt",
                m.process.name,
                m.site_name(r.command)
            ),
            actor: m.actors[c.actor.0].name.clone(),
            capability: c.name.clone(),
            process: Some(m.process.name.clone()),
            span: call.span,
        });
    }
    (selected, capabilities)
}

#[cfg(test)]
mod tests {
    use crate::{Verdict, conformance, diag};
    use serde_json::json;
    const BASE: &str = r#"model M version "0.2" {type V=enum {ok,fail};people [p]
        actor A {executors [p];capability a()->V {duration fixed 1min;result nondet {ok}}}
        process Flow {r=A.a();end done}
        cut C {include [A]}
        contract C {excludes A.a.finish, A.a.start}
        bounds {cases 1;live_tasks 1;clock 5min} }"#;
    fn log(finish: u64) -> String {
        let header = json!({"kind":"header","schema":"relca-log/v1","clock":"test","time_unit":"min","window":{"from":0,"to":10},"cases":[{"case_id":"c","from_case_start":true,"through_case_end":true}],"actor_map":[{"source_id":"la","actor":"A"}],"executor_map":[{"source_id":"xp","person":"p"}]});
        let start = json!({"kind":"event","event_id":"s","case_id":"c","task_id":"t","operation":"A.a","lifecycle":"start","cancel_phase":null,"time":{"earliest":0,"latest":0},"after":[],"actor_id":"la","executor_id":"xp","input_version":"i","output_version":null,"result":null});
        let end = json!({"kind":"event","event_id":"f","case_id":"c","task_id":"t","operation":"A.a","lifecycle":"finish","cancel_phase":null,"time":{"earliest":finish,"latest":finish},"after":["s"],"actor_id":"la","executor_id":"xp","input_version":"i","output_version":"o","result":"ok"});
        format!("{header}\n{start}\n{end}\n")
    }
    #[test]
    fn q1_source_and_ir_semantics_are_not_automatically_log_supported() {
        let log = log(1);
        let baseline = conformance::conform(BASE, log.as_bytes(), None, None);
        assert_eq!(
            baseline.verdict,
            Verdict::Satisfied,
            "{:?}",
            baseline.diagnostics
        );
        let idempotent = r#"model M version "0.2" {type V=enum {ok,fail};people [p]
            actor A {executors [p];state keys:set<CaseId>;capability a(key:CaseId)->V {commits;idempotency key key;writes keys;duration fixed 1min;atomic {if key in keys {result fail}else{keys+=key;result ok}}}}
            process Flow {r=A.a(case.id);end done} cut C {include [A]}
            contract C {at_most_once A.a.commit per case.id}
            bounds {cases 1;live_tasks 1;clock 5min} }"#;
        for source in [
            BASE.replace("r=A.a()","r=A.a() response_loss none"),
            BASE.replace("r=A.a()","r=A.a() response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed"),
            BASE.replace("duration fixed 1min","effect_scope modeled;duration fixed 1min"),
            BASE.replace("r=A.a();","fork f {r=A.a() response_loss none} join all f;"),
            idempotent.into(),
        ] {
            let r=conformance::conform(&source,log.as_bytes(),None,None);
            assert_eq!(r.verdict,Verdict::Unsupported,"{:?}",r.diagnostics);
            assert!(r.diagnostics.iter().any(|d|d.code==diag::U_LOG_1 && d.reason.as_deref()==Some("unsupported_q1_model_feature")));
            assert!(r.log.sha256.is_some());
        }
    }
    #[test]
    fn q1_unsupported_keeps_independent_duration_violation_and_model_gate() {
        let source = BASE.replace("r=A.a()", "r=A.a() response_loss none");
        let r = conformance::conform(&source, log(2).as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Violated);
        assert!(r.diagnostics.iter().any(|d| d.code == diag::U_LOG_1));
        assert!(
            r.diagnostics
                .iter()
                .any(|d| d.code == diag::E_LOG_2 && d.reason.as_deref() == Some("duration"))
        );
        let invalid=BASE.replace("duration fixed 1min","commits;duration fixed 1min").replace("r=A.a()","r=A.a() response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed");
        let r = conformance::conform(&invalid, log(1).as_bytes(), None, None);
        assert_eq!(r.verdict, Verdict::Unsupported);
        assert!(r.diagnostics.iter().any(|d| d.code == diag::U_LOG_1));
        assert!(r.diagnostics.iter().any(|d| d.code == diag::U_SEM_0));
    }
}
