# ADR0045 implementation-side seams (not claimed executed by authored RED)

The public acceptance and closed oracles are `quality_cohort.rs` and
`support/quality_*`. These tables do not establish production coverage.
Attach the following private tests there without changing numeric expectations.
Visited source is not completed verification. Every item below is **unverified**
until the named private seam runs against the implementation.

| ID | seam and required assertion | calibration |
|---|---|---|
| QP01 | Feed both independent reducers `[0,0,1,1]`, `[0,0,0,1]`, `[0,1,1,1]`, `[0,1,0,1]`. First is known schedule width; others unknown. Never reconstruct unknown from outer min/max. | F1,F21 |
| QP02 | Unknown cut kinds Clock, LiveTasks, StateBudget, Numeric, Pending, Range and a future unknown kind mean truncated; Horizon only means undetermined. Ignore stale Pending on already expanded nodes. | F6,F20,F33 |
| QP03 | Two replication observations N=[1,0],D=[1,3],all U=0 give point1/4; samples2,N sample(1/2,0,1),D sample(2,1,3). D=[1,1] gives1/2. | F9 |
| QP04 | QP03 with U_N=[0,1] gives point null, bounds[1/4,1/2]. With U_D=[0,1] only, bounds[1/5,1/4]. N unknown,D=0 uses truncated/undetermined before zero-denominator. | F5,F10 |
| QP05 | Inject invalid public IR separately into engine and reference: Some(empty Spec), duplicate/reserved property name, duplicate metric name, empty End set, duplicate/out-of-range End ID, out-of-range numerator and denominator property ID, malformed State type/value/binding/Cut. No panic, fabricated0 or ignored declaration; engine U-SEM-0,reference SemanticAbort/U-SEM-0. Include an unreferenced bad property. | F19,F32 |
| QP06 | Census/State Eq+Hash and old Outcome Debug for T13 and T22.1 match the explicit no-quality source. Quality is not uses_probability, not a fake probability_query, not additional random keys. | F15,F26,F27 |
| QP07 | Engine and reference have separate aggregation/reason reducers. For each runtime fixture, serialize each new Outcome.quality and compare **each first to expected.json** and then each other. Public test already compares whole Outcomes but currently cannot name the not-yet-existing quality field without causing compile failure. Add this direct-field check when field lands. | F16 |
| QP08 | Mutate wire one field at a time before production comparison projection: missing/extra property/case/cohort/outcome, wrong order, duplicate key, wrong case ID/arrival/definition/mode, invalid basis/reason/noncanonical rational, missing null, undeclared quality. Fail U-SEM-0,not_applicable cannot hide computation failure. | F17,F25 |
| QP09 | Artificial small-bit failure in **quality only** must not update existing Facts.numeric, truncated, old verification, metrics, diagnostics or root verdict. | F34 |
| QP10 | No completed simulation samples: all aggregates null/no_samples,samples0; early/late refusal all aggregates null/aborted,samples0,case hits/unknown/trials0. Checked calculation inventory moves to unchecked. | F29 |
| QP11 | The terminal union target is evaluated once. One nondeterministic a/b case gives1,not[0,2]. within is arrival-relative and inclusive. | F13,F14 |
| QP12 | Overflow row is never duplicated as ordinary end. End hit freezes business outcome, not state-property observation: optional post-End contest route still sets state. | F35,F36 |

## Existing cfg(test) source populations needing explicit closure

Do not regenerate census fingerprints or increase old numerical constants to
absorb qual fixtures. Use `tests/support/quality_population.rs` for exact new names.

* `syntax.rs::tests::every_fixture_obeys_its_expected_syntax_diagnostics`,
  all-fixture loop at former line3479: old parser population.
  Old inline `property p = x` / `metric m = elapsed` remain unsupported, so these
  two old tests must **not** be migrated to accepted quality declarations.
* `engine.rs::legacy_census::e_legacy_state_identity`, all_dirs loop at
  former1801: exclude new closed
  population from the frozen legacy count/fingerprint, leave other fields intact.
* `engine/simulation.rs::simulation_tests::s_engine_migration_classification_preflight`,
  all-fixture loop at former1058: same old population.
* `lower.rs::metrics_tests::metrics_all_fixture_syntax_and_selected_lowering`
  filters met-* already; no new qual inclusion or numeric migration is needed.
* Other loops at engine.rs187/279/891/2164 are duration/state/prob/exec prefix-limited;
  inspect rather than blanket-changing all fixture scans.

Final workspace results identify which of these actually fail in this snapshot.
The population boundary must not change old numerical expectations.
