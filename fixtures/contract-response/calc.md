# F3: inclusive deadline and end target

Hand sequence: case.received0 (trigger, deadline0+5=5), Worker.start0,
Worker.finish(ok)5, end.done5. End target is processed before closing pending obligations.
5 <= 5, so clause #1 satisfied. elapsed=work=5 minutes; no any, result_at null.
Safety/root satisfied, exit0, no truncation; W-CUT-0 required.
Pairs change exactly duration to6 or clock to4. No unresolved business end is synthesized.
