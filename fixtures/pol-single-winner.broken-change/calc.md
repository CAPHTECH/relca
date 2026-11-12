# pol-single-winner.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

TIE single winner: only A returns pass; B ok does not qualify. Both complete2, work4, coin1/2. Nondet syntax alone does not create two policy winners; both sides fixed-policy.

The pair changes only 'tie by [a,b]' to 'tie nondet'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
