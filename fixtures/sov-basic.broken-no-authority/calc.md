# sov-basic.broken-no-authority

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P01: Single edit '  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }\n' → ''. approve fail: End2/work2. pass: post2..3, End3/work3. Guard candidate Approver only; a1 has Permission and ApprovalGrant. Route3..4, deadline8. Removing grant leaves Permission あり、Authority 未接続.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
