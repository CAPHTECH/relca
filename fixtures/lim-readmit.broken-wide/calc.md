# lim-readmit.broken-wide

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Task A.run holds one slot on [0,2), [2,4), [4,6). C=(2,4,6), work=(2,2,2). At b, capacity2 admits IDs1,2 and holds3; f starts1, then g admits3 at0. Capacity1 initially admits1, g admits2 at0, then after start2 at2 admits3 at2. Peak Held is respectively1 and2. First_start_wait is arrival-based (0,2,4), not admission-based (0,2,2). K01 is an implementation mutation, not a duplicate fixture.
Single transformation: 'queue capacity 1 on_overflow wait_external fifo' -> 'queue capacity 2 on_overflow wait_external fifo'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
