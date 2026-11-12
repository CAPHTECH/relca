# sov-effect-and-end

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P31: Effect and End both at3. Four independent filing choices; both filed responses4/5 <= deadline5, End3/work3, summary0..1. Broken duration2: responses5/7, each row has late alternative7. One-filed response5 remains on time.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
