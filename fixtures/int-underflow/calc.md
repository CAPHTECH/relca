# F03 — lower bound and signed exact attempt

1 − 1 = 0 is valid at minute 1. Initial 0 instead attempts −1: violation,
no terminal, before remains 0. Neither wrapping to u64::MAX nor clamping to 0
is permitted. Range stop is not truncated. Elapsed/work of the safe end: 1 min.
