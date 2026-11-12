# mut2-summary-atomic.broken-atomic — ADR 0046

T13c: case flag is not read by a process guard, ordinary writes may duplicate (done4→5). Atomic alone excludes producer. No commits or Actor state used.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
