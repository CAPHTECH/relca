# done-arrivals.broken-arrival-after-clock — hand oracle (ADR 0025)

Single change: second arrival 3 -> 6.

Capacity 1 and FIFO: arrivals 0 and 3 do not overlap; ends at absolute 1 and 4, each elapsed/work 1. Deadline is relative to arrival, so Q=1 in both cases. Moving second arrival to 6 beyond clock 5 keeps case1 satisfied, case2 Future/unknown, not rejected. Each new completion property folds satisfied+inconclusive to inconclusive; case2 P/Q [0,1]. Global cut suppresses warnings.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.


Admission bound: admission.rejected_count is {min:0,max:1}, not max 0. Under a cut, rejected_count uses a conservative upper bound: case2 arrives at 6min beyond clock 5, so the cut leaves its rejection unknown and the upper bound counts it. Case2 is still not reported as rejected in case_outcomes; only the bound changes. The completion properties are unaffected.
