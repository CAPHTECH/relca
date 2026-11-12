# F17 aobs-read-write-order.broken-later-arrival — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Case1 Desk0..2. At2 Branch(case2) before Finish(case1) sees false, Desk2..4; reverse sees true, Alt2..3. Case2 completed_at3..4 elapsed1..2 work1..2, both meaningful schedules required. Broken arrival3 always sees true, Alt3..4, elapsed/work1.

Single input transformation: '[0min,2min]' → '[0min,3min]'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
