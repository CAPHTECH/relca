# F8: observed writes, not latent values, widen time

Initial route=none, receipt0. Picker starts0 and finishes1 with routine or
difficult, writing both choice and route. routine: route==difficult false,
Fast starts1/finishes2, done2, elapsed2/work1+1=2. difficult: true, Slow starts1/
finishes5, done5, elapsed5/work1+4=5. No later state writes; result_at=null.
done range elapsed[2,5], work[2,5]. Safety/root satisfied/exit0. No contracts.
There are no latent values or distributions. Agency uses state_guards/new rules.
The pair adds finish{route=routine} to replace only Result with a Constant
source; the two Picker results must still exist but no longer choose Slow.
