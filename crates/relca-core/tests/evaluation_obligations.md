# Comparison evaluation internal seam linkage

These are **internal test obligations, not silently counted
as executed private acceptance**. At fc2a5a9 the original adapters exist in
src/evaluation_tests.rs and src/evaluation_mutation_tests.rs; their presence is
not a claim of rerunning them here. Public integration tests never import a
proposed enum or function that does not yet exist. The fixed hand data is
`support/evaluation_private_oracles.json`.

1. **T14 interval endpoints:** bind all six strict/inclusive controls directly
   to the new interval comparator. A public exact-half pair separately checks
   all four operators. The `[0,1/2] < 1/2` and `[1/2,1] > 1/2` cases are I,not V.
2. **F14 / non-dyadic CP aggregate:** construct a legal simulation report and
   inject the fixed case observations for each of two plans,one arrival,R4,
   confidence3/4. Both hard cells must have exactly
   `[28863/262144,233281/262144]`,m2,alpha1/16,point1/2. Do NOT call the existing
   production interval function to construct the expected value. The separate
   BigInt polynomial test certifies the outward lattice oracle independently.
   `cmp7-nondyadic` now additionally exercises the real public aggregate:the
   ADR0016 SHA256 key contract at seed2 gives2/4 fair initial hits in both plans,
   derived independently by the hand materializer,not copied from a report.
3. **T23 malformed reports and IR:** apply each listed fault in isolation to
   the successful two-plan CP input. Evaluation is absent,U-SEM-0,U/4,no panic,
   no partial evaluation or Agency table. Exercise both numerator and denominator
   inventories,not just the first numerator. `checked_add` must precede any
   interval call. Keep normal aborted/no_samples distinct from malformed data.
4. **T26 definition comparator:** bind the original and each identical/one-end
   deletion record. Identical is undetected/no_worsening,not always detected.
   Preserve numerator,kind,property,within,cohort. Changed gives exactly the two
   assigned definition rows. Install spies at lower,engine,reference and compare;
   all four counts remain0 for each structural kind7 candidate. No mutant ratio
   field or recomputation from original quality is permitted.
5. **T24 budget:** the four fixed limits apply after the ordinary two guard
   mutants. Retain all four kind7 rows even when not_run;one structural
   comparison consumes one shared slot. State skipped sites consume none.
   The C6-specific six-kind order is a deferred integration obligation,not a
   reason to move kind7 before the existing kinds on this base.
6. **Agency F32:** both `ComparisonSide`s carry the responsibilities belonging
   to their own selected report,including meaningful differences and derivation
   limits. Public `eval-agency` checks the independent empty M inventory and
   transport/direction. An empty M responsibility list alone cannot detect a
   `None`-instead-of-responsibilities fault;use `cmp7-responsibility` too.

No engine/reference transition,quality reducer,waiting or quantile change is
authorized by these obligations. Scope crossings and B6 horizon reasons are
separately enumerated in the integration checks.

## Review additions (2026-10-29)

The exact new cases and hand rationale are in evaluation_review_fixes.md.
Implementation must cfg(test)-link these acceptance-owned adapters:

- lib.rs: path ../tests/support/evaluation_review_private_spy.rs.
- compare.rs: path ../tests/support/evaluation_review_private_compare.rs.
- mutation.rs: path ../tests/support/evaluation_review_private_mutation.rs.

Expand the existing four-slot spy to seven slots while preserving indices0..3,
append separate engine simulate4/reference simulate5/trace explore6 hooks, and
update the original src four-slot zero assertions accordingly. The new controls
exercise each entry separately, including completed simulation and real explain.
The new T23 counterpart requires evaluate Err to remove checked table claims,
not just discard evaluation/Agency output. Adapter code is acceptance; production
and instrumentation changes remain implementation-owned.
