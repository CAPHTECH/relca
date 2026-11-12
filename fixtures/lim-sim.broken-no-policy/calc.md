# lim-sim.broken-no-policy

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Task A.run holds one slot on [0,2), [2,4), [4,6). C=(2,4,6), work=(2,2,2). At b, capacity2 admits IDs1,2 and holds3; f starts1, then g admits3 at0. Capacity1 initially admits1, g admits2 at0, then after start2 at2 admits3 at2. Peak Held is respectively1 and2. Ten deterministic replications: lead2,4,6; E0,0,0; first_start_wait0,2,4; samples10; received/admitted observed_total30. Safety remains inconclusive. Without FIFO first start at0 has two successors: unsupported, attempted1/completed0, no statistics.
Single transformation: '  policy fifo tie_by case_id\n' -> ''.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
