# pol-configuration

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P24: selecting CP yields A[0,2), CQ B[0,3). Default control follows selected process: CP policy[A],environment[B], CQ policy[B],environment[A]. Both simulations finish with assign; without it CP rejects at0 while CQ remains deterministic. Compare must carry exact selected reports, not reject configuration.

The pair changes only ' assign first_idle' to ''.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
