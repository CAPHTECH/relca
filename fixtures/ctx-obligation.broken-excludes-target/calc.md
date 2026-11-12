# F2 single fault: A result ok to bad

A returns bad at 1. The exclusion activates, but B is not called, so excludes
is satisfied. Wait runs 1..6; received's response deadline is 4. No B.finish
occurs: expiry at 4, status violated forever. Exclusion must not erase it.
End done 6, work 1+5=6, root violated/exit1. W-CUT-0 required.
The plan's arithmetic is retained. Expiry resolved_at is deadline 4, not end 6.
