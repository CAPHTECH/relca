# F15 aobs-scope — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Authority scope true is observed and permits commit at1. Identical latent scope is forbidden. False observed scope is a runtime authority violation, not a type failure.

Single input transformation: 'state allow' → 'latent allow'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
