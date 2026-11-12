# ADR0046 private linkage obligations (acceptance owns the oracle)

The public targets are mutation_default_set and mutation_default_differential.
The production AST editor and comparator are private; the integration suite does
not change crates/*/src. Implementation must link
tests/support/mutation_default_private.rs under cfg(test) in mutation.rs, then
call comparison_cases and ast_cases with adapters into the real implementation.
No second comparator/editor may be written in an adapter. The signatures are
documented beside each function. This is a linkage seam, not an assertion that
these obligations ran during the RED authoring checkpoint.

Finite obligations:

* P01 normalization of source order/duplicates (extend with duplicate independent source).
* P02 same-size but different independent source => independence_credited.
* P03 group split and same-group count reduction => independence_credited.
* P04 missing consumer, group, agency => unavailable, never held.
* P05 empty producer source and non-included source => no_assigned_check.
* P06 count2→3 => detected with held payload2→3, per Q4.
* P07 unexplained subject change => unavailable.
* P08 static invalid/unsupported, runtime unsupported and truncation priority;
  additionally link Authority cases-row disappearance and before dynamic truncation.
* P09 the assigned projection may not erase another consumer's equal payload.
* T17 guard source stays original; full edited AST equality ignoring spans only.
* T22 nested then/else, fork consumer, immediate insertion, and T16 independent edits.
  Add the repeated argument control (args=[e,e], one call is one consumer).
* Defensive VersionCheck-only AST predicate (not legal-source detection strength:
  legal version_check also writes shared state, so removing only that exclusion
  is observationally equivalent while state exclusion remains).

Population rule: mut2-* must be excluded by the closed name table, never
by an arbitrary prefix, from existing historical src cfg(test) fixture censuses.
Expected affected seams: syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics,
engine::legacy_census::e_legacy_state_identity and
engine::simulation::simulation_tests::s_engine_migration_classification_preflight.
Numerical historical expectations must not be increased to absorb new inputs.
Observed RED: engine old124 becomes203, simulation old148 becomes227 (both +79).
The syntax walker also visits the new group, but its current assertion passed;
do not report it as a third failing census. Exclude the group there only if
preserving the historical syntax denominator requires it during integration.
