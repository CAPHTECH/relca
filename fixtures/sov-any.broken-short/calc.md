# sov-any.broken-short

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P18: Single edit 'respond_by 5min' → 'respond_by 4min'. Fast3..5 adopted5; Slow runs3..6 then ack6, End6. Ordinary work2+1+2+3=8, result_at5. Route3..8 is not canceled/drained by normal Any. Deadline8/7. Fail approval path End2/result_at null.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
