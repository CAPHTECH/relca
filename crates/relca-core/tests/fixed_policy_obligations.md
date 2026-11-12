# Fixed-policy internal test seams

Stage C. These are **unexecuted obligations**, not passing tests. The acceptance
role cannot edit `crates/*/src`, except the fixture-name table. Public acceptance
is in `fixed_policy.rs`; CLI selection/compare/diff/explain/mutation acceptance is
in the CLI target of the same name. All 37 exact pairs / 74 directories are
closed by `support/policy_pairs.json` and `support/policy_names.json`.

The private probes below must be installed under `cfg(test)` by the implementation
role. Do not replace them by a successful ordinary explain: that cannot detect
removal of the replay policy check. The intended hand oracles follow the adopted
plan and design v0.2 §22.3, not generated engine output.

| ID | Seed / exact hand assertion | Entry / single faulty contrast |
|---|---|---|
| CENSUS-1 | One received case, two Queued calls in the same capacity2 Actor, two idle executors, time0, same enqueue time; both duration3. Legacy4 successors, call_order2, assign2, both1. After first start, exactly1 start remains. At3 retain both Finish orders. | `engine/arrivals.rs` transition with Room1 must generate the sole selected successor without Budget; independently `reference/arrivals.rs`. Generate-all-then-filter must fail this probe. Compare State keys before/after metadata erasure, no rank/history in state. |
| CLASS-1 | Lower pol-separation, pol-cross-class-person, pol-permission. Erase every ActorMetadata; separately change only metadata.control to another valid class. Outcome and state/generated/held census identical. | Extend a **new** pol-only census, not the historical 24/4 metadata census. Public Outcome erasure is already in fixed_policy.rs. |
| IR-1 | Valid extended pol-separation IR is the positive. Independently mutate classes to None; append ActorId(len); duplicate an ID within a class; place one Actor in two classes; omit one Actor. Each must yield all properties Unsupported and U-SEM-0 from engine; reference stop SemanticAbort. | Validate engine/reference independently. Include zero actors and all-environment/empty-policy valid classifications as positive controls. With no extensions classes must be None. Metadata is not a validation source. The pre-B3 type cannot represent these corruptions, so do not declare this obligation discharged by a parser failure. |
| ORDER-1 | Swap all BindingIds0/1 consistently in pol-tie-actor (binding table, calls, join matches, tie IDs), preserve body order: result3/completed3..4/work3..4. Reverse body branch order without changing tie: result5/completed5..6. | Public renumber/reverse test supplied. Additionally Capability declaration permutation must not select a different first call. Contest synthetic sites are a suffix, not binding indices. |
| REPLAY-1 | pol-separation positive has c1 enter then c2 approve. Give its private global_start a Stage for Clerk=c2 while both idle: IllegalTransition. A matching c1 stage succeeds. | Do not mutate source hash/span or choose a different model. Disable only replay eligibility: illegal c2 must now succeed so the negative assert fails. Valid c2->c2 replay in the **bad** fixture is separately tested publicly. |
| REPLAY-2 | At0 in pol-tie-actor, y-first is illegal; at0 in pol-resource-hol, B-first is illegal. Minimum Refuse candidate must be selected before the next policy Start. | Direct private `global_start` and `global_instant` probes, then projection-consistent whole-trace tampering. Require IllegalTransition, not ModelMismatch/EventMismatch. Refuse executor=None is valid if at least one compatible person is idle; reject Refuse if no person is idle. |
| REFUSE-1 | Two Queued policy calls, smaller key requires=false, later key runnable. Exactly one action/one effect is Refuse; next state may Start later call. Refuse occupies no Actor/person/resource and has work0. | Compare candidate count, Room charge, Stage length, holders. Removing requires from pol-refuse separately produces attempted2/upper1/time2/case2, not a normal terminal. Skip-false or refuse+start-in-one-successor must fail local assert even when final values coincide. |
| LATENT-1 | pol-latent joint initial tuples TT,TF,FT,FF each1/4. For every tuple require case1 Start0/Finish2 before case2 Start2/Finish4. FT is the decisive adversarial control. | Inspect private initial worlds/terminal paths. Marginal bad1/2 alone does not detect hard-first; disable key latent independence to make FT fail. No business condition may read latent. |
| LABEL-CUT | pol-label-cut completes at clock2 with fixed-policy, clock1 has chance from initial coin and nondet B result but cuts before A kernel, so full-information row remains. | Public clock pair supplied. Add private Room/numeric/range/semantic-abort controls after an observed chance+nondet prefix. Do not create a scheduler row when the old boolean was false; unknown future policy is conservative full-information. |
| TIE-ACK | pol-tie-ack only A qualifies at2. B ack3 or4 creates2 adoption children but only1 winner: fixed-policy. pol-single-winner tie nondet also remains fixed-policy. | Winner count must be inspected before ack expansion, not inferred from flattened children. Two qualifying winners (pol-tie-nondet bad) remain full-information. |
| CONTEST-1 | Copy ctx-contest-resource/origin and their exact bad pairs, add both extensions (public differential already enumerates all4). Route default remains environment; explicit control policy enables first_idle. | Seed2 simultaneous contest routes with synthetic sites after normal call count; lower contest ordinal wins. No process.bindings[synthetic] access. Replay must reject reversed route order; contest filed/declined environment choice remains, so no simulation-success claim. |
| MUTANT-1 | Lower/parse pol-refuse; actually generate >=1 guard mutant. Every mutated AST retains the complete Policy item byte-equivalent to original; lower recomputes ranks/classes from mutated body. | `mutation.rs` Candidate::apply clone seam, do not add policy handling to Agency. CLI supplied test compares policy.queue/classes/schedule in original and every generated report; AST comparison itself remains private. |

