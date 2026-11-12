# F6: named pool and equal explicit capacity

Hand oracle, ADR 0011. Units min. A.capacity1 equals |{p}|, legal.
Only assignment A=p/B=q: receipt0 → A.start(p)0/finish(p,ok)1 →
B.start(q)1/finish(q,ok)2 → done2. Controls #1 left={p}/right={q}, disjoint,
active satisfied; #2 starts A at0 with default Permission={p}, satisfied;
#3 normal finish A at1 with G.A.a={p}, satisfied even without commits.
Safety/root satisfied, exit0, elapsed/work=1+1=2, result_at null,
no truncation, W-CUT-0. Default Permission assumptions apply to both Actors;
neither has an anonymous executor assumption.
