# F4: successful finish before start

ADR 0010 hand oracle. received0, Gate.start0, Gate.finish(pass/fail)1.
pass: Worker.start1 follows the separate Gate.finish(pass) bundle at the same time;
Worker.finish2, end.done2. Clause #1 true, elapsed/work2 minutes.
fail: end.rejected1, no Worker start, no activation; elapsed/work1.
Aggregate activation and no violation/cut -> #1/root satisfied, exit0; safety satisfied.
result_at null. Pair removes only the rejecting else end.
