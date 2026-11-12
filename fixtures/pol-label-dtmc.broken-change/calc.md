# pol-label-dtmc.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P08: starts0,1 and finishes3,4 are distinct. Each bad=1/10; no unresolved choice so no scheduler row. One arrival changed to0 introduces two cofinishes and fixed-policy row, not a policy choice.

The pair changes only '0min,1min' to '0min,0min'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
