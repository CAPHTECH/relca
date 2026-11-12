# lim-counts.broken-reject

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). Removing the overflow declaration leaves an undeclared response target: E-NAME-0 before metrics.
Single transformation: 'on_overflow end overflowed' -> 'on_overflow reject'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
