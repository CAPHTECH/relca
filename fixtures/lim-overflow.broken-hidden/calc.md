# lim-overflow.broken-hidden

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). Removing the explicit target violates both receipt obligations. Overflow end is excluded from F.
Single transformation: 'end.done, end.overflowed' -> 'end.done'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
