# pol-observable-assign.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P14: observable changes only queue spelling, not semantics. Assignment retained yields P01 times/counts. Removing only assignment retains observable but cannot choose between c1/c2 at0.

The pair changes only ' assign first_idle' to ''.
This is the single edit side. Expected verdict: unsupported; exit 4.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
