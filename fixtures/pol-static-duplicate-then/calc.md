# pol-static-duplicate-then

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P13 duplicate-then: positive declaration fixes starts0,1, finishes3,4, work3 each. The single syntactic edit must fail closed with U-SEM-0, before exploration. No probability or timing is inferred for the static negative.

The pair changes only 'then call_order' to 'then call_order then call_order'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
