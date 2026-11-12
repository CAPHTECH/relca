# lim-clock.broken-short-clock

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Task A.run holds one slot on [0,2), [2,4), [4,6). C=(2,4,6), work=(2,2,2). Concurrent1 includes running cases until their business end: A=(0,2,4), C=(2,4,6), external_wait=(0,2,4). At clock3 case1 complete, case2 running, case3 Held: received3/admitted2/completed1/unfinished2/rejected0. Exhaustive metrics withdraw EVERY Scalar to null/truncated; admitted_at=(0,2,null).
Single transformation: 'clock 20min' -> 'clock 3min'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
