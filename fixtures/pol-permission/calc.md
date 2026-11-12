# pol-permission

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P20: first declared p starts[0,1), despite permit only q. permitted violated. Swap order: q works, satisfied. Permission/Authority are monitors, never candidate filters. Unused executor warning changes only person.

The pair changes only 'executors [p,q]' to 'executors [q,p]'.
This is the positive side. Expected verdict: violated; exit 1.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
