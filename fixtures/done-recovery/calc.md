# done-recovery — hand oracle (ADR 0025)

Single change: none.

Judge finishes at 1. good ends at 1; bad needs one further minute. With capacity 0 the bad state is a closed deadlock, but good remains reachable. The nondeterministic scheduler can choose either: min 0, max 1, not 1/2. With capacity 1 both end: R=A=P=1.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
