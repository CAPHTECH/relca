# sov-equality.broken-late

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P16: Single edit 'duration fixed 1min; result nondet { pass, fail }' → 'duration fixed 2min; result nondet { pass, fail }'. Target3+1=deadline4=response4 succeeds. Duration2 responds5>4 fails; End3/work3 unchanged.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
