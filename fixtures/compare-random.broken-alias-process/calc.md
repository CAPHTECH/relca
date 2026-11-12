# F3 single fault: same process alias

Exactly one replacement: Bplan process Q→P. All standalone check/simulate and exhaustive compare
still accept aliases (ADR 0017 U1). In each standalone simulation, z hits4, A.z hits2, y hits2,
P yes2/no2, all trials4/unknown0. See the correct pair for all 24 SHA observations and exact CP tails.
The work keys kernel.P.a and duration.P.a now coincide across plans, violating initial-only coupling.
compare simulation therefore emits U-SEM-0 naming Aplan,Bplan,P, root unsupported/exit4,
both tables empty, mismatch null, common_random null. This is a comparison gate, not static rejection
of either standalone plan. No seed or key alteration is permitted to manufacture separation.
