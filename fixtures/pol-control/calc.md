# pol-control

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P06: same occupancy as P01. Explicit environment has an empty policy set, valid lowering but two runtime executor successors at0. Reject in simulation, not statically.

The pair changes only 'control policy' to 'control environment'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
