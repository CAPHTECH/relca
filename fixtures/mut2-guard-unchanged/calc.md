# mut2-guard-unchanged — ADR 0046

T17: duplicate only changes consumer args. Guard reading e is never rewritten. Aggregate reports alone cannot witness this; private AST obligation is explicit.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
