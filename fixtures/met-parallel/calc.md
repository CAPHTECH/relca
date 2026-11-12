# M-parallel / M-denominator

ADR 0019, plan F2. Both tasks ready/start at 0, intervals [0,3),[0,5).
End/lead=max(3,5)=5, work=3+5=8, all waits 0, peak=2.
Capacity 2 and common window 10 give utilization=8/20=2/5.
One start of each capability: occupancy 800 + start fees 2+3 = 805.
One completion, no exclusion. All schedules have the same scalar values.
