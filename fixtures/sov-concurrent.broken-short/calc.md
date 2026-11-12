# sov-concurrent.broken-short

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P02: Single edit 'respond_by 5min' → 'respond_by 2min'. At3 notify and recheck compete for c1. Recheck3..4 then notify4..6 gives End6; notify3..5 then recheck5..6 gives End5. Ordinary work2+1+2=5 (not6). Deadlines8/5; late response6 exists only in short pair.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
