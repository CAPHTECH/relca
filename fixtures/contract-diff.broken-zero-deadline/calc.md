# F7 single fault: zero deadline

Only by1 -> by0. Before: received0, start0 creates deadline0, no target at0,
finish1 is too late, end.done1. #1/root violated, safety satisfied, elapsed/work1.
After: received0, end.done0, no activation; #1/root vacuous, safety satisfied, elapsed/work0.
Both complete uncut, result_at null. Diff root violated/exit1 (violated outranks vacuous).
Static removed Worker/Worker.run, reason/loses, common Cut and property inventory
are identical to the correct pair. Values fixed from ADR 0010 and arithmetic.
