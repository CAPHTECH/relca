# F2 single fault: enable second callsite

Only result set changes: add again. Hand paths after received0:
1. start(x)0 -> finish(ok)+commit1 -> end.done1: count1, work1.
2. start(x)0 -> finish(again)+commit1 -> start(y)1 -> finish(ok)+commit2 -> done2.
3. As path2, but y=again. No loop: still only two calls, work2.
Paths2/3 count the same Capability twice despite different bindings. Clause #1 violated.
done elapsed/work range1..2 minutes, result_at null; safety satisfied, root violated/exit1.
No bounds reached. Oracle is arithmetic plus ADR 0010, not observed output.
