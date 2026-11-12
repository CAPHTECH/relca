# lim-overflow-admitted.broken-receipt

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). Admitted obligation exists only for case1; overflow cases vacuous. Received origin obliges all3 and rejects the missing spill target.
Single transformation: 'case.admitted' -> 'case.received'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
