# F07 aobs-policy — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Observed fixed fast chooses wait1 (work0). Else is unreachable, W-UNREACH-1. Replacing state by latent makes the same policy read forbidden, regardless of initial value.

Single input transformation: 'state mode' → 'latent mode'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
