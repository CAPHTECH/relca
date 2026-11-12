# F5 single fault: activate impossible branch

Only Gate results gain fail. pass path still ends done1 with work1 and no activation.
fail path: received0 -> Gate.start0 -> finish(fail)1 -> Effect.start1 ->
finish(ok)+commit2 -> done2; elapsed/work2 minutes, result_at null.
#1 false at commit2 -> violated. #2 one commit -> satisfied (activation is the first commit).
#3 start1 creates deadline2, finish2 fulfills inclusively -> satisfied.
#4 start1 lacks preceding Gate.finish(pass) -> violated. Aggregation is not a fold
of per-path vacuity: active nonviolated #2/#3 are satisfied despite the pass path.
Safety satisfied; root violated/exit1; no cut. Independent hand oracle.
