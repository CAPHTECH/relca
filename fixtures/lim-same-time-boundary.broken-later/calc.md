# lim-same-time-boundary.broken-later

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At t2 b runs before case1 end: both new cases rejected. At3 case1 already ended; case2 admitted and ends5, case3 rejected.
Single transformation: '[0min, 2min, 2min]' -> '[0min, 3min, 3min]'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
