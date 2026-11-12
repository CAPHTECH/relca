# ADR 0047 review-fix acceptance, expectations fixed before fixes

Basis: ADR 0047 grammar/compatibility/meaning, approved plan 2.2/2.4/2.6,
and the B7 review-1/review-2 rulings dated 2026-10-29.
This is an inline crossing population, not a change to the closed 116 fixtures.
No new quality, metric or statistical expectation is copied from production.

## R1 / m1: old parser routes and exact bytes

Three committed sources in CLI tests/support fix the old syntax. The first item
inside Day is the seven-byte identifier compare: old non-bracketed compare A B
must be U-SYN-0 / unsupported / exit4, message unsupported syntax: scenario item,
span [51,58). Model-level compare A B stays unsupported syntax: compare,
span [33,40), U-SYN-0 / unsupported / exit4. Binding compare = A.run() is legal:
one one-minute call, done at1, work1, satisfied / exit0. No new reserved word.
The only changed route allowed here is compare [A, B] in Day: U-SYN-0 /
unsupported / exit4, message unsupported syntax: compare, same seven-byte span.

evaluation_review_legacy.json holds full stdout/stderr SHA256 and exit captured
from the supplied PRE-B7 baseline executable, on exactly these three files.
Authoring binary SHA256:
4b598db7ece41fc12ee4b67e5f8711402b5c7ce81aa23a92dafd08ac72b320dd.
Those captures fix OLD bytes, not new semantics. Runtime reads committed hashes
only and does not need that authoring binary. Entire binding output (including
Agency, state/terminal data, notices, warnings, ordering and newline) is bound.
The pre-existing 4003-job manifest is not regenerated or weakened.

## R2 / m2: three independent Cut conditions

Start with eval-agency: Office capacity2, short3 and long5, Serial3+5=8,
Parallel=max(3,5)=5, work3+5=8 for both. clock10/live_tasks2, common done end.
Both nested reports remain satisfied. Introduce Cut D with exactly C's include
set and no other metadata, so only names differ. No metric or probability
threshold changes, no malformed substrate.

1. AsIs:C, ToBeP:D, declaration:C: root invalid / exit3. Diagnostics in traversal
   order: declaration E-CMP-0 then existing interplan E-CMP-0. One mismatch pair
   AsIs->ToBeP, left_cut C, right_cut D, terminals null. differences is empty:
   names are represented by left_cut/right_cut, not a fictitious name field row.
2. Remove only the declaration's cutref: same pair and interplan E-CMP-0, no
   declaration diagnostic; invalid / exit3.
3. Both plans C, declaration D: only declaration E-CMP-0, mismatch null,
   invalid / exit3. Do not manufacture a pair to repair case1.

All three stop without evaluation or tables.agency. CLI checks real exit codes.

## m3: private entry counters are independent, positively calibrated

Implementation must link the three support/evaluation_review_private_*.rs files
under cfg(test): spy under lib.rs, compare under compare.rs, mutation under
mutation.rs. This is the established private acceptance seam; acceptance does
not commit src. For RED characterization, link only these modules temporarily,
without implementing the missing hooks, then restore src completely.

Seven slots retain existing indices and append separate execution routes:
0 lower, 1 engine::explore, 2 reference::explore, 3 compare, 4 engine::simulate,
5 reference::simulate, 6 engine::trace::explore as used by explain. Each direct
entry's positive control demands exactly one in its own slot and zero in every
other slot. The compare argument-failure control deliberately avoids nested
analysis. Both simulate controls use met-sample (R4, task duration2 or4, clock10)
and must complete all four replications before asserting their own spy slot;
an early unsupported return is not counted as a successful control. explain must
actually reach its trace explorer, count trace exactly
once and no ordinary explore/simulate/compare. It may lower its model first.
The existing eval-mut7 substrate supplies four detected structural deletions;
all seven counters must remain zero for kind7, with null mutant reports and no
other_changes. No mutable global instrumentation in production builds.
Existing src/evaluation_mutation_tests.rs four-slot assertions must follow the
seven-slot contract when implementation expands the spy.

## m4: unsupported weighted syntax, not malformed separators

Four independent CLI inputs append weight 1 or score 1 to the existing legal
hard row or objective row of eval-hard. The otherwise legal row fixes parser
progress before the unsupported word. Each must be U-SYN-0 / unsupported /
exit4, no E-SYN-0 and no evaluation. Ordinary missing separators between two
legal rows remain E-SYN-0; no broad parser reclassification is authorized.

## m5: no checked claims for discarded tables

The private compare adapter uses eval-sim-ratio's valid R1 reports. The normal
control computes metric_tables, property_tables and terminal_tables. Change
only Bad's selected IR confidence from1/2 to3/4, leaving its report unchanged.
Projection can still construct tables, but evaluation must reject the two
different simulation clauses as U-SEM-0 / unsupported / exit4, no panic, no
evaluation, default empty tables. The exact diagnostic simulation clauses differ
is required to distinguish this from early lower/projection rejection.
All three table words must then be absent from checked, as must the evaluation
words. This targets the post-projection evaluate Err path, not early lower or
projection rejection. It does not invent a successful partial table output.

## nit: no dangling shared-observation statement

Use eval-shared-source.broken-one-objective: only minimize overall.work. Both
plans have point work8, no strict improvement, remain admissible and non-dominated,
root satisfied. A single objective has no pair of shared observations. Preserve
the existing sure-dominance/never-add/simulation-not-computed statements and end
compare.pareto.affects with exactly:

    Shared observations: none among the declared objectives.

This explicitly describes only the declared objective pairs; it does not assert
statistical independence or absence of shared observations anywhere in reality.
The shared-pair naming obligation remains covered by the original T08 tests.

## m6: deferred integration, not current executed coverage

D05 requires hand-fixed ordered checked/unchecked arrays
for BOTH waiting breakdown and compare declarations after A7/B7 intake, including
waiting_tables and the existing B7 table/evaluation vocabulary, with a byte-bound
no-waiting control. This branch does not have A7; no integration GREEN is claimed.
