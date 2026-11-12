# F26 — cancellation preserves start but suppresses finish

Both actors start at0. Slow start n0→1. Fast wins at1; immediate slow cancel
does not undo start or run finish. n1, end1, work1+1=2 min, result_at1.
Changing only remaining to drain lets slow finish at3: n1→2, drained end3,
work1+3=4 min, result_at1. Only Slow writes n, no sibling write conflict.
