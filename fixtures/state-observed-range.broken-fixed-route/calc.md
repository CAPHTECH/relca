# F8 fault: constant route

Single edit: add finish{route=routine}. Initial route=none, receipt0/start0.
At Picker.finish1, choice is routine or difficult, but route is routine in both
paths because the explicit constant takes precedence over Result. The guard
is false in both, Fast runs1..2 and done2. Slow never starts. route stays routine.
Elapsed[2,2], work[2,2]=1+1, result_at=null; safety/root satisfied/exit0.
No clauses or probabilities. The fault is detected by loss of the [2,5] range,
not by a verdict change. Ignoring the constant or always copying Result fails.
