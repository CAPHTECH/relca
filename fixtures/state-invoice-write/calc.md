# F3: result and constant writes before invariant

One initial tuple: (approval=none,registered=false). Receipt0 and Approver.start0
do not change it. At finish(pass)1, checked=pass and approval=pass are published
in one bundle, before C1 reads approval: C1=true. Ledger.start1 still sees false.
At Ledger.finish(committed)+commit2, posted=committed and registered=true become
visible before C2: pass==pass and true==true, so C2=true. approval remains pass.
No process guard suppresses posting. done2, elapsed=1+1=2, work=1+1=2, result_at=null.
Safety is satisfied (one task at a time); root satisfied/exit0.
The Ack result is not a bool; a finish constant need not have the result type.
Approver writes without commits. No nondet latent assumptions are needed.
The pair changes only Approver result pass to fail, leaving both clauses intact.
