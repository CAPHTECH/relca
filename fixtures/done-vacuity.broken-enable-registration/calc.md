# done-vacuity.broken-enable-registration — hand oracle (ADR 0025)

Single change: Judge only result bad -> good.

Base Judge can only return bad at 1. Registration is never started, its commit antecedent never occurs: invariant vacuous, not effective registered safety. Then and registered are unreachable; returned probability 1. Registered reachable and Q are violated. Change result to good: post starts once and commits at 2, invariant satisfied, registered Q=1. Else/returned now unreachable. R/A/P stay satisfied in both. A separate contract-only test requires vacuous exit 5 despite warnings.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
