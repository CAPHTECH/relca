# pol-static-synthesize

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P13 synthesize: positive declaration fixes starts0,1, finishes3,4, work3 each. The single syntactic edit must fail closed with U-SYN-0, before exploration. No probability or timing is inferred for the static negative.

The pair changes only 'policy fifo tie_by case_id then call_order assign first_idle' to 'policy fifo tie_by case_id then call_order assign first_idle\n  synthesize'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
