# lim-zero-limit.broken-end

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Queue0 reject is legal and rejects3 with no receipt. End spill is also legal: received3/admitted0/overflow3 at0, work0. Both leave run and done unreachable.
Single transformation: 'on_overflow reject' -> 'on_overflow end spill'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
