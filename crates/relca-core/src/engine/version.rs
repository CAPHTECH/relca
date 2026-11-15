//! Result identity is path-local and exists only when explicitly requested.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct Examined {
    binding: ir::BindingId,
    version: ir::BindingId,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
struct Started {
    args: Vec<Option<ir::BindingId>>,
    examined: BTreeMap<ir::ClauseId, Option<Examined>>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub(super) struct History {
    // Only copies need an alias; every call (including a refusal) produces its
    // own identity. Copies store the resolved origin, including nested merges.
    aliases: BTreeMap<ir::BindingId, ir::BindingId>,
    started: BTreeMap<ir::BindingId, Started>,
    latest: BTreeMap<ir::ClauseId, Examined>,
}

impl History {
    fn origin(&self, b: ir::BindingId) -> ir::BindingId {
        self.aliases.get(&b).copied().unwrap_or(b)
    }
    pub(super) fn adopt(&mut self, destination: ir::BindingId, source: ir::BindingId) {
        self.aliases.insert(destination, self.origin(source));
    }
}

fn call(body: &ir::Block, b: ir::BindingId) -> Option<&ir::Call> {
    body.iter().find_map(|s| match &s.kind {
        ir::Stmt::Call(c) if c.binding == b => Some(c),
        ir::Stmt::Fork { branches, .. } => branches.iter().find(|c| c.binding == b),
        ir::Stmt::If {
            then_body,
            else_body,
            ..
        } => call(then_body, b).or_else(|| call(else_body, b)),
        _ => None,
    })
}

pub(super) fn event(
    s: &mut State,
    event: Bundle,
    binding: Option<ir::BindingId>,
    m: &ir::Model,
) -> EngineResult<()> {
    let (Some(h), Some(b)) = (&mut s.version, binding) else {
        return Ok(());
    };
    match event {
        Bundle::Start(cap, _) => {
            if !m.contracts.iter().any(|c| matches!(c.kind, ir::ContractKind::SameVersion { first, second } if first.0 == cap || second.0 == cap)) { return Ok(()) }
            if m.site_contest(b).is_some() {
                return Ok(());
            }
            let c = call(&m.process.body, b).ok_or("same_version Start missing call")?;
            let mut begun = Started {
                args: c
                    .args
                    .iter()
                    .map(|&arg| (arg != ir::CURRENT_CASE).then(|| h.origin(arg)))
                    .collect(),
                ..Started::default()
            };
            for (i, c) in m.contracts.iter().enumerate() {
                if let ir::ContractKind::SameVersion { second, .. } = c.kind
                    && second.0 == cap
                {
                    let id = ir::ClauseId(i);
                    begun.examined.insert(id, h.latest.get(&id).cloned());
                }
            }
            h.started.insert(b, begun);
        }
        Bundle::Finish(cap, result, _) => {
            let Some(begun) = h.started.remove(&b) else {
                return Ok(());
            };
            if !crate::same_version::normally_completed(&m.capabilities[cap.0], result) {
                return Ok(());
            }
            for (i, c) in m.contracts.iter().enumerate() {
                let ir::ContractKind::SameVersion { first, second } = c.kind else {
                    continue;
                };
                let id = ir::ClauseId(i);
                if s.monitors[i].evidence_issue.is_some() {
                    continue;
                }
                if second.0 == cap {
                    let version = begun
                        .args
                        .get(second.1)
                        .copied()
                        .flatten()
                        .ok_or("same_version argument missing")?;
                    let examined = begun.examined.get(&id).and_then(Option::as_ref);
                    s.monitors[i].activated = true;
                    s.monitors[i].violated |= examined.is_none_or(|a| a.version != version);
                }
                if first.0 == cap {
                    let version = begun
                        .args
                        .get(first.1)
                        .copied()
                        .flatten()
                        .ok_or("same_version argument missing")?;
                    h.latest.insert(
                        id,
                        Examined {
                            binding: b,
                            version,
                        },
                    );
                }
            }
        }
        Bundle::CancelAck(..) => {
            h.started.remove(&b);
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legacy_state_has_no_value_version_history() {
        let ast =
            crate::syntax::parse(include_str!("../../../../fixtures/serial-time/model.relca"))
                .unwrap();
        let m = crate::lower::lower(&ast).unwrap();
        assert!(State::initial(&m, 0).version.is_none());
    }

    #[test]
    fn value_version_history_census_for_merge_and_parallel_models() {
        let out = std::process::Command::new("python3")
            .args(["-B", "crates/relca-core/tests/same_version/inputs.py"])
            .current_dir(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .output()
            .unwrap();
        assert!(out.status.success());
        let cases: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
        for (name, expected) in [("a3_recheck", (41, 40, 41)), ("a5_parallel", (18, 20, 18))] {
            let source = cases[name]["source"].as_str().unwrap();
            let plain = source
                .lines()
                .filter(|s| !s.starts_with("contract V {"))
                .collect::<Vec<_>>()
                .join("\n");
            let model = |src| crate::lower::lower(&crate::syntax::parse(src).unwrap()).unwrap();
            let m = model(source);
            assert!(State::initial(&m, 0).version.is_some());
            let old = super::super::quantile_private_census(&model(&plain));
            let new = super::super::quantile_private_census(&m);
            eprintln!("VERSION-CENSUS {name} old={old:?} new={new:?}");
            assert_eq!(
                old, expected,
                "{name}: measured legacy visited/generated/held"
            );
            assert_eq!(
                new, old,
                "{name}: same_version must preserve this measured census"
            );
        }
    }
}
