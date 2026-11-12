# pol-latent

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P10: (T,T),(T,F),(F,T),(F,F) have mass1/4. All start case1 at0, case2 at2, finish2,4 and work2. Each marginal bad1/2. In particular (F,T) must not reverse. then hard is a syntax refusal, not latent-policy semantic error.

The pair changes only 'then call_order' to 'then hard'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
