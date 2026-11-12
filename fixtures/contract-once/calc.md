# F2: count Capability commits across callsites

Hand oracle (ADR 0010). received0 -> Ledger.start0 -> finish(ok)+commit1 -> end.done1.
again belongs to Ack but is not a possible result, so the second site is unreachable.
Clause #1 activates on the first commit, count=1: satisfied, not vacuous.
elapsed=work=1 minute, result_at null, safety/root satisfied, exit0, no truncation.
Pair adds only again to the nondeterministic result set; probabilities are unspecified.
