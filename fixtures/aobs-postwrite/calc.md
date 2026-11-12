# F18 aobs-postwrite — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Own Desk invariant sees postwrite true. Good Probe finishes2 after Desk1. Broken both finish1; Probe-before-Desk reads false (violates), Desk-before-Probe true. Final true alone loses violation.

Single input transformation: 'capability ping() -> R { duration fixed 2min' → 'capability ping() -> R { duration fixed 1min'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
