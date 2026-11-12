# sov-label.broken-other-grain

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P11: Single edit 'grain ledger;' → 'grain document;'. Label/metamorphic control, not a business violation: renamed Actors and ledger/document grain cannot change commit3/response4/End3/work3. Source byte spans are normalized using independently paired tokens, not ignored globally.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
