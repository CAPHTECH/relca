# lim-concurrent-wait.broken-short

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Task A.run holds one slot on [0,2), [2,4), [4,6). C=(2,4,6), work=(2,2,2). Concurrent1 includes running cases until their business end: A=(0,2,4), C=(2,4,6), external_wait=(0,2,4). 6<=6; 6>5.
Single transformation: 'by 6min' -> 'by 5min'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
