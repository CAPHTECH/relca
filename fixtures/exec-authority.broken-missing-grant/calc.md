# F3 single fault: delete grants q row from Grant

Hand oracle, ADR 0011. Units min. Runner=p or q remains legal; Permission={p,q}.
Both paths: receipt0 → start0 → [finish(ok),commit] bundle1 → done1.
Grant.run={p}: p path satisfies Controls #1 at1, q path violates at1, despite
having Permission. Aggregate authority_basis/root violated, exit1; safety satisfied.
Elapsed1/work1/result_at null, not truncated; W-CUT-0, no E-CON-0.
At least one grants row remains, so this is a runtime Authority failure, not a
missing-required-field or missing-target static failure. issued_by/not_by unchanged.
