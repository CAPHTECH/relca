# F8: order-sensitive response

Hand sequence: received0, A.start0, A.finish1 (trigger, deadline2), B.start1,
B.finish2 (target inclusive), Slow.start2, Slow.finish4, end.done4.
#1 satisfied, safety satisfied; elapsed/work=1+1+2=4 minutes, result_at null.
No cut, root satisfied/exit0. Pair changes only A/B scheduling from serial to fork-all.
ADR 0010 addendum requires target to follow trigger in event order, not merely timestamp.
