# F1: distinct executors

Hand oracle from ADR 0011, not implementation output. Times and work are min.
The only assignment is Clerk=c1, Approver=a1. Receipt0 → Clerk.start(c1)0
→ Clerk.finish(c1,ok)1 → Approver.start(a1)1 → Approver.finish(a1,ok)2 → end.done2.
Each person holds at most one Task. Safety is satisfied. Controls #1 has
left={c1}, right={a1}; both events occur on this path, intersection is empty:
separation satisfied. Root satisfied, exit0. Elapsed=1+1=2, work=1+1=2,
result_at=null. No cut (clock20, live_tasks2). Undeclared Cut gives W-CUT-0.
The broken pair adds c1 to Approver and changes no other semantic field.
