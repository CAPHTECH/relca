//! ADR 0059 static partition and reference validation. Runtime is gated until
//! both interpreters and their collectors implement the same public contract.
use super::*;

impl Lower {
    pub(super) fn resource_time(
        &mut self,
        model: &ast::Model,
        clock: ir::Tick,
    ) -> Option<ir::ResourceTime> {
        let active = model.items.iter().any(|i| match i {
            ast::Item::Calendar(_) | ast::Item::PersonCalendar { .. } => true,
            ast::Item::Resource(r) => r.calendar.is_some(),
            ast::Item::Actor(a) => a
                .items
                .iter()
                .any(|i| matches!(i, ast::ActorItem::Calendar(_))),
            _ => false,
        });
        if !active {
            return None;
        }
        let mut time = ir::ResourceTime {
            overtime_rates: vec![None; self.actors.len()],
            calendars: Vec::new(),
            actors: vec![None; self.actors.len()],
            resources: vec![None; self.resources.len()],
            people: vec![None; self.people.len()],
        };
        let mut names = BTreeMap::new();
        for item in &model.items {
            if let ast::Item::Metrics(spec) = item {
                for row in &spec.items {
                    if let ast::MetricsItem::OvertimeRate { actor, price } = &row.value {
                        if let Some(id) = self.actor_names.get(&actor.value) {
                            let rate = &mut time.overtime_rates[id.0];
                            if rate.replace(price.value).is_some() {
                                self.unsupported("duplicate overtime rate", row.span);
                            }
                        } else {
                            self.error(E_NAME_0, "unknown overtime Actor", actor.span);
                        }
                    }
                }
            }
        }
        for item in &model.items {
            let ast::Item::Calendar(c) = item else {
                continue;
            };
            if names
                .insert(c.name.value.clone(), time.calendars.len())
                .is_some()
            {
                self.unsupported("duplicate_calendar", c.name.span);
            }
            match &c.on_close {
                None => self.unsupported("calendar_close_behavior_required", c.span),
                Some(v) if v.value != "continue" => {
                    self.unsupported("calendar_preemption_unsupported", v.span);
                }
                Some(_) => (),
            }
            let through = self.ticks(&c.through);
            let mut intervals = Vec::new();
            let mut end = 0;
            for row in &c.intervals {
                let from = self.ticks(&row.from);
                let until = self.ticks(&row.until);
                if let (Some(from), Some(until)) = (from, until) {
                    if from != end || from >= until {
                        self.error(E_CAP_0, "invalid_calendar_partition", row.span);
                    }
                    end = until;
                    intervals.push(ir::CalendarInterval {
                        from,
                        until,
                        capacity: row.capacity.value,
                    });
                }
            }
            if intervals.is_empty() || through != Some(end) {
                self.error(E_CAP_0, "invalid_calendar_partition", c.span);
            }
            time.calendars.push(ir::Calendar {
                name: c.name.value.clone(),
                through: through.unwrap_or(0),
                intervals,
                span: c.span,
            });
        }
        // Resolve each explicit occurrence; duplicates must not be collapsed.
        for item in &model.items {
            match item {
                ast::Item::Actor(a) => {
                    let id = self.actor_names[&a.name.value];
                    for field in &a.items {
                        if let ast::ActorItem::Calendar(c) = field {
                            let resolved = self.calendar_reference(
                                c,
                                &names,
                                &time.calendars,
                                self.actors[id.0].capacity.unwrap_or(0),
                                clock,
                            );
                            if time.actors[id.0].is_some() {
                                self.unsupported("duplicate_calendar", c.span);
                            }
                            time.actors[id.0] = resolved;
                        }
                    }
                }
                ast::Item::Resource(r) => {
                    if let Some(c) = &r.calendar {
                        let id = self.resource_names[&r.name.value];
                        time.resources[id.0] = self.calendar_reference(
                            c,
                            &names,
                            &time.calendars,
                            r.capacity.value,
                            clock,
                        );
                    }
                }
                ast::Item::PersonCalendar { person, calendar } => {
                    if let Some(&id) = self.person_names.get(&person.value) {
                        let resolved =
                            self.calendar_reference(calendar, &names, &time.calendars, 1, clock);
                        if time.people[id.0].is_some() {
                            self.unsupported("duplicate_calendar", person.span);
                        }
                        time.people[id.0] = resolved;
                    } else {
                        self.error(E_NAME_0, "unknown calendar PersonId", person.span);
                    }
                }
                _ => (),
            }
        }
        // A single model-wide PersonId mapping filters every Actor that names
        // that person. Never synthesize a distinct executor for a later shift.
        let mut missing = Vec::new();
        for cap in &self.capabilities {
            if duration_max(cap) == 0 {
                continue;
            }
            let actor = &self.actors[cap.actor.0];
            if time.actors[cap.actor.0].is_none() {
                missing.push(actor.span);
            }
            for person in actor.executors.iter().flatten() {
                if time.people[person.0].is_none() {
                    missing.push(self.people[person.0].span);
                }
            }
            for usage in &cap.uses {
                if time.resources[usage.resource.0].is_none() {
                    missing.push(self.resources[usage.resource.0].span);
                }
            }
        }
        missing.sort_by_key(|s| (s.start, s.end));
        missing.dedup();
        for span in missing {
            self.unsupported("calendar_target_unconfigured", span);
        }
        Some(time)
    }

    fn calendar_reference(
        &mut self,
        name: &ast::Ident,
        names: &BTreeMap<String, usize>,
        calendars: &[ir::Calendar],
        capacity: u64,
        clock: ir::Tick,
    ) -> Option<usize> {
        let Some(&id) = names.get(&name.value) else {
            self.error(E_NAME_0, "unknown calendar", name.span);
            return None;
        };
        let calendar = &calendars[id];
        if calendar.through <= clock {
            self.unsupported("calendar_coverage_missing", name.span);
        }
        if calendar.intervals.iter().any(|r| r.capacity > capacity) {
            self.error(E_CAP_0, "calendar_capacity_out_of_range", name.span);
        }
        Some(id)
    }
}
