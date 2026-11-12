# mut2-guarded-write — ADR 0046

T18: case write without later flag guard may duplicate; with later observed flag guard exclude, even if branches empty. Time4.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
