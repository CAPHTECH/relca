# sov-two-contests.broken-longer

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P20b: Single edit 'duration fixed 1min; result nondet { pass, fail }' → 'duration fixed 2min; result nondet { pass, fail }'. Two independent filed bits give four choices. Both filed serialize c1, responses4/5 (broken5/7). Each marginal includes both positions. End3/work3; business_ended_open max1, not2.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
