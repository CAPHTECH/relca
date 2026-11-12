# done-probability — hand oracle (ADR 0025)

Single change: none.

The single kernel has masses 9/10 good and 1/10 bad. Both end when Blocked has capacity 1. At capacity 0 only good ends, mass 9/10. The remaining 1/10 is known deadlock, not an additional terminal. Both probability endpoints are 9/10; R/A/P violated.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
