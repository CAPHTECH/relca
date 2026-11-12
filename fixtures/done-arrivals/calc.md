# done-arrivals — hand oracle (ADR 0025)

Single change: none.

Capacity 1 and FIFO: arrivals 0 and 3 do not overlap; ends at absolute 1 and 4, each elapsed/work 1. Deadline is relative to arrival, so Q=1 in both cases. Moving second arrival to 6 beyond clock 5 keeps case1 satisfied, case2 Future/unknown, not rejected. Each new completion property folds satisfied+inconclusive to inconclusive; case2 P/Q [0,1]. Global cut suppresses warnings.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
