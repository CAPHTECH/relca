# ADR0037 implementation-side obligations (not executed by the RED author)

Public tests are deliberately compilable against 7f8ee1e. New IR enum arms and
private Stage replay cannot be named in those integration targets yet. Link
`tests/support/state_invariant_private_ir.rs` and
`tests/support/state_invariant_private_replay.rs` from `src/lib.rs` under cfg(test)
after implementing the planned types. These two modules are authored acceptance,
not claimed executed evidence. Adapt mechanical field spelling only if the
approved plan changes; do not weaken assertions to get a green run.

Also link `tests/support/state_invariant_private_state.rs` as an engine child
module under cfg(test). It supplies the concrete monitor Eq/Hash/HashSet test;
its separate fingerprint assertion is structural, not a semantic-kill claim.

Finite additional private obligations:

1. **Monitor identity:** construct two states with identical business values,
   tasks/time/control, different activated/violated bits. Eq must distinguish
   histories. Exercise converging histories and sticky sinv-sticky repair. A
   missing Hash contribution alone can be only a collision: record hash/census
   evidence separately, never call that a semantic kill without discrimination.
2. **Hooks:** instrument receipt (fixed/nondet/probability), initialization,
   start, full finish/commit, cancel request/ack/immediate, refusal, end,
   management wait/wake/tick, and selected simulation/probability transitions.
   Normalize before hashing/insertion/capture; do not create a monitor transition.
3. **Passive case:** update only monitor bits, not Global::store, for ended
   readers. sinv-ended-clean returned completed_at/elapsed/work must stay2 when
   the other case writes at3. Both case IDs violate across histories; not both
   in one history. Stage belongs to writer, target/detail belongs to ended reader.
4. **Bounds and room:** seed/Future/Rejected/range-failed candidates are never
   evaluated. Budget0 produces no receipt witness. A measured received-then-bad
   budget must retain the known violation, with W-BUDGET-0. Reserve before clones;
   no uncharged Global clone or inserted candidate outside clock/live bounds.
5. **Replay:** run authored private tamper module; also forge final assignment
   location, property span and valid writer stage paired with another target
   valuation. Check first-bad prefix, 200000 capture-edge limit and report scope.
6. **Old census:** retain all old state/successor/frontier/fingerprint numbers.
   Never rewrite numeric expectations merely because new fixtures were added.
7. **Integration:** A3 read sets/validators must traverse admitted Contains and
   PersonId atoms; B3 fixes only starts, not equal-time completion order. Deferred
   The CaseId/set integration inputs are outside this denominator.

## Existing cfg(test) populations

The workspace run observed exactly these three src-population failures:

- `syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics`
  (`src/syntax.rs:3152`): the new legal `sinv-static-always` control is rejected
  before C3 is implemented.
- `engine::legacy_census::e_legacy_state_identity` (`src/engine.rs:1864`):
  old remaining-input count expected124, observed248 =124+124 new inputs.
- `engine::simulation::simulation_tests::s_engine_migration_classification_preflight`
  (`src/engine/simulation.rs:1204`): old counts expected(16,148), observed(16,272).

No lower.rs population failure was observed. Route the **closed**
`state_invariant_population::NAMES` to their own
tests or exclude that closed list from an old fixed census; do not use an open
sinv prefix exemption. Preserve old count/fingerprint expectations byte for byte.
The public core differential inventory is a fixed list and must not absorb the
new population. Unknown future sinv names must remain an inventory failure.

## Measurement limits

RED observations establish missing feature entry points only. They do not prove
any new runtime hook, typed-IR refusal, replay tamper rejection, performance,
budget-after-witness threshold or compiled production mutation kill. Complete
these obligations in the implementation worktree and record actual commands,
exit and discriminating assertion for K01–K30 before claiming calibration.
