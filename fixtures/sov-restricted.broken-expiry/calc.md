# sov-restricted.broken-expiry

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P22: Single edit 'basis "posting rule";' → 'basis "posting rule"; expires 100min;'. Revocation record alone is not a restriction. Expiry100 is restricted even though target3<100, so reject; no silent eligibility filtering.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
