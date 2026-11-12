# F1: serial and parallel, not a ranking

ADR 0017 and ADR 0008. Times in minutes. One case arrives at zero on the old route.
The only result is pass (nonprobabilistic singleton, not an invented uniform distribution).
Serial starts a at 0, completes at 3, starts b at 3, completes at 8.
Parallel starts both at 0, completes at max(3,5)=5. Occupied work is 3+5=8 in either plan.
All permitted start/finish interleavings have these same minima and maxima.
result_at is null because no any adoption occurs. done is reachable; capacity 2 is never exceeded.
Both properties and both plans are satisfied, root satisfied/exit 0. Requested order is ToBeP,AsIs.
The table normalizes the old report to case 1/arrival 0, with global safety case_id null.
The raw old report must not acquire case_outcomes, admission, or comparison assumptions.
No probability keys, differences, winner, score, or preferred baseline are inferred.
