# sov-deadline.broken-zero

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P05: Single edit 'respond_by 5min' → 'respond_by 0min'. Deadline must be positive structurally; zero is a violated property, not invalid. Target3 +0=3 < response4. Actual response and closure remain4.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
