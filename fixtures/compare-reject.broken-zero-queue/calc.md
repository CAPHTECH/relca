# F8 single fault: queue capacity zero

Exactly queue capacity1→0. Each of the arrivals0,1,2 sees waiting>=0 and is rejected before
Receipt, initialization or Task creation. Both plans: rejected count[3,3], each case rejected probability1,
all four times null, no done rows, query hit0 >=0 satisfied. Root satisfied/exit0, truncated false.
All three case IDs and their arrival times remain; no reindexing, no success and no cut fabrication.
Additional simulation with a latent initial distribution must record zero draws in both plans.
