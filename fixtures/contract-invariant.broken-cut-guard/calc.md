# F1 single fault: remove rejection

Only change from contract-invariant: else no longer terminates.
received0 -> Approver.start0 -> finish(pass or fail)1 -> Ledger.start1 ->
finish(ok)+commit2 -> end.done2. Each path has elapsed=work=2, result_at null.
Clause #1 is true on pass and false on fail at commit2; one violation suffices.
#1/root violated, exit1; safety satisfied, no cut. W-CUT-0 only required warning.
Expected values derive from ADR 0010, not an implementation run.
