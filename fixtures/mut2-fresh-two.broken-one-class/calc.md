# mut2-fresh-two.broken-one-class — ADR 0046

T21: case e_dup and type e_dup2 reserve two names, so fresh e_dup3. Removing only the type leaves fresh e_dup2. Task count and time unchanged.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
