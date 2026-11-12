# lim-counts-obligations.broken-reject

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). Both targets only done: overflow cases violate, rejected cases are vacuous. Root violated vs vacuous; do not confuse with E-NAME-0 of P12.
Single transformation: 'on_overflow end overflowed' -> 'on_overflow reject'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
