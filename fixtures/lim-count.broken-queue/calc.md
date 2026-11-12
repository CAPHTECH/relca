# lim-count.broken-queue

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

At t2 b sees concurrent=2 before case1 finishes; queue sees only unstarted case2=1. Concurrent rejects3; queue admits3. C=(2,4) vs (2,4,6); elapsed=(2,3) vs (2,3,4).
Single transformation: 'concurrent cases 2' -> 'queue capacity 2'.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
