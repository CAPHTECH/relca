# mut2-expiry.broken-zero-duration — ADR 0046

T01: finish1 > expiry0 gives satisfied→violated. Control finish0 ≤ expiry0 stays satisfied.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
