# ADR 0048 private seams and integration obligations

The public `waits` targets now exercise the implemented opt-in syntax.
`support/waits_private_oracles.rs::run` is connected separately by
`engine_waits_private_p01_p08` and `reference_waits_private_p01_p08` in the two
collectors' unit-test modules. Review additions P09–P12 use `run_review`, called
by `engine_waits_review_p09_p12` and `reference_waits_review_p09_p12` at 4cec795.
The survivor addition P13 uses a NEW `run_mutation` callback and is **unlinked**;
see `support/waits_mutation_calc.md`. Existing adapters do not pass waits.

## Required independent linkage

Link `run` from engine metrics and reference metrics unit tests **separately**.
The callback builds the plain input histories/leaf records using that
implementation's private types, calls the actual collector/reducer, and selects
the normalized quantities in `Probe.expected`. Do not implement another
calculator in the callback and do not return the oracle as the observation.
Both linked tests need execution evidence.

| Probe | Hand calculation and control | Mutation seam |
|---|---|---|
| P01 | ready/start2, stop3, End3: unknown2 + service1; ready/start1 gives unknown1 + service2 | F30 |
| P02 | queued ready0, cancelled-before-start stop2, T5: total2/max1/avg2/5; stop3 gives3/5; causes null/cancelled_before_start | F23,F38 |
| P03 | blocker [2,2), target queued[1,3): actor0, unknown2; blocker[1,2) gives actor1, unknown1 | F24 |
| P04 | leaves (A0,B3),(A5,B0): overall3..5, never0..8; reverse order | F19 |
| P05 | completion first then cutoff service3: dwell3..3 lower; reverse order; two completions means absent dwell | F27,F28 |
| P06 | horizon declaration in both records, only actual stop OR adds lower; avg1/2..1, total2..3, never bound on max | F25 |
| P07 | (total,T)=(2,4),(6,3): mean(1/2,2)=5/4, not8/7; second total3 =>3/4 | F29 |
| P08 | completion+cutoff then global computational cut: every new Scalar null/truncated; dwell and all bound flags absent | F14 |
| P13 | a[0,4), wait[3,5), gap[5,6), b[6,7): service5/intentional1/unknown1; wait[4,5) control same partition | F21 |

P13: independent callbacks must build both Tasks, preserve `cases[].waits`
as actual interval input, and (on reference) pass intentional_wait as the raw
clipped interval sum (2, control1), not the exclusive partition's intentional1.
Call the actual collector and select the complete partition fields only; do not
compute the partition in the adapter. Add separately named engine/reference
P13 unit tests calling `run_mutation`. An inventory test alone is not GREEN for
the collector, and no F21 RED is claimed before this source-owned linkage.

Also connect P05's late-presence records to the **sample** reducer: only the
cutoff replication contributes to dwell, samples1, min=max3, never zero-filled.
For a truncated simulation prefix, keep observed predecessor/started-task
causes sample values under the existing ADR 0019 rule; an unstarted Task's
causes remain null/unfinished. Clock-only does not create dwell. An aborted
simulation invalidates all new samples and removes dwell/bound.

## State and adapter boundaries

No ready/start/stop/executor/wait trace, Room, hash/equality, schedule, draw key,
or transition is added or changed. The collector must see **all** admitted cases,
including completed blockers and synthetic contest routes. Effective admission
falls back to receipt only for ordinary already-admitted legacy slots, not Held,
Future, rejected or overflow slots. Reference trace Wait intervals, not just
their sum, must reach the independent reference collector (T09).

Explicit forged-IR public tests already cover required Actor/Capability indices.
Add resource-uses, executor and branch binding malformed inventories at the
private validation entrances, preserving engine U-SEM-0, reference SemanticAbort,
and no partially published metrics. Correct manually constructed opt-in IR must
not require a lower-produced assumption row merely to execute.

Private zero-minute and cancelled-before-start histories are artificial probes,
not claims that a public model creates an unexplained gap. Tick arithmetic must
use interval boundaries and exact rationals, not loops proportional to clock.

## Existing source-owned censuses

Use the **closed** wait
name exclusion to historical, fixed-denominator censuses inside cfg(test), not
an open wait-prefix filter. No old numeric expectation changes are permitted.
The historical observations below distinguish population failures. Waits
have their own independent exhaustive/reference/sample population in `waits.rs`.

The final pre-implementation workspace run observed exactly these three
source-owned follow-ups (all43 new directories entered the old censuses):

| Existing test | Observed failure | Required follow-up, no new oracle number |
|---|---|---|
| `engine::legacy_census::e_legacy_state_identity` | engine.rs:1900, dirs167 vs124 | Exclude the exact43 wait names and retain124 |
| `engine::simulation::simulation_tests::s_engine_migration_classification_preflight` | engine/simulation.rs:1234, (16,191) vs(16,148) | Same closed exclusion; retain16/148 and the old classification values |
| `syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics` | syntax.rs:4263, wait-person U-SYN-0 breakdown | Keep this fixed old population separate from the new waits syntax matrix |

These observations are population follow-ups, not permission to regenerate
old state hashes, classifications, numerical expectations or manifests.
