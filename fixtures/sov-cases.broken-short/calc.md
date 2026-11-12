# sov-cases.broken-short

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P09: Single edit 'respond_by 5min' → 'respond_by 1min'. a1 approve(case1)0..2 then approve(case2)2..4. First route queued3 waits until4, responds5; second target5 responds6. End case1=2/3, case2=4/5. Short deadlines4/6: only first late, second exactly on time. Summary counts 0..2 distinct cases.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
