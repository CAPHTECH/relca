# done-within — hand oracle (ADR 0025)

Single change: none.

duration 2 has mass 3/4; duration 5 has mass 1/4. End occurs immediately after finish. At deadline 5 both are included: 3/4+1/4=1. At deadline 2 equality includes the first mass only: Q=3/4. Completion R/A/P stays satisfied; elapsed/work [2,5] is unchanged.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
