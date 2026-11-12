# F7 single fault: Slow cancel → forbidden_after_start

Hand oracle from ADR 0011. Units min. Fixed Slow=p/Next=p, Fast anonymous.
Fast/Slow start0 in either order. Fast.finish(pass)1 → adopt1/result_at1 →
Slow ignores request and holds p → Slow.[finish(p,ok),commit(p)] bundle5 →
Next.start(p)5 → Next.finish(p,ok)6 → done6. No rejected path.
#1 permitted at0: p∈default {p}, satisfied. #2 authority_basis at5:
p∉G.Slow.work={q}, violated. #3 left={p} at5/right={p} at6: violated.
Safety satisfied; root violated/exit1. Elapsed6/work=1+5+1=7/result_at1,
not truncated, W-CUT-0, no runtime E-CON-0. This differs from the base by one
cancel policy only and detects counting Permission as sufficient Authority.
