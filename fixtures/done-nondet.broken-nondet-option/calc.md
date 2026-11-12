# done-nondet.broken-nondet-option — hand oracle (ADR 0025)

Single change: Judge result {good} -> {good,bad}.

Only good is possible in the base model: all-end 1. Adding bad to the nondeterministic result set enables a closed deadlock. The worst scheduler gives probability 0, best 1; min-upper=0<1 proves P violated despite max-upper=1. Blocked never starts in either model. Base also never takes else.

Time and work are integer minutes. R=AG EF end, A=AF end, P=all-end probability under the worst scheduler. A deadlock is not an end. Unknown cut mass has lower 0 and upper 1. No probability is invented for nondeterministic choices. All expected values above were authored before running the new implementation. Warning spans are the declaration/If/first End/people identifier spans, checked independently by the acceptance tests.
