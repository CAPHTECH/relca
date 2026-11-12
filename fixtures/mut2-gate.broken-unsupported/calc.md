# mut2-gate.broken-unsupported — ADR 0046

T09: retry semantics absent, U-SYN-0 blocks six kinds. Parsing fails before an AST is available, so report::check_with retains empty_report Cut=undeclared and finish adds W-CUT-0 (report.rs:1178–1185,1511–1526). The warning is not mutation detection.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
