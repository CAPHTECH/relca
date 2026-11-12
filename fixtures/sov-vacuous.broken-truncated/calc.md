# sov-vacuous.broken-truncated

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P03: Single edit 'clock 20min' → 'clock 1min'. Only fail is effective, so post is never called: static guard still exists, response vacuous, End2/work2. clock1 stops before approve finishes: inconclusive, not vacuous. Complete warnings: post/recheck not_called, else arm, posted end, c1 unassigned; truncation must not assert unreachable.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
