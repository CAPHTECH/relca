# pol-separation

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P02: enter [0,2), approve [2,3), work=2+1=3. First idle yields c1 then c2: separated. Swapping only Clerk order yields c2 then c2: violated. a1 is unused; in the bad side c1 is also unused. Agency potential people are unchanged.

The pair changes only 'executors [c1,c2]' to 'executors [c2,c1]'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
