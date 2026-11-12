# lim-both-end-id.broken-other-end

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). Both limits share ONE EndId spill. Different end IDs are mismatched actions even though both variants are End.
Single transformation: 'concurrent cases 2 on_overflow end spill' -> 'concurrent cases 2 on_overflow end other'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
