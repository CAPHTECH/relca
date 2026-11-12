# ADR 0043 implementation linkage and finite obligations

The public hand oracle is `agency_second_half.rs`, with 82 inputs/41 control pairs
in `support/agency_second_half_names.json`. Stage C deliberately stops RED.
No measured budget k, production mutation calibration, or private-IR result is claimed.

## Private budget adapter (written, not yet linked)

Include `tests/support/agency_second_half_budget.rs` from an implementation
`cfg(test)` module, following the existing ADR 0034 budget adapter. Supply only
the injection closures; do not change expected values to observed output.

- `effects`: run `agy2-concentration` with exactly one effects limit changed.
  Hand E/J/U/roots and one W-AGY-2 must match first. Binary-search accounting k,
  record k, require k GREEN / k-1 whole-block not_derived, no rows or partial
  W-AGY-2, executors retained, legacy/F/L/diagnostics otherwise unchanged.
- `differences`: select From/To in `agy2-diff-items`, inject one differences limit.
  First require the ten hand rows (item-major ordering); k-1 discards all rows,
  retains side reports and verdict, and does not consume the effects/legacy budget.
- `fallback_retention_failure`: select To of `agy2-keep-fallback`; zero retention
  work produces not_derived, never W-AGY-3. Derived zero exercise is a different state.
- Do not invent charges for an unused dimension; record that inspection separately.
  The default maxima are depth128/paths4096/contexts65536/cells1000000/work1000000.

## Public IR and unavailable Agency seams

After the new IR exists, add explicit unit tests at the private derive entry:

1. `read_variables` out of bounds, latent StateVarId, self/cyclic edge, and same-fork
   read/write in both lexical orders must refuse U-SEM-0, not silently drop an edge.
2. A mechanism-form atomic with an inserting fork sibling is rejected by the
   public IR validator or has unknown normal-path refusal; never false. Legal DSL
   is already rejected by the old write/write guard, so do not relax that guard.
3. effects block absent/not_derived retains executors; row-level unknown preserves E
   and state_refusers, omits j/u, retains unknown generation_roots. Known two people
   can establish false before anonymous/unknown, but never a fabricated singleton.
4. comparison side agency=null (either or both): differences not_derived with
   agency_unavailable and no rows, root unchecked agency_differences exactly once.
   Comparison mismatch instead uses comparison_mismatch, not a successful empty diff.
5. Both sides F state_guards with equal reasons remain not_comparable; a one-sided
   effect call is a null-valued addition/removal only when blocks are known.
6. Caller-provided expected actor/call keys, duplicate meaning rows, unknown item,
   wrong normal-side reason, and score/count/rank fields must fail the closed wire.

## Existing cfg(test) population census (numeric oracles remain fixed)

These generic
walkers encounter agy2-* and must exclude **exact names** from the new names table
before applying their historical population/budget oracle:

- `src/syntax.rs`, `tests::every_fixture_obeys_its_expected_syntax_diagnostics` (the read_dir loop
  around former line3426): new grammar/static faults must not enter its frozen count.
- `src/engine.rs`, `legacy_census` generic fixture loops (around former line1798):
  preserve old state/successor/budget counts, do not add agy2 models to the census.
- `src/engine/simulation.rs`, historical simulation migration census (read_dir
  around former line1057): preserve its old148 population and classification counts.

The final workspace log identifies the exact test names that actually fail. The
prefix-only met/resource-specific loops were inspected as distinct; they do not
need a broad agy2 wildcard exemption. Integration-level `tests/differential.rs` and
other generic test walkers are handled separately in the acceptance patch.

## Predecessor seams

T17 currently uses the two non-scenario call_order substrates, as explicitly
accepted in the design decision. The actual B5 scenario crossing, A4 record reads, B4
overflow_end exclusion and C4 contest-only effect exclusion have exact deferred
separate integration inputs, expectations and targets. T20 version_check
fixtures exist now but also require A5 acceptance of concurrent commutative writes.
Do not attribute those missing predecessor semantics to a C5 implementation defect.
