# ADR 0043 hand derivation

T01/T18. R capacity 0 < demand 1: b remains queued, starts=0. a runs [0,2], work=2. cancel_then_drain cancels queued b iff a=pass; done/result_at=2. a=fail cannot take if_none because b never finishes. drain never completes even when a matches. Blocked sinks count a=1,b=0; only satisfiable cancellation paths set a flag (not actual cancellations).
Single edit: 'remaining cancel_then_drain' -> 'remaining drain'.


The expected values above are algebraic/static hand oracles, not captured output. Stage C RED is not implemented semantics, calibrated mutation detection, real-world legitimacy, skill retention, or all-schedule safety.
