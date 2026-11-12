# pol-cross-class-person.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P18: both A-first(result3,complete3..4) and environment B-first(result5,complete5..6) remain. Deadline4 violated. Changing B control to policy fixes A-first only. Neither class may be globally prioritized.

The pair changes only 'control environment' to 'control policy'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
