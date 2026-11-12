# wait-reject.broken-overflow

ADR 0048; design v0.2 §22.3. Units: minutes.

T15: capacity1 counts admitted case1 at0; case2 is rejected before Task creation. Case1 run0..2. Rejected breakdown null/rejected; all done rows for case2 no_terminal. Overflow control records spill at receipt0 with lead0 and all breakdown0, no fake task or dwell.

[R] Hand interval arithmetic, not captured collector output. Stage C acceptance stops RED; production fault calibration not performed.
