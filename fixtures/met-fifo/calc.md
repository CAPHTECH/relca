# M-fifo

ADR 0015 FIFO compares currently queued tasks; ADR 0019 measures queued to start.
Arrivals 0,1,2. A intervals [0,3),[3,6),[6,9); ready=0,1,2.
B ready=3,6,9; intervals [3,8),[8,13),[13,18).
First and a waits (0,2,4); b waits (0,2,4), not arrival-to-B-start.
End times (8,13,18); leads (8,12,16); work per case 8.
A work9, B work15, total24, peak2, actor peaks1.
Utilization A9/20, B15/20=3/4. Starts 3 each.
Cost=9*2+15*3+3*1+3*4=78; per-arrival and terminal per-case=26.
Counts offered=received=completed=3, all other counts0; all min=max.
