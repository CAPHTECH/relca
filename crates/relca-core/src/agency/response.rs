//! ADR 0056: aliases are provenance links, not additional judgements or Tasks.
use super::*;
use crate::{outcome::CapabilityExercise, report};

pub(crate) fn apply(
    report: &mut report::Report,
    m: &ir::Model,
    exercise: Option<&[CapabilityExercise]>,
) {
    let _timer = crate::perf::span("agency.response");
    let commands: BTreeSet<_> = m
        .response_attempts()
        .into_iter()
        .map(|(_, r)| m.site_name(r.command))
        .collect();
    if commands.is_empty() {
        return;
    }
    let reference = exercise.map(|_| crate::reference::explore_response_exercise(m));
    let counts = reference
        .as_ref()
        .filter(|r| r.stop == crate::reference::StopReason::Complete && !r.outcome.truncated)
        .and_then(|r| r.outcome.response_exercise.as_deref())
        .filter(|r| Some(*r) == exercise);
    report.assumptions.push(ir::Assumption {field:"agency.response_alias_exclusions".into(),value:format!("count={}; bindings={:?}",commands.len(),commands),
        affects:"non_executable_result_alias: 元bindingは既知の採用値へのlinkであり、追加のTask・J・独立根拠ではない。実際の全試行サイトは残す。".into()});
    report.assumptions.push(ir::Assumption {field:"agency.response_control".into(),value:if counts.is_some(){"both_interpreters_complete_per_case_start_envelope"}else{"derivation_limit"}.into(),
        affects:"新入力のF/Uはresponse_controlでnot_derived。O・実試行のJ系譜・Eは保持。Lは両探索器の切断なし完全探索のStart範囲が一致した場合だけ導く。共通根拠は試行数で増やさない。".into()});
    // The path table belongs to Report, independently of optional Agency data.
    report.via_model_paths = None;
    report.assumptions.push(ir::Assumption {
        field: "via_model_paths".into(),
        value: "not_derived(response_control)".into(),
        affects: "応答の受信と採用を含む読取り経路は未導出。via model宣言の有無にかかわらず、空の経路集合を検証したことにはならない。".into(),
    });
    let Some(agency) = report
        .agency
        .as_mut()
        .and_then(AgencyDerivation::as_derived_mut)
    else {
        return;
    };
    agency.rules = "agency-p2-response-1".into();
    let aliases = agency.lineage.clone();
    for (call, r) in m.response_attempts() {
        if let Some(alias) = aliases
            .iter()
            .find(|row| row.call == m.site_name(r.command))
        {
            let name = m.site_name(call.binding);
            if !agency.lineage.iter().any(|row| row.call == name) {
                let mut row = alias.clone();
                row.call = name;
                agency.lineage.push(row);
            }
        }
    }
    agency.lineage.retain(|r| !commands.contains(&r.call));
    // The legacy graph keeps aliases to propagate result dependencies. They
    // must not become extra kernel executions in either derived table. A
    // dependency on an adopted result refers to its real lexical attempts,
    // not to an additional execution of the command binding.
    if let Some(causes) = agency
        .common_causes
        .as_mut()
        .and_then(CommonCauseDerivation::as_derived_mut)
    {
        for cause in &mut causes.causes {
            cause.targets.retain(|t| !commands.contains(&t.call));
        }
        causes.causes.retain(|c| c.targets.len() >= 2);
    }
    if agency
        .common_causes
        .as_ref()
        .and_then(CommonCauseDerivation::as_derived)
        .is_some_and(|c| c.causes.is_empty())
    {
        agency.common_causes = None;
    }
    if let Some(uncertainties) = &mut agency.model_uncertainties {
        uncertainties.items.retain(|r| !commands.contains(&r.call));
        let mut attempts: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for (call, r) in m.response_attempts() {
            attempts
                .entry(m.site_name(r.command))
                .or_default()
                .push(m.site_name(call.binding));
        }
        let redirect = |names: &mut Vec<String>| {
            *names = names
                .iter()
                .flat_map(|n| attempts.get(n).cloned().unwrap_or_else(|| vec![n.clone()]))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
        };
        for row in &mut uncertainties.items {
            redirect(&mut row.targets);
            if let common_cause::Suppression::Correlation { judgements } = &mut row.suppression {
                redirect(judgements);
            }
        }
    }
    if agency
        .model_uncertainties
        .as_ref()
        .is_some_and(|u| u.items.is_empty())
    {
        agency.model_uncertainties = None;
    }
    let mut groups: BTreeMap<Vec<String>, (usize, BTreeSet<String>, BTreeSet<String>)> =
        BTreeMap::new();
    for row in &agency.lineage {
        let g = groups.entry(row.subject_sources.clone()).or_default();
        g.0 += 1;
        g.1.insert(row.capability.split('.').next().unwrap_or("").into());
        g.2.extend(row.independent_sources.iter().cloned());
    }
    agency.lineage_groups = groups
        .into_iter()
        .map(
            |(subject_sources, (judgements, actors, sources))| LineageGroup {
                subject_sources,
                judgements,
                actors: actors.len(),
                independent_sources: sources.into_iter().collect(),
            },
        )
        .collect();
    agency.refusal.clear();
    agency.unchecked.push("response_control".into());
    if let Some(v) = &mut agency.viewpoints {
        v.scope="O/J/E static reach; F/U not_derived(response_control); L independently cross-checked exhaustive per-case Start envelope".into();
        v.framing = viewpoints::Framing::NotDerived {
            reason: viewpoints::LimitReason::ResponseControl,
            complete: false,
            unchecked: vec!["response_control".into()],
        };
        if let (Some(counts), viewpoints::Retention::Derived { capabilities, .. }) =
            (counts, &mut v.retention)
        {
            for c in capabilities {
                let (capability, contest_route, basis, fallback, join_any_cancellation) =
                    match c.clone() {
                        viewpoints::Exercise::Derived {
                            capability,
                            contest_route,
                            basis,
                            fallback,
                            join_any_cancellation,
                            ..
                        }
                        | viewpoints::Exercise::NotDerived {
                            capability,
                            contest_route,
                            basis,
                            fallback,
                            join_any_cancellation,
                            ..
                        } => (
                            capability,
                            contest_route,
                            basis,
                            fallback,
                            join_any_cancellation,
                        ),
                    };
                if let Some(n) = counts.iter().find(|n| {
                    let cap = &m.capabilities[n.capability.0];
                    format!("{}.{}", m.actors[cap.actor.0].name, cap.name) == capability
                }) {
                    *c = viewpoints::Exercise::Derived {
                        capability,
                        normal: viewpoints::Count {
                            min: n.min,
                            max: n.max,
                        },
                        contest_route,
                        basis,
                        fallback,
                        join_any_cancellation,
                    };
                }
            }
        } else {
            v.retention = viewpoints::Retention::NotDerived {
                reason: viewpoints::LimitReason::DerivationLimit,
                notice: "切断・上限・両探索器の不一致では行使回数を導かない".into(),
                unchecked: vec!["response_control".into()],
            };
        }
        if let Some(effects::Effects::Derived { rows, unchecked }) = &mut v.effects {
            rows.retain(|r| !commands.contains(&r.call));
            unchecked.push("response_control".into());
            for row in rows {
                row.j = None;
                row.u = None;
                row.state_refusers.clear();
                let underived = effects::Underived {
                    status: effects::NotDerived::NotDerived,
                    reason: viewpoints::StateReason::ResponseControl,
                };
                row.process_judgements = effects::ProcessJudgements::NotDerived(underived.clone());
                row.concentration.generation_roots = effects::Roots::NotDerived(underived);
                row.concentration.single_actor = effects::Truth::Unknown(effects::Unknown::Unknown);
                row.concentration.single_person =
                    effects::Truth::Unknown(effects::Unknown::Unknown);
                row.concentration.single_generation_root =
                    effects::Truth::Unknown(effects::Unknown::Unknown);
            }
        }
    }
    for duty in report.responsibilities.as_mut().into_iter().flatten() {
        duty.refusal.status = "not_derived".into();
        duty.refusal.reason = Some("response_control".into());
        duty.refusal.effects.clear();
        duty.warnings.retain(|w| w != "W-RESP-2");
    }
    // These legacy diagnostics assume unconditional call/control edges. Their
    // absence on a response-controlled process is not a successful check.
    for code in ["W-AGY-0", "W-AGY-2", "W-LIN-2", "W-RESP-2"] {
        report.assumptions.push(ir::Assumption {
            field: format!("agency.diagnostics.{code}"),
            value: "unchecked(response_control)".into(),
            affects: "応答の受信と採用に依存する制御を旧警告の導出は扱わない。警告の不在を問題なしと解釈しない。".into(),
        });
    }
    report.diagnostics.retain(|d| {
        !matches!(
            d.code.as_str(),
            "W-AGY-0" | "W-AGY-2" | "W-LIN-2" | "W-RESP-2"
        )
    });
}

