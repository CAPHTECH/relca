# F8: admission mass is not renormalized

All task results deterministic singleton; the probability query makes terminal probabilities explicit.
Three arrivals0,1,2; capacity1, FIFO, queue1 counts admitted cases that NEVER started a task.
P case1 runs [0,4), case2 waits from1 then runs [4,8); at2 case2 is waiting, so case3 rejected.
P completed_at=(4,8), elapsed=(4,7), work=(4,4), result_at null. Each reached done mass1;
case3 rejected mass1, ALL four times null, not a zero-minute completion. hit=(1,1,0).
Q runs cases [0,1),[1,2),[2,3), completed_at=(1,2,3), elapsed/work1, all hit/done1.
At arrival1/2 earlier cases already started, so they do not count as admission wait even before finish.
All paths have mass1, so all min/max endpoints equal; rejected_count P[1,1], Q[0,0].
Query >=0 satisfied for every case (including rejected case), root satisfied/exit0.
Comparative terminal rows include done and rejected for EVERY potential case; absent exhaustive
outcomes are not_reported/null, never invented probability0. Additional simulation tests add
latent case z: only admitted IDs draw initial keys, while metadata lists potential IDs1,2,3.
