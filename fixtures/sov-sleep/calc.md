# sov-sleep

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P17: Post commit3; wait wakes8, not a Task. Route runs3..4 during sleep, closes4, execution waits End8. Broken route3..9, End8 open, deadline8 <9. Work remains3.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
