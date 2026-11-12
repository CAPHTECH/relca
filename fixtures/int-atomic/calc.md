# F25 — preflight entire write bundle

Safe finish at t1 atomically produces flag=true,n=1; done time/work1 min.
Only initial n0 → n1 makes attempted n2 invalid. Neither write applies:
flag remains false and n1. Failed attempt emits no Finish/Commit/Write and
publishes no result, despite flag being the first source write.

The supplemental START-overflow input places the same flag/n writes in start,
with no implicit finish writer. The attempt occurs at time0: initial(received),
queue edge1, range_violation edge2. Before is flag=false/n1, attempted2, and
there is no successful Start or resource allocation. Shortest count2 is fixed
before implementation, distinct from the four-edge failed finish prefix.
