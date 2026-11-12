# sov-end-open

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P20: Route3..4 closes before Tail3..13 → End13/work13, summary0. Zero tail End3/work3 sees route open before dispatch; later response4 must not clear saved open flag:0..1.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
