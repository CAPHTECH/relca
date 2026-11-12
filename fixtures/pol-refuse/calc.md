# pol-refuse

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P11: case1 post starts0, writes posted=1 at1. Case2 is selected next and requires false refuses at1 with work0 and no occupancy. Removing requires attempts2 above upper1 at2; no successful case2 terminal. Refusal does not reserve an executor or increment the state.

The pair changes only '      requires posted == 0 else rejected\n' to ''.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
