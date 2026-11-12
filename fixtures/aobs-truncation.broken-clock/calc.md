# F19 aobs-truncation.broken-clock — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Fixture changes only clock to0, no completed end and no range witness. Dedicated core execution-condition counterpart uses state budget0 on unchanged good input: held root, truncated true, state_in_range inconclusive. Not a range stop.

Single input transformation: 'clock 10min' → 'clock 0min'.

Expected verdict: inconclusive; command: check.
RED acceptance is not implementation verification.
