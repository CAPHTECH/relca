# pol-control.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P06: same occupancy as P01. Explicit environment has an empty policy set, valid lowering but two runtime executor successors at0. Reject in simulation, not statically.

The pair changes only 'control policy' to 'control environment'.
This is the single edit side. Expected verdict: unsupported; exit 4.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
