# A20 fault — other branch cut, hand oracle
Single edit clock10->3 min. Fast violation at2 remains known. Slow finish6 is cut.
Authority fold stays violated/exit1, truncated=true. Only known terminal is done2/2.
A cut must not overwrite an already observed violation or complete the slow case.

- 安全性(no_resource_overcommit)は、切断のある探索では切断の外を見ていないので inconclusive(既存の規則。fixtures/done-arrivals.broken-arrival-after-clock と同じ)。