Concrete REFUSE-1 seed: time0, one received case, no Running tasks or holders;
fork order is Ledger.post at binding0, then Tail.work at binding1. Both entered
the queue at0. Ledger has capacity1 and shared observed posted=1 in0..1;
post requires posted==0 else rejected. Tail has capacity1, no requires, duration2.
Both actors are policy and then call_order is enabled. Keys are (0,1,0) and
(0,1,1): the sole first action is Refuse(binding0), with work0 and empty holders.
Its successor still has binding1 Queued; only the next edge starts Tail at0,
which finishes2. Use Room1 at the settled transition seam, not global budget1
where initialization/management already costs room. The contrast either skips
binding0 or emits Start(binding1) on the same edge: both must fail the action,
effect, holder, Stage and Room assertions. In the named variant give Ledger
executors[p]; Refuse.executor=None remains legal while p is idle, but the same
Stage is illegal if a separate Running task holds p. The false requires does
not make an occupied executor available.

## Historical populations touched by adding pol fixtures

Do not adjust their numeric expected censuses or old hashes. Include the **exact**
policy name list via `policy_population::fixture`; unknown pol names panic.

* `syntax.rs::tests::every_fixture_obeys_its_expected_syntax_diagnostics`
  (read_dir near3061): pol positive needs new syntax acceptance; static negatives
  must be classified independently, not the historical parser denominator.
* `engine.rs::...::e_legacy_state_identity` (read_dir near1787): exclude the closed
  pol list from the old population, preserving its old numeric fingerprints.
* `engine/simulation.rs::simulation_tests::s_engine_migration_classification_preflight`
  old entire-corpus population (read_dir near1057): exclude
  the exact pol list; `fixed_policy` owns the new simulation denominator.
* Prefix-only censuses duration-,state-,prob-,exec- and lower's met- population
  do not include pol and should not change. Existing metadata 24/4 is unchanged.

The former observable rejection control is separated in
`tests/arrivals_fifo_queue.rs` to `partial_observation`, still U-SYN-0. It did not
change any numeric expectation, old fixture model/calc, or old manifest. Historical
integration population exclusions include the exact new list only.

## Completion limit

No private execution or production fault injection was done by the acceptance
role. Stage C parser RED does not verify any timing, policy classification,
noninterference, replay restriction, Room charge, or mutation strength above.
After implementation first obtain GREEN, then apply one fault at a time, require
the named assertion to fail (not compilation), restore and obtain GREEN again.
