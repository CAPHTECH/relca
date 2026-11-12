# X-D: queue is not completion

Only change capacity1 -> 0. initial0 received; queue1 creates x; no start
is legal and no clock advance is enabled; deadlock proof2 at0. Count2.
No tie. Waiting x/Worker, capacity0, occupancy0, holders[], obligations[],
join bindings[x]. Process line4 is context; property_location null;
ID diagnostic:W-DEADLOCK-0. Call line5 col5 byte181. All values arrays empty.
Safety remains satisfied; unreachable done violated is omitted with
non_path_property, not a fabricated path. Replay independently confirms
no legal successor. Root violated/1, complete/model, one verified candidate.
Exactly W-DEADLOCK-0/warning; no W-CUT-0 because Scope is explicit.
