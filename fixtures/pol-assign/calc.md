# pol-assign

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P01: c1 occupies [0,3), c2 [1,4). Each elapsed/work=3. Four full replications give done count4 per case. Without assign, time0 has two executor choices and replication1 aborts, not a uniform draw.

The pair changes only ' assign first_idle' to ''.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
