# F7: retirement is vacuous, not satisfaction

Independent ADR 0010 hand oracle. Before: received0, Worker.start0 creates deadline1,
finish1 fulfills inclusively, end.done1. #1/safety/root satisfied; elapsed/work1 minute.
After: received0, end.done0. No start trigger, no cut: #1 vacuous, safety satisfied,
root vacuous; elapsed/work0. Both result_at null and truncated false.
Both reports retain identical property IDs. Diff row #1 is satisfied -> vacuous,
root vacuous/exit5. Worker and Worker.run are removed, each with matching intent reason/loses.
Common Cut C includes Worker, so no W-CUT-0 or unexplained-difference warning.
Pair changes only response deadline1 ->0; process times do not change.