#[cfg(test)]
mod tests {
    #[test]
    fn aliases_are_not_judgements_and_l_requires_both_complete_interpreters() {
        let source = r#"model M version "0.2" {type Ack=enum {committed,duplicate}
            actor Original {source document}
            actor L {capacity 1; state keys:set<CaseId>; capability post(key:CaseId)->Ack {
                reads [Original]; commits; idempotency key key; writes keys; duration fixed 1min;
                atomic {if key in keys {result duplicate} else {keys+=key;result committed}}}}
            process P {r=L.post(case.id) response_loss sequence [unknown,delivered]
                retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed; end done}
            bounds {cases 1;live_tasks 1;clock 5min} verify safety no_resource_overcommit }"#;
        let report = crate::report::check(source);
        let v = serde_json::to_value(&report).unwrap();
        let a = &v["agency"];
        assert!(
            a["lineage"]
                .as_array()
                .unwrap()
                .iter()
                .all(|r| r["call"] != "r")
        );
        // CaseId-only posting is not an artifact judgement in the existing
        // lineage contract. It must not acquire a synthetic J from its alias.
        assert_eq!(a["lineage"].as_array().unwrap().len(), 0);
        for row in a["lineage"].as_array().unwrap() {
            assert_eq!(row["independent_sources"], serde_json::json!(["Original"]));
        }
        assert_eq!(a["viewpoints"]["framing"]["reason"], "response_control");
        assert_eq!(
            a["viewpoints"]["retention"]["capabilities"][0]["normal"],
            serde_json::json!({"min":2,"max":2})
        );
        assert_eq!(
            a["viewpoints"]["effects"]["rows"][0]["e"]["actors"],
            serde_json::json!(["L"])
        );
        let cut = serde_json::to_value(crate::report::check(
            &source.replace("clock 5min", "clock 1min"),
        ))
        .unwrap();
        assert_eq!(
            cut["agency"]["viewpoints"]["retention"]["status"],
            "not_derived"
        );
        assert_eq!(
            cut["agency"]["viewpoints"]["retention"]["reason"],
            "derivation_limit"
        );
        assert!(
            v["assumptions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|a| a["field"] == "agency.response_alias_exclusions"
                    && a["value"].as_str().unwrap().contains("count=1"))
        );
        let judged = r#"model J version "0.2" {type V=enum {pass,fail}
            actor Original {source document}
            actor Input {capacity 1;capability read()->V {reads [Original];duration fixed 1min;result nondet {pass}}}
            actor Judge {capacity 1;capability inspect(x:V)->V {reads [Original];duration fixed 1min;result nondet {pass}}}
            actor Act {capacity 1;capability post()->V {commits;duration fixed 1min;result nondet {pass}}}
            process P {raw=Input.read();v=Judge.inspect(raw) response_loss sequence [unknown,delivered]
                retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed;
                if v==pass {a=Act.post();end done}else{end no}}
            bounds {cases 1;live_tasks 1;clock 5min} verify safety no_resource_overcommit }"#;
        let j = serde_json::to_value(crate::report::check(judged)).unwrap();
        let rows = j["agency"]["lineage"].as_array().unwrap();
        assert_eq!(rows.len(), 2, "{j}");
        assert!(
            rows.iter()
                .all(|r| r["call"] != "v" && r["independent_sources"] == serde_json::json!([]))
        );
        assert_eq!(j["agency"]["lineage_groups"][0]["judgements"], 2);
        assert_eq!(j["agency"]["lineage_groups"][0]["actors"], 1);
    }
}
