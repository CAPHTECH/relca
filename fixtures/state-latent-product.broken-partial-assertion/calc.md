# F2 fault: partial assertion

Single edit: replace the difficulty tautology with Invoice.difficulty == routine.
Initial tuples remain (true,routine),(true,difficult),(false,routine),(false,difficult).
Each has receipt0/start0/finish(ok)1/done1; both latent values stay constant.
C1 at finish1 is respectively true,false,true,false. Thus its universal verdict
is violated, safety satisfied, root violated/exit1. All tuples still terminate
at elapsed1/work1/result_at=null; a failed assertion does not prune execution.
Both nondet assumption rows are unchanged. No uniform probability is assumed.
