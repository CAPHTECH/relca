# X-O: repeated capability at a different call site

Only change is result support ok -> ok,again. Hand path initial0, queue1 x,
start2, advance3(t1), finish4(x=again,commit), join5, branch6 then,
queue7 y, start8, advance9(t2), finish10(y=ok,commit). Count10.
Second result again is an equal-length counterexample, but ok is first in
the declared result order and must win. A single call cannot violate once.
Call x line5 col5 byte205, y line6 col21 byte243, both Ledger.post; their
capability_location is identical while their call locations differ.
Clause line12 after contract Controls opening. Empty values; anonymous
executors null. finish contains finish then commit, one bundle. Expected
replay verified, root violated/1, complete/model, diagnostics []. This is a
finite second call, not a claim that response-loss retry is implemented.
