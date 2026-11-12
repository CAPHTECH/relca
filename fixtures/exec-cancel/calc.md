# F7: ack is not normal finish

Hand oracle, ADR 0011 and ADR 0010. Units min. Assignments Slow=p, Next=p,
Fast anonymous. Two starts orders Fast→Slow / Slow→Fast at0, both legal because
anonymous slots are independent. Receipt0 → starts0 → Fast.finish(pass)1 →
adopt fast/result_at1 → request Slow cancellation1 → ack2/release p →
Next.start(p)2 → Next.finish(p,ok)3 → done3. rejected never reached.
Slow retains p from0 through ack2; no Slow normal finish or commit occurs.
Controls #1 starts Slow at0 with default Permission={p}, satisfied.
#2 normal-finish antecedent absent: vacuous, despite G granting only q.
#3 left=∅/right={p}: vacuous. Safety satisfied, root vacuous/exit5.
Elapsed3, work Fast1+Slow2+Next1=4, result_at1, no truncation, W-CUT-0.
Public timing alone cannot prove p is held immediately after the request:
the join drains before Next is queued; the internal occupancy test is essential.
