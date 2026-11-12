# X-Q: fail is not pass

Only change: else no longer ends returned. Hand path initial0, queue1 gate,
start2, advance3(t1), finish4(gate=fail), join5, branch6 else, queue7 x,
start8 x. Count8, time1. Worker.run starts without an earlier successful
Gate.check bundle. Earliest failure is start, not finish at2.
gate Gate.check line7 col5 byte331; x Worker.run line9 col5 byte388;
clause line15. All values arrays empty, anonymous executor null, start result
null. No alternate failing result or tie. Replay verified; root violated/1,
complete/model; diagnostics [].
