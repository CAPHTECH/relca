# F13 — evaluation time

Start at t0 reads 0 and stores 1; finish at t1 must read that 1, storing 2.
Invariant sees the postwrite 2. elapsed/work 1 min. Changing only start operand
to 0 gives 0 → 0 → 1, invariant violated while S remains satisfied.
