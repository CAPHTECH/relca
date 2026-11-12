# lim-legacy-route.broken-no-limit

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

No arrivals, policy or queue. Concurrent alone forces multi-case JSON even for cases1. Removing concurrent restores legacy terminal done elapsed/work2.
Single transformation: '  concurrent cases 1 on_overflow reject\n' -> ''.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
