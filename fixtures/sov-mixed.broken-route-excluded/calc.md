# sov-mixed.broken-route-excluded

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P07: Single edit 'include [Approver, Ledger, Clerk, Vendor]' → 'include [Approver, Ledger, Vendor]; exclude Clerk reason "r" loses "l"'. Fail End2 triggers terminal route2..3. Pass commit3 triggers effect route3..4. The two rows are mutually exclusive per path, not jointly active. Excluding Clerk removes both routes; filed rows have missing_route and null response/closure.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
