# pol-tie-nondet

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P19: both work[0,2), work4, visible result2, complete2. Initial coin mass1/2 unaffected. Fixed tie has only environment completion-order choices so fixed-policy. Nondet tie has two winners once both finish, so full-information.

The pair changes only 'tie by [a,b]' to 'tie nondet'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
