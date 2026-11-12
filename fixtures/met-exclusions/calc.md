# M-exclusions / M-prefix-cost

R=4, seed0; fixed times and one result cause no draws. W10, clock3.
Each replication: at0 admit case1, reject case2 before starts; case1 runs[0,3).
At1 admit case3, at2 reject case4 because case3 waits. At3 finish case1,
start case3; its finish6 exceeds clock. Observe until3, no extra drain.
Case1 lead3/wait0/cost3*2+5=11; case3 wait3-1=2, lead excluded,
work0 but start1 costs5. Rejected cases have lead null/rejected and cost0.
Total work3, peak1, utilization3/10, starts2, total3*2+2*5=16,
per-arrival16/4=4; end.done per-case11 (one completed case per rep).
Each rep counts received2/completed1/rejected2/unfinished1/deadlocked0.
Totals over4 reps:8/4/8/4/0. Every prefix scalar has samples4 and mean=min=max.
Lead case1 samples4; case3 samples0/no_samples; case2/4 samples0/rejected.
Safety cannot be proved by simulation: root inconclusive/exit2.
