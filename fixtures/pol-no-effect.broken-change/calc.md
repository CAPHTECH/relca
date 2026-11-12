# pol-no-effect.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P22: anonymous capacity2 starts0,1 and ends3,4,work3 each. assign has no application target; assumption says so. Replacing only capacity with named executors uses c1,c2 and removes that no-op suffix. Both samples count4 each.

The pair changes only 'capacity 2' to 'executors [c1,c2]'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
