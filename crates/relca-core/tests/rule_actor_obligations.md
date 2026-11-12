# Rule internal-state and IR checks

Stage C acceptance is intentionally RED. Public Rule integration tests do not
prove internal scheduling-key isolation or validate fields that do not exist
before Rule implementation. These are finite internal test obligations;
none is represented as a silently ignored passing test.

| ID | seam / setup | independent expected result / fault |
|---|---|---|
| IR1 | R0 lowered Rule value, replace Fixed by None/Nondet/Distribution/type-out-of-range | engine U-SEM-0 and no numeric terminal; reference SemanticAbort separately (public tests supplied) |
| IR2 | owner or StateVarId/TypeId out of range, metadata.rule None while StateKind Rule, metadata for non-Rule, duplicate rule value | fail closed independently, never panic/fallback |
| IR3 | rule_grants target non-Rule or undeclared, person out of range; under target non-Rule; under plus any Capability grant | engine unsupported / reference abort even if grant unused |
| IR4 | RuleInfo declared/effective/default/amendment versions or types disagree; optional=true; integer/set metadata | independent rejection; do not repair declared metadata by overwriting it |
| IR5 | start_writes/writes/atomic.writes/kernel_writes/row.branch.overrides Rule target | both entry validators reject unused capabilities too; public mutation matrix supplied |
| IR6 | kernel given/kernel_reads/version_check.variables Rule reference | independent unsupported/abort; rows after the first must also be scanned |
| IR7 | Rule last_write_wins or concurrency_resolutions | reject instead of treating fixed Rule as mutable ActorObserved |
| P1 / F07e | engine child test: Rule is_shared_state, is_shared_observed, reads_shared_observed false; process Branch guarded by Rule in 2 simultaneous arrivals | direct predicates false and count of candidates added solely for shared Rule guard =0; inject only engine fault, then restore |
| P2 / F07r | reference child test: same independent shared_state/shared_guard | false, added candidate0; inject reference fault separately; public time/Outcome equality is not evidence of this kill |
| P3 | existing budget/census for new payload | no Rule seed/random/version runtime state; state count measured from actual private census, never hand-invented |
| X1 | explain Rule state snapshot, replay/capture k and k-1 | verified full path under adequate bound, explicitly unsupported under insufficient bound; no configuration selector |
| X2 | corrupt replay state Rule value/owner/version label or projection | fail replay/unsupported, not verified because both engine and replay used the same corrupt value |
| C1 | src syntax/lower/engine/simulation historical populations | exclude exactly support/rule_names.json, keep old counts; unknown rule name fails CLI classification |

The initial public IR test intentionally uses existing public fields only; IR2,
IR3, IR4, IR6's version_check and IR7 must be linked after new fields and a5 are
available. The public comparisons strip lower diagnostics only from reference,
not engine, and compare complete Outcomes after individual hand arithmetic.
F07 requires real private observation in both implementations, not a guessed
state-count constant or the same helper used by production scheduling.
