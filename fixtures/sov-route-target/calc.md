# sov-route-target

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P21: Target process one site still cannot also be a route: second occurrence could recur. Fail closed before exploration.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
