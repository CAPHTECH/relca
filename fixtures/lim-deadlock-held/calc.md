# lim-deadlock-held

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Concurrent1 includes running cases until their business end: A=(0,2,4), C=(2,4,6), external_wait=(0,2,4). Capacity0: case1 admitted but no start; IDs2,3 Held at0; no future event. Deadlock not clock cut, received3/admitted1/deadlocked3/unfinished0. Held external_wait is null/deadlock, not0.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.
