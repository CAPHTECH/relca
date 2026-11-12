# X-Q: successful prior bundle

ADR 0010 requires a prior Gate.check.finish(pass), not any finish.
pass path initial0/queue1/start2/advance3(t1)/finish4(pass)/join5/branch6/
queue7/start8/advance9(t2)/finish10(ok)/join11/end12. Fail ends returned
after branch6/end7 at1 and never starts Worker. satisfied/0, no path, [].
gate line7 col5 byte331; x line9 col5 byte401; clause line15.
No state values; anonymous executors null. No replay candidate.
