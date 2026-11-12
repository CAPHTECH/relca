# mut2-repeated-argument — ADR 0046

T22/F19: a call consuming e twice is one consumer. Both args must change to e_dup in one mutant. Time4→5, group2; the single-argument control has the same lineage.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
