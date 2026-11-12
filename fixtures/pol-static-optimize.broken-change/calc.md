# pol-static-optimize.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P13 optimize: positive declaration fixes starts0,1, finishes3,4, work3 each. The single syntactic edit must fail closed with U-SYN-0, before exploration. No probability or timing is inferred for the static negative.

The pair changes only 'policy fifo tie_by case_id then call_order assign first_idle' to 'policy fifo tie_by case_id then call_order assign first_idle\n  optimize'.
This is the single edit side. Expected verdict: unsupported; exit 4.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
Observed once with the earlier binary: a parse failure ends report::check_with before the Cut is projected, and report::finish adds W-CUT-0 (span null) after the syntax error. This is existing behavior.
