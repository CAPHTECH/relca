use super::*;

impl Global {
    pub(super) fn queued_for_performance(
        &self,
        k: usize,
        j: usize,
        actor: ir::ActorId,
        m: &ir::Model,
    ) -> u64 {
        self.cases
            .iter()
            .enumerate()
            .filter_map(|(case, slot)| match slot {
                Slot::Admitted(c) => Some((case, c)),
                _ => None,
            })
            .flat_map(|(case, c)| {
                c.local
                    .tasks
                    .iter()
                    .enumerate()
                    .map(move |(task, t)| (case, task, t))
            })
            .filter(|(case, task, t)| {
                (*case, *task) != (k, j)
                    && t.lifecycle == Lifecycle::Queued
                    && m.capabilities[t.capability.0].actor == actor
            })
            .count() as u64
    }

    pub(super) fn freeze_performance(
        &self,
        k: usize,
        j: usize,
        executor: Option<ir::PersonId>,
        m: &ir::Model,
    ) -> EngineResult<Option<ir::FrozenPerformance>> {
        let Slot::Admitted(c) = &self.cases[k] else {
            return Err("invalid_q6_ir".into());
        };
        let cap = &m.capabilities[c.local.tasks[j].capability.0];
        let Some(source) = &cap.performance else {
            return Ok(None);
        };
        let profile = m.performances.get(source.profile).ok_or("invalid_q6_ir")?;
        let q = self.queued_for_performance(k, j, cap.actor, m);
        let mut choices = profile.rows.iter().enumerate().filter(|(_, r)| {
            r.executor == executor
                && (!profile.queued || r.from <= q && r.until.is_none_or(|end| q < end))
        });
        let row = choices.next().ok_or("invalid_q6_ir")?.0;
        if choices.next().is_some() {
            return Err("invalid_q6_ir".into());
        }
        Ok(Some(ir::FrozenPerformance {
            profile: source.profile,
            row,
            executor,
        }))
    }
}
