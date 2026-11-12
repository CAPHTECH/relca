# CaseId/set acceptance checks

The oracle is the approved A3 plan and each `aset-*/calc.md`, not output from a
future implementation. `generate_caseset_fixtures.py` does not execute Relca.
The closed population is 91 pairs / 182 directories (P01–P23, P06b, Cut R,
the exhaustive probability counterpart, and 66 separately isolated static pairs).
All positive skeletons are source-bound in `caseset_legacy_controls.json`.
That preflight checks old surrounding grammar, not future semantics.
The closed execution denominator is 104 ordinary runtime inputs (all require
reference Complete), 72 static refusals, four sample inputs and two comparison
inputs. The separate derived zero-duration input is engine-only, not a silently
excluded member of those 104 ordinary inputs.

## Runtime entry points

`caseset_atomic.rs` requires both independent interpreters to agree with the
hand time/work before comparing them to one another. It asserts the reference
lower-diagnostic prefix before stripping it; it never strips that prefix from
engine outcomes. Zero duration is engine-only, with an explicit reference
exclusion. The samples and comparison configurations have dedicated entry tests.
Explain inputs have no configuration selector.

The typed public-IR tests deliberately obtain existing storage tags from lowering
and then change one field. Future-IR-specific field tests are separate below.
`State/Global` equality is checked by distinct HashSet entries, not a claim that
all unequal states must have unequal hash fingerprints. A hash-only performance
mutation may legitimately survive if Eq still keeps states distinct.

P13 is a **then-branch commit**: gate=false at start, Toggle finishes at1 and
writes true, atomic finishes at3. Its negative control keeps false. P01 covers
else-branch commit. Both legacy and one-arrival engine/reference routes are tested.
The static fork-target pair uses two atomic bodies, not ordinary finish writes,
so omitting only atomic footprints cannot be rescued by legacy finish footprints.

## API faults vs valid DSL controls

P18's DSL pair changes only the replication declaration; both exhaustive models
are valid controls. Its actual negative API inputs are the adjacent
`aset-compare-notes.broken-replications/wire_faults.json`: unknown word, duplicate
word, empty list and null, plus a separate wrong-property mutation in the test.
Each must be refused by both exhaustive and simulation projections.

P19's DSL pair changes only wait3→wait2, giving complete violations at5/6→4/5.
Its actual negative replay inputs are adjacent `replay_faults.json`: descending,
duplicate, zero, out-of-domain, valid-but-wrong and stringified sets. The private
test also changes the initial empty set to a valid-but-wrong singleton.
These wire faults cannot be expressed as Relca comments or extra model syntax.

## Required internal linkages

Append to `src/engine/trace.rs`:

```rust
#[cfg(test)]
#[path = "../../tests/support/caseset_private_trace.rs"]
mod caseset_private;
```

Append to `src/engine/arrivals.rs`:

```rust
#[cfg(test)]
#[path = "../../tests/support/caseset_private_arrivals.rs"]
mod caseset_private;
```

Append to `src/lib.rs`:

```rust
#[cfg(test)]
#[path = "../tests/support/caseset_projection_internal.rs"]
mod caseset_projection_internal;
#[cfg(test)]
#[path = "../tests/support/caseset_ir_internal.rs"]
mod caseset_ir_internal;
```

These four support modules are authored but are not linked in this acceptance
worktree. Their field spelling follows the approved proposed IR. Mechanical
adaptation to the implemented spelling may not change expected numbers, branch
meaning, invalid-IR refusal, or the requirement for independent replay.
The private test uses Deserialize as a convenient display-value constructor;
if the chosen display type is Serialize-only, mechanically construct those same
Scalar/CaseSet values in the private test rather than widening a public API.
A closed decoder may reject malformed arrays before replay. Valid-but-wrong
sets must still be rejected against the independent replay state. Stage itself
need not gain Deserialize. Do not widen a public replay API.

Private obligations include:

* shared final `{1,2}` and late receipt `[0,3]` preserving `{1}` at receipt3;
* cancelled atomic has no insert while a start insert survives cancellation;
* overflow publishes none of insert/registered/result/commit;
* reference replay rejects malformed and valid-but-wrong initial/final sets;
* rejected normal finish has two finishes but just one Commit in the P01 prefix;
* bit63 displays `[64]`; cardinality is exactly 18446744073709551616;
* set payload alone distinguishes legacy State and arrivals Global identity;
* Contains alone is classified as reading shared observed state;
* malformed set/atomic/parameter/executor public IR aborts without panic;
* private projection consumes only the one known note at the right IR property.

The public OR-guard race has case1 end2 and case2 end3 or4. It kills moving
**all shared guards** into the management closure. It does not, by itself,
kill omission of the Contains arm because its scalar sibling also reads shared
state. The direct `reads_shared_observed` test is separate. The private arrivals
candidate-layer test reaches case1's Contains-only branch at2 while case2's Gate
finish is due at2. From one and the same graph node it requires both alternative
actions, not observations from different management phases. This private test
is authored, not yet linked or calibrated; do not infer its outcome from the OR
case's per-case time ranges.

## Existing src tests that need closed-population follow-up

Use `support/caseset_population.rs::fixture`, not an open-ended prefix exemption.
All existing numeric counts/fingerprints stay unchanged.

1. `engine.rs::legacy_census::e_legacy_state_identity`: exclude the closed aset
   group alongside the existing aobs/percase/integer/resource groups.
2. `engine/simulation.rs::simulation_tests::s_engine_migration_classification_preflight`:
   exclude exactly those new names from its old denominator.
3. `syntax.rs::tests::every_fixture_obeys_its_expected_syntax_diagnostics`:
   give aset its own finite population; a U-SYN expectation can be lowering-only,
   so do not assume every statically unsupported fixture must fail parsing.
4. Existing parser unit coverage of case.id arguments must now use case.other
   for unsupported-member coverage. Existing lower parameter coverage must use
   PersonId for the unsupported parameter type. CaseId/PersonId declared-type
   precedence is no longer an old accepted alias case; the new builtin-conflict
   pairs require U-SEM-0. Keep all unrelated integer/boolean expectations.

The broad integration populations in differential.rs, simulation.rs and
simulation_differential.rs already exclude the closed aset list in this patch.
The migrated CLI exact diagnostic table expects PersonId and case.other, with
the original source bytes retained in the new compatibility manifest.

## Closed migration diagnostic sentences

Authorized answers 1 and 3 allow exactly these replacement-source transitions:

| migrated fixture | old binary | independently fixed A3 diagnostic |
|---|---|---|
| int-class-param.broken-caseid | E-NAME-0 / invalid3 | U-SYN-0 / unsupported4; `PersonId parameter type is unsupported`; PersonId token span |
| int-class-caseid.broken-reference | E-SYN-0 / invalid3 | U-SYN-0 / unsupported4; `unsupported syntax: case.other argument`; entire case.other span |
| aobs-reserved.broken-set | U-SYN-0 / unsupported4 | same code/verdict/exit; `unsupported syntax: set element type is unsupported`; PersonId element token span |

These are independent spec/source-byte expectations, not observations of new
output. All other diagnostic fields, other rows, stderr lines, metadata, source
and invocation bytes remain frozen. Original CaseId/set<CaseId> inputs are also
executed by new acceptance: two become end-only0/0 satisfied, and passing case.id
to an enum parameter becomes E-TYPE-0.

## Calibration is not done by authoring RED tests

The F01–F42 fault inventory requires execution evidence. Link internal seams,
run controls, then inject one compilable semantic fault, observe the mapped assertion, restore,
and record the control. Mark unexecuted or surviving faults as unresolved.
