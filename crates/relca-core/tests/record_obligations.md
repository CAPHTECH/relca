# Record acceptance boundary and internal checks

## Fixed public denominator

73 registered source-transformation pairs /146 fixtures;91 runtime and55 static.
All91 legacy-only source skeletons are separately preflighted, including both
runtime sides. Refusals require one primary error at parser/lower, exactly one
W-CUT-0 and no secondary fallback diagnostics. Six literal/name errors also
assert independent byte spans. Engine outcomes exclude lower diagnostics;
reference prepends them. Tests keep those layers separate.

T01–T19/T21–T23/T25 have input pairs or supplemental tests. T09 is the label
metamorphism with a restricted full-Report comparator. T10 uses approval/bypass,
not duplicated display inputs. T13 contrasts binding/current-state guards.
T15 fixes six-value tables, a single-green-false control and an intentionally
corrupted legal Or. T24 mutates member/domain/name fields available before A4.
T10 intentionally reuses T03's distinct approval/bypass sources with a different
explain invocation;73 is a registered-pair count, not73 unique production faults.

## Shared blind spot (U9)

Runtimes see Eq/StateEq/Or, not the source field. They may agree when a legal Or
omits a member. No independent reference field evaluator is claimed. Mixed
radix/projection tables, terminal masses and exact rational probabilities are
authored constants checked before differential comparisons.

## Source cfg(test) populations

Three source tests scan all fixture directories and currently absorb rec-*.
Preserve old numeric denominators/hashes; exclude only the closed list:

1. engine.rs: legacy_census::e_legacy_state_identity.
2. engine/simulation.rs: simulation_tests::s_engine_migration_classification_preflight.
3. syntax.rs: tests::every_fixture_obeys_its_expected_syntax_diagnostics. Use
   record_oracle.json phase data: lower U-SYN is not a parser rejection.

Production source is untouched except the appended fixture_match table. Three
old integration corpus scanners and nine CLI compatibility population scanners
exclude the closed rec list. Unknown rec names fail. Old numeric expectations
and all old compatibility manifests remain unchanged.

## Existing rejection migration and API literals

observed_latent::s_out_record moves from a supported named declaration to an
unsupported inline record. Enum-named-record/PersonId controls remain. Searching
old Rust strings found no other inline enum/record migration. New accepted
declarations live in rec-*; rejection coverage is not simply deleted.

After EnumType.record is added, mechanically add record:None at four literal
sites in tests/differential.rs and one in tests/support/completion_oracle.rs;
the engine.rs source-test literal belongs to implementation. Do not alter values.
The planned RecordShape/RecordField API does not exist on the RED baseline.
The metadata-field continuation is authored in support/record_shape_probes.rs.
Once the API exists, link it as a child module of records.rs (it uses super::hand).
It is deliberately not part of the current executed count; do not call it run.

## Shape/private probes required once the new API exists

These are implementation obligations, not executed coverage claims:

- Corrupt shape fields independently: out-of-range TypeId, self-record, builtin
  PersonId/CASE_ID_TYPE, empty/duplicate names and zero fields. Check product
  overflow before allocation, including unused declarations; never return a
  truncated domain. Existing public tests corrupt members/count/order/names.
- Public record state is ObservedCase/Fixed only. Reject latent/ActorObserved,
  distributions/nondet, integer/case-set metadata, invalid initial ValueId,
  kernel given and atomic return/write at legacy/arrivals/simulation/replay
  entries. Never rely on lower having run.
- Grouped f,g:inline-enum shares one type position; separately written fields
  do not. Metadata must match canonical names and order; never intern same-member
  nominal types. TypeId guards must reject reserved builtin IDs without panic.
- Keep Eq/Hash distinctions for six Pair values, optional none, record bindings,
  pending result and current case alias. Public branch/probability checks do not
  prove private state-key completeness.
- Replay tampering: first accept control, then independently change canonical
  value, ValueId, before/after whole-record write, source location and time.
  Reject altered replay; never verify after erasing record values.

## Dependency-deferred activation (ADR0035/0037)

Integration inputs include T20 initial high/low state invariant,
implication antecedent/consequent, binding-only refusal and unselected-process
same-name alias; T23 state-invariant consumer; S29 record+atomic result/write;
CaseId reserved TypeId and Contains/atomic atom-regression controls. They are not
counted as run on df9b3eb. That observation is limited to that historical state.

## Adopted correction

T25 old-syntax clock0 gives only W-CUT-0, not W-BUDGET-0 under
existing clock-cut semantics. Probability remains[0,1], verdict and safety
inconclusive, terminal completion unknown. Arrivals keep existing
case_outcomes.terminals/properties.cases wire.

## Calibration scope

Proposed production mutations are not execution evidence. Calibration needs a compiling
implementation, one fault at a time, semantic assertion failure and restoration,
not a compile error or parser-only refusal.
