# F8 single fault: parallelize A/B

Only A/B calls become fork-all. received0, both starts0 in order AB or BA.
Both finish1, independently in order AB or BA: four start/finish-order paths.
Finish AB (two paths): trigger1 then target1 discharges deadline2 -> satisfied locally.
Finish BA (two paths): target1 is earlier than trigger1 in sequence and cannot fulfill it;
trigger1 leaves deadline2 pending; Slow.start1 -> finish3 -> done3 proves violation.
Both variants have elapsed3, work1+1+2=4 minutes, result_at null, safety satisfied, no cut.
Aggregate #1/root violated/exit1. Pending monitor differences must survive state merging.
Mirror-direction unit tests complement this oracle; no implementation output used.
