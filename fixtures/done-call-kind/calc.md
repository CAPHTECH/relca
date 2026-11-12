# done-call-kind — hand oracle (ADR 0025)

Single change: none.

Both declared capabilities have duration 1 and one result. Calling run leaves unused not_called. Changing just the call to unused leaves run not_called. There is exactly one cap warning in each, but at different declaration spans. No duplicate warning per call. All completion properties, Q, elapsed/work 1 and exit 0 are unchanged.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
