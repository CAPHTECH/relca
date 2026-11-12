# pol-deadlock.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P23: H cannot obtain S0, A waits S0 and heads R, B cannot overtake A on R. No task starts, clock0, work0, genuine deadlock, not truncated. Three unstarted capabilities and done unreachable. Restoring S1 yields P16 completion6/work6.

The pair changes only 'resource S capacity 0' to 'resource S capacity 1'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
