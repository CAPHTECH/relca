# pol-label-env.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P09: A[0,2),B[0,1),work3. Environment can always select ok (prob0) or late (prob1/10), independent of bad draw. Positive fixed-policy. Removing call order leaves a/b starts as policy alternatives: full-information, same probability/time.

The pair changes only ' then call_order' to ''.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
