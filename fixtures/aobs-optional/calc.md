# F02 aobs-optional — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Optional answer starts none, flag starts false. Result ok writes answer and constant true writes flag together at t1. Removing only nonoptional default has no legal initial value.

Single input transformation: 'flag: bool = false' → 'flag: bool'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
