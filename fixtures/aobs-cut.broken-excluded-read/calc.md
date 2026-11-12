# F08 aobs-cut.broken-excluded-read — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Probe stays included. Only the read owner D leaves Cut, so this must fail for the condition, not for a Capability call.

Single input transformation: 'include [D,Probe]' → 'include [Probe]; exclude D reason "out" loses "state"'.

Expected verdict: invalid; command: check.
RED acceptance is not implementation verification.
