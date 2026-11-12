# F4 single fault: bypass rejection

Only rejecting else end removed. received0 -> Gate.start0 -> finish(pass/fail)1 ->
Worker.start1 -> finish2 -> end.done2. elapsed=work=2 minutes on both paths.
pass: clause #1 has preceding finish(pass). fail: no finish(pass) on this path, so violated.
An A on another path cannot satisfy B here, nor can finish(fail) match finish(pass).
Aggregate #1/root violated/exit1, safety satisfied, no cut, result_at null.
