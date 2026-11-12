# sov-clock

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P28: Route3..6; deadline5. clock4 has not reached deadline: inconclusive/unresolved reason null. Inclusive clock5 reaches deadline with no response: violated/unresolved deadline_passed_unresolved, not late_response. Response/closure/work/wait null; cost truncated; declined never_created.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
