# sov-policy

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P29: Policy normal call_order notify precedes synthetic contest route: notify3..5, End5, recheck5..6 only. Environment choice restores both schedules End5..6/response4 or6. FIFO without call_order does not settle same case/tick.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
