# lin-correlation — hand oracle before implementation

The calculation uses lexical call identity and source ancestry.
This is a lexical structural analysis, not proof of real-world independence.
Warnings do not alter the root verdict. No schedule is replaced by a random choice.
The mutation wrapper keeps W-LIN-2; Report.agency.unchecked does not.

F5: a is the result written by aV in G(e), so given[bad,a] declares correlation and suppresses bV. All six rows pass, Pr(done)=1, 3 min.

## Boundary corrections M1/M2/M3 and m7 (fixed before running implementation)

E(aV)={S,R}, E(bV)={S}; neither is the other's argument ancestor. Without
correlation, only bV warns. Sequential aV then bV guarantees a is written before
bV starts, so bV instead has kind wlin2_correlation, judgements=[aV],
targets=[e], error_latents=[bad]. New rules wlin2-suppression-p1-1.
No basis field. All six rows are deterministic pass: outcome never depends on
the optional value of a. The existing unrelated-given fault reads c (no writer),
so its inventory remains [bV], no model_uncertainties.

| Dedicated source transformation | W-LIN-2 calls | Suppression rows | Elapsed/work min |
|---|---|---|---|
| e1: bV before aV | bV | none; a is not yet written | 3/3 |
| e4: remove R, KA given [bad,b], KB given [bad,a] | aV,bV | none; mutual correlation is expressly rejected even in serial code | 3/3 |
| fork aV and bV, join all | bV | none; one schedule starts bV before aV finishes | 2/3 |
| bV consumes aV as actual argument | none | bV correlation; argument ancestor guarantees completion | 3/3 |

m7 group restriction: insert another producer e2 (same S, distinct call/writer),
and another judgement cV=C.check(e2) before aV and bV. C.check has the same KA,
return type and reads R as A.check, but writes c rather than a. Put c into
KB.given and retain aV=A.check(e), which still writes a. Only aV and bV
are in G(e); cV belongs to G(e2). bV must still warn, no uncertainty row.
Changing B.check(e) to B.check(e2) is a positive control: bV correlates with cV,
and aV is a singleton. All 5 serial Tasks pass in 5 min, Pr(done)=1.

m7 result type: the public IR boundary test starts from the valid correlation
model, adds an isomorphic but distinct enum OtherVerdict={pass,fail}, then changes
only observed a.base to OtherVerdict. Its value ordinals still coincide but its
type identity differs from A.check's return type. This variable is not a valid
result alias: agency::derive must retain bV's warning (or reject malformed IR),
never silently suppress it. The unmodified typed IR is the positive control.

Exact W-LIN-2 call inventory: [].
No other warning is expected (explicit Cut; no committing effect).
Every Task has positive duration; serial durations add to work and elapsed.
All routes finish before clock 30 min, safety satisfied, no truncation.
No oracle is copied from a new binary. Extra boundary cases live in dedicated tests.
