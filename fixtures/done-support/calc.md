# done-support — hand oracle (ADR 0025)

Single change: none.

A zero kernel branch is absent from the support. Base reaches only good/end done: R/A/P=1, and else/Blocked/end bad are unreachable. Changing the distribution to 9/10 and 1/10 reaches else but cannot start Blocked. Therefore else warning vanishes, cap and bad-end warnings remain. End mass is 9/10, not 1.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
