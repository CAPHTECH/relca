# F2: union of explicit permissions

Hand oracle, ADR 0011. Units min. Exactly two assignments: Runner=p or q.
For either: receipt0 → start(person)0 → normal finish(person,ok)1 → done1.
The two permit rows union to {p,q}. Controls #1 activates at start0 and
satisfies on both paths. Safety/root satisfied, exit0. Elapsed1, work1,
result_at null, no truncation, W-CUT-0. No Authority is inferred or required.
All Runner capabilities have explicit Permission: no Runner.permission default
assumption, and named executors imply no Runner.executors anonymous assumption.
