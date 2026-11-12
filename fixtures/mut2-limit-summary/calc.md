# mut2-limit-summary — ADR 0046

T10 extension: 60 unused reads×2 + S old6 =126 old trials. Authority runs127. Summary e→a runs128, e→n not_run129. Authority expiry is checked despite whole-run generation/comparison remaining unchecked.

Expected values were derived before running the implementation. Time/work unit: minute.
Mutation is not a coverage or real-world quality score. Four unavailable kinds remain unsupported.
The exact new-row oracle is mutation_default_oracle.json in core tests/support.
Unimplemented mutation acceptance stops RED; this is not semantic verification.
