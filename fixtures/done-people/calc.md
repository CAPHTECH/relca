# done-people — hand oracle (ADR 0025)

Single change: none.

Included pool union is {p,q}; declaration-only r is outside it. Base A can start with p or q, hence union of actual assignments {p,q}. Removing q only from A leaves q in B's pool and in the population, but B has no calls. Actual starts now use only p: exactly one W-UNREACH-3 at q's identifier, none at r. End/work/time remain 1.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
