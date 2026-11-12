# F03 aobs-owner — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

A.run owns A.x, not B.x even though both have type R. The logical target ownership is the only changed rule; both its mentions change.

Single input transformation: 'A.x' → 'B.x'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
