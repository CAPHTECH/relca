# Actor-state test seams and bounded claims

All hand expectations live in the aobs fixture calc.md files and the tests, not
captured analyzer output. Direct-IR tests obtain the ActorObserved tag
from a valid Actor state declaration and mutate one logical property at a time.

## Internal cfg(test) linkages

The following crate-private support modules require direct linkage.
Public explain cardinality is not expanded to run these tests.

Append to crates/relca-core/src/engine/trace.rs:

```rust
#[cfg(test)]
#[path = "../../tests/support/actor_observed_private_trace.rs"]
mod actor_observed_private;
```

Append to crates/relca-core/src/engine/arrivals.rs (not engine.rs):

```rust
#[cfg(test)]
#[path = "../../tests/support/actor_observed_private_arrivals.rs"]
mod actor_observed_private;
```

Both modules were compiled and clippy-checked in an isolated source copy inside
this worktree, with only these linkages added. The final 12-test private suite
observed 11 RED tests at unsupported state and one GREEN path-enumerator control
on the existing arrivals-staggered graph. This is not a claim that private
Actor-state assertions after the missing parser gate have executed or passed.

### Trace tests: calls, assertions and arithmetic

The trace module calls engine::trace::explore, walks Capture.edges (using canonical
aliases and a path-local cycle guard), numbers stages, and calls
reference::replay::replay_choices with the original private choice provenance.
It does not globally deduplicate complete paths into marginal case envelopes.
Its enumeration bound is explicit; reaching it is a test failure, not success.

* aobs_joint_complete_paths_are_exactly_posted_denied_and_denied_posted:
  every complete F05 joint path has exactly one committed Finish at1 and one
  zero-work Refuse at1, distinct case identities, both Ends at1, final posted1.
  Joint end-pair set is exactly (posted,denied) and (denied,posted).
* aobs_read_write_same_tick_has_both_complete_joint_histories:
  F17 case1 Ends at2; case2 Ends at3 after Finish-before-Branch, or at4 after
  Branch-before-Finish. Both actions occur at2. Assert both joint pairs and both
  boolean orderings, not just case2's extrema.
* aobs_range_prefixes_for_both_writer_orders_replay_independently:
  select a state_in_range target separately for case1 and case2 from F05's broken
  graph. Reference replay must accept each prefix. The other case has exactly
  one normal Finish, and the selected case attempts1+1=2 at2 without Finish.
* aobs_replay_rejects_single_value_visibility_name_span_missing_duplicate_and_case_tamper:
  F01 broken, legacy and one-arrival routes, valid replay first; alter just one
  Actor.x value, visibility, name, declaration span, missing/duplicate observation,
  or case identity. Each alteration must be rejected by independent replay.
* aobs_late_admission_keeps_current_shared_snapshot_and_rejects_stale_other_frame:
  F09 plus a positive-time observation Probe after the refusal. First post1,
  late arrival3 sees current1 and refuses with work0, Probe3..4 violates a
  deliberately reversed invariant. Replay case2 prefix; reject resetting its
  snapshot to0, leaving case1's writeback stale, or swapping transition order.
* aobs_start_is_visible_in_the_other_case_branch_snapshot:
  F06 case1 Start0 publishes busy=true and case2 Branch2 sees true, on every leaf.
* aobs_cancel_ack_preserves_start_and_omits_finish_in_the_actual_snapshot:
  F10 CancelAck1 has started=true/finished=false and no Slow Finish.
* aobs_per_case_observations_do_not_exist_before_each_admission:
  F20 Initial has no local observations; per-case Initialize value events are
  flag=true/answer=none at0 and3, with observed visibility and distinct case IDs.
* aobs_path_enumerator_control_runs_the_existing_global_graph:
  exercise the same graph walker on an already-supported three-case fixture.

### Arrivals tests: state identity and initialization census

* aobs_global_shared_slot_alone_changes_eq_hash_identity calls initial(m,2),
  changes only shared posted0→1, and requires unequal states, two HashSet entries,
  and different DefaultHasher fingerprints. Eq alone would not kill removal of
  the shared field from Hash, so the direct hash assertion is intentional.
* aobs_graph_root_has_one_fixed_bank_not_per_case_seed_product requires the
  root to contain exactly one posted0 bank and two Future slots. Removing the
  declaration for a control changes payload only, not time/cursor/phase/slots.
* aobs_fixed_bank_adds_zero_nodes_generated_or_held_to_control_only_graph calls
  compatibility_counts on two end-only models differing only by fixed bool and
  optional<bool> Actor state. Node/generated/held tuples must be identical.
  These counts are structural controls, not a snapshot learned as a hand oracle.

## Closed source-test populations

Use support/actor_observed_population.rs's closed fixture(name) predicate. Unknown
aobs names must panic rather than gain an open-ended prefix exemption. Preserve
all old numeric expectations and fingerprints.

1. engine.rs::legacy_census::e_legacy_state_identity
   (all_dirs filtering near the existing integer/per_case/authority filters):
   exclude the closed 48 names before asserting the old 124-dir population.
2. engine/simulation.rs::simulation_tests::s_engine_migration_classification_preflight:
   exclude the same closed 48 before the frozen old simulation denominator.
3. syntax.rs::tests::every_fixture_obeys_its_expected_syntax_diagnostics:
   separate this new group from the frozen old count. Parse valid new fixtures,
   but do not infer parser refusal from every U-SYN expectation: enum ordered
   comparisons and external ownership can be syntactically valid and rejected
   by lowering. The new acceptance's static refusal tests cover both stages.

The duration/state/exec/prob prefix-specific censuses and lower's metrics-only
scan do not admit aobs names; they need no new numeric expectations. Broad old
integration populations in differential.rs, simulation.rs and simulation_differential.rs already exclude the
closed group in this patch. Four existing CLI compatibility censuses likewise
exclude exactly the new names; their manifests and numbers are unchanged.

## Calibration boundary

A test being present, compiling, or failing at a parser gate does not establish mutation detection.
Inject one compilable semantic fault at a time after GREEN, record the precise
assertion killed, restore, and retain any survivor as unresolved. Private support
tests must be linked before claiming M17/M18/M22/M23 or replay-tamper coverage.
