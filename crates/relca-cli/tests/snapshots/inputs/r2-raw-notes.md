# Recovery raw regression inputs

These are current-output regressions, not new semantic oracles. ADR 0065 and
receipt_recovery_calculations.md supply the independent acceptance values.
All durations, results, arrivals and keys here are explicit model assumptions.

Flow has charge [0,1), query [1,3), decision [3,6), resolve at6, shipping [6,7).
Thus Full completes at7 with work7 and one original commit. The unknown receipt
at1 remains unknown; the query evidence at3 is not a received original Ack.
The arrival-based deadline is6, so completion at7 violates it; resolve at6 does
not repair the deadline. Full explain must replay that late business End.
All actors are anonymous, and prices are unconfigured, not zero.

Clock stops at2 computationally, before query evidence; Horizon stops at2 for
the business scenario. Both have observed work2 and one charge, no business End.
They must retain different stop reasons and must not turn their pending query
into successful resolution. Other stops at3 after the query without resolve:
work3, one charge, no completed End. This is inconclusive, not a known failure.

Simulation uses four samples and never claims exhaustive coverage. The two
sensitivity points are deliberately identical controls: both exact probability
queries are one, and their difference is zero. Duration perturbation and receipt
probability hand values remain covered by the fixed independent R2 acceptance.
Comparison and diff retain both child reports and the unresolved B side.
Mutate, conform and verify remain unsupported. The conform job has a regular
header-only log mapped to these actors to reach the model-feature refusal;
it does not claim log conformance.

New jobs are explicitly listed in scripts/snapshot-extra-jobs.json. Existing
jobs and their saved bytes are not updated to match this feature.
