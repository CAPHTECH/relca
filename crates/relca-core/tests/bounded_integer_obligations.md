# Bounded-integer acceptance boundary and internal checks

## Fixed public denominator

`bounded_integers` owns 28 pairs / 56 inputs: runtime46, static10. Static10 are
F04 default, F05 mixed comparison, F06 latent policy, F12 sibling writers,
F17–F21 classification and F22 integer kernel given, all on the broken side.
The runtime test checks each backend against the hand oracle before comparison.
Engine Outcome does not prepend lower diagnostics; reference Outcome does.
Integer witness/flag payload assertions use public JSON, not future Rust DTOs.
CLI tests fix range witness arithmetic, exact effect span, source-bounded
declaration span, simulation counts, all wrapper slots and shortest explain.

`bounded_integer_boundaries` adds typed comparisons at four consumers, policy
short-circuit/dead/unselected checks, signed/huge domains, distribution validation,
kernel gate, implicit property, direct-IR mismatch and incomplete probability
mass. Hand derivations for these supplemental numbers are in int-two-add/calc.md.

## Existing rejection-test migration census

At base 85c5485 literal searches of crates' Rust tests and DSL fixtures for
bounded state types, integer effects/comparisons, undeclared int/CaseId state and
parameter types, case.id actual arguments and observed reads found no existing
rejection assertion on a newly accepted form or C1–C5. Numeric expectations are
not changed. In particular, unknown state/reads names, bool result/parameter
types and enum inequality/order rejection remain their old errors. The new
`legacy_enum_comparisons_and_unbounded_return_name_errors_are_not_migrated` control
passes against the old implementation. The five new classification pairs are
new inputs outside the compatibility population, not permissions to change old
diagnostics. The independent capture census found eight old E-SYN/E-NAME positions
and zero migration intersections, including nested mutation Report slots.

Only population exclusions in existing integration tests change:

- differential.rs: differential_corpus
- simulation.rs: s_check_ignored_all_148_old_inputs_check_and_diff_json_keys_and_assumptions
- simulation_differential.rs: all_old_models_migration_148dirs_179inputs_160lower_80complete_80nondet_19static
- CLI authority_compatibility.rs: its frozen old population
- CLI responsibility_compatibility.rs: its frozen old population

The first three use the closed 56-input integer helper. The old values/census
constants and all old compatibility manifests remain unchanged.

## Source cfg(test) populations

No production source is changed except fixture_match's acceptance tables.
Three source tests scan all fixture directories and currently absorb int-*:

1. engine.rs, legacy_census::e_legacy_state_identity: close and exclude the int
   group before the old 124-directory denominator; keep old census hashes/counts.
2. engine/simulation.rs, simulation_tests::s_engine_migration_classification_preflight:
   exclude the closed int group from old_dirs/counts, not from integer acceptance.
3. syntax.rs, tests::every_fixture_obeys_its_expected_syntax_diagnostics: classify
   C1/C2/C4/C5 as lower diagnostics, not parser U-SYN expectations; C3 is a parser
   U-SYN and F22's kernel gate is semantic. A blanket unsupported skip is wrong.

## Private invariants that public APIs cannot expose

These are required implementation tests, not claimed completed acceptance:

- engine State and reference Frame: h0 versus h1 must compare unequal; HashSet2.
  Legacy Value(0) and integer0 are not equal. Integer values are in the key;
  witnesses/history must stay outside it.
- engine initial/arrivals initialization and reference initial/arrivals:
  one h:0..2 yields exactly {0,1,2}; two h:0..1 cases yield exactly (0,0),
  (0,1),(1,0),(1,1), regardless of later schedule duplication. Test remaining
  budgets0/1/count−1/count/count+1, before clone/allocation, and never Received
  with an uninitialized value. N=0 has one nondet seed edge; fixed0 has none.
  Huge u64::MAX domains must cut/EnumerationLimit before collecting N+1 values.
- engine Start/Finish bundle paths, kernel finish, arrivals and independent
  reference dispatch/finish: F25 flag=false/n1 remain after failed attempt2;
  resource holders, result publication, normal-completion counters and
  Start/Finish/Commit/Write events remain unchanged. Include a START overflow,
  which must not allocate a resource or emit Start.
- reference/replay.rs `replay` and `replay_choices` are pub(crate), unavailable
  to integration tests. Feed the F02/F03/F25 four-edge range paths; control
  replay succeeds, then independently alter before, operator, operand, attempted,
  time, case_id, write_span and declaration_span: each mutation must return Err
  (StateMismatch/IllegalTransition/ViolationMismatch/SourceMappingFailed as
  appropriate), never Verified with erased/changed steps. Recompute arithmetic
  independently instead of calling engine helpers or trusting attempted payload.
- Noncanonical/direct IR: integer above upper, integer in enum (public transfer
  test already provided), enum in integer, numeric kernel and duplicate implicit
  properties fail closed or uniquely normalize as specified. Outward IntAssign
  remains legal and produces dynamic S violation.
- Responsibility finite-domain enumeration and effect truth analysis must respect
  Budget/Limited and must not assume a normal successor after range stop. The
  public state-condition visitor probe is not an internal enumeration proof.

## Agreed wire spelling

Only bounded-integer models have implicit last property `state_in_range`.
Range-only stop: bounds.truncated=false, bounds.range_violation_stops=true;
no W-BUDGET-0. Optional nonempty `state_range_violations` contains the plan's
16 fields, exact decimal strings, assignment operator `=`, `+=` or `-=`, and
phase start/finish. Legacy witness case_id is null; arrivals IDs are one-based.
`capability` is the capability's local name, separate from actor.

The simulation report includes simulation.range_stopped_replications,
case_outcomes[].range_stopped_count and explain steps[].state_range_violation.
Sample safety is inconclusive/basis sample when no violation, violated/basis
witness otherwise. Range-stopped repetitions are not cuts/deadlocks/terminals.
Explain final attempted finish has values BEFORE the bundle, no success events,
no result, and null property_location. With a single fixed one-minute call:
initial(received), queue, start, advance1, range_violation = four edges, five
stages. Counterexample publication requires successful independent replay.

## Calibration status

Failing tests alone do not verify the semantics. The M01–M38 fault inventory and
minimum12 compile-success/semantic-kill/restore runs require execution evidence;
the presence of assertions alone does not establish calibration.
