# sov-live-tasks

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P15: At3 filed route and notify are two queued live Tasks before dispatch. Bound1 cuts that branch; declined branch still reaches End5. A surviving complete branch cannot turn global cut into satisfied.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
