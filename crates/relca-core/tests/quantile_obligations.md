# ADR0049 implementation-side seam obligations

Hand oracle support is NOT evidence that a private reducer was exercised.
Bind **both independent** sides to quantile_private_contract.rs and retain the
fixed quantile_private_oracles.json numerical values.

| Seam | Adapter obligation | Fixed observations |
|---|---|---|
| graph_contract | Translate hand trees into the real first-hit graph/reducer, NOT a new test-side solver. Return requested A/B/D endpoints and flattened values. | T19 direct B and redistribution, T20 two crossing levels/shift, first-hit absorption B=0, min-only/max-only unknown, current-tick cuts, within-closes-B |
| sample_contract | Real Hit/Never/Censored aggregation, pooled across replications/cases. | T07 identified median and missing tail, T14 distinct all/given optimistic bounds, T25 finite latest, no_samples, late abort, known miss |
| world_contract | Observe all accepted case views on selected transitions, not only self traces. No new Eq/Hash/history signature fields. | T24 case2 elapsed1 without case2 trace, no-hit pair, pre-receipt true ignored, unselected hit candidate ignored |
| ir_contract | Parse valid source then forge public IR directly; engine and reference entry defenses independently. No lower shortcut. | p0,negative,>1,raw denominator0,index=len,empty name,cross-namespace duplicate,huge valid rational |
| numeric_isolation_contract | Force only quantile arithmetic bits=1 without changing old numeric flags. | reason truncated; old Outcome/diagnostics/states unchanged |
| sparse_graph_contract | Instrument graph construction; clock=10^12 and breakpoints only. | one graph build,median3; no clock-linear rebuild |

Public T19/T20 DSL is also installed: positive one-minute selector followed by
the declared conditional durations gives the SAME T2/T4 or T2/T6 leaf clocks.
Its old-syntax substrate was independently checked before acceptance installation.
The private seam remains necessary to catch B first-hit absorption and exact
four-endpoint provenance directly rather than relying solely on public output.

Required source linkage is deliberately not invented here. Follow the existing
quality_impl_tests.rs / quality_private_contract.rs adapter precedent, but do not
share the engine quantile CDF/reason reducer with reference/quality.rs. Shared IR,
wire types and immutable metadata do not count as shared numerical algorithms.
F15's shared-dependency fault requires a source dependency review; passing numeric
equality is NOT proof of independent implementations.

## Frozen old source cfg(test) populations

Append the exact quantile_population module under cfg(test), separately from
quality_population; never merge C7 names into the earlier B6 count/fingerprint.
These are test-population boundaries, not numerical expectation updates.

- syntax.rs::tests::every_fixture_obeys_its_expected_syntax_diagnostics, fixture
  read_dir loop at PRE-C7 line4288: C7 runtime source is newly accepted while
  fraction/metrics/nested/structure negatives retain their specified classes.
- engine.rs::legacy_census::e_legacy_state_identity, all_dirs loop near1815 and
  old population filter near1858: exclude exact C7 names from historical census.
- engine/simulation.rs::simulation_tests::s_engine_migration_classification_preflight,
  read_dir/filter near1147/1161: use the exact C7 population, not a qtl prefix.
- lower.rs::metrics_tests::metrics_all_fixture_syntax_and_selected_lowering near
  10092 is met-prefix-limited; do not alter its numerical census.
- quality_impl_tests.rs loops only quality_population; leave its existing numbers.

The workspace census must distinguish actually failing source tests
from inspected-but-unaffected loops. Existing core/CLI integration census tests
and earlier compatibility checkers receive separate exact C7 exclusions without
regenerating earlier manifests or changing their numeric expectations.

## Remaining integrations (not quietly skipped runtime cases)

quantile_deferred.json contains full hand sources/calculations for B6 horizon
T14/T18/T25 and B7 objective T08/T09/T10/T21/quantile-hard. It is a different
population, not counted in installed fixtures. Separate integration tests
also tracks Rule initial truth/configuration/scenario, C6 mutation transport,
A7 waiting/quantile assumption order, B7 species7 compatibility and T24 public
shared-state input. These prerequisites must be resolved before claiming C7 fully
integrated. Draft objective wrappers are not an alternative evaluation wire.

No private attachment, performance threshold, implementation mutation calibration,
or independent dependency review is claimed by authored Stage C RED alone.
