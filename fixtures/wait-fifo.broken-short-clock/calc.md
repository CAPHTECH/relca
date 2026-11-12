# wait-fifo.broken-short-clock

ADR 0048; design v0.2 §22.3. Units: minutes.

T08: clock12 truncates while B case2 and queued case3 remain. All NEW scalars null/truncated, including the completed case1. No dwell or bound; window20 is unchanged.

[R] Hand interval arithmetic, not captured collector output. Stage C acceptance stops RED; production fault calibration not performed.
