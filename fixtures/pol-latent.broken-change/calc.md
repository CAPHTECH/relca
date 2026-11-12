# pol-latent.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P10: (T,T),(T,F),(F,T),(F,F) have mass1/4. All start case1 at0, case2 at2, finish2,4 and work2. Each marginal bad1/2. In particular (F,T) must not reverse. then hard is a syntax refusal, not latent-policy semantic error.

The pair changes only 'then call_order' to 'then hard'.
This is the single edit side. Expected verdict: unsupported; exit 4.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
Observed once with the earlier binary: a parse failure ends report::check_with before the Cut is projected, and report::finish adds W-CUT-0 (span null) after the syntax error. This is existing behavior.
