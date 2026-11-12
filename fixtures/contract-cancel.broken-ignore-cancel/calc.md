# F9 single fault: Slow ignores cancellation after start

Only Slow cancellation policy cooperative -> forbidden_after_start.
received0; starts Fast/Slow at0 in either order; Fast.finish(pass)1 -> adoption1.
Slow continues, finish(ok)+commit3 -> end.done3. #1 reads published slow=ok, true;
#2 count1, satisfied; #3 trigger3, deadline4, end.done3 fulfills -> satisfied.
Safety/root satisfied/exit0, elapsed3, work1+3=4 minutes, result_at1, no cut.
Fault is detected by the expected contract/metric difference, not a root failure.
No implementation output used to derive these values.
