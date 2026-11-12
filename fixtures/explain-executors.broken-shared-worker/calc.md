# X-E: same person across two differently named Actors

Only change Worker.executors [q] -> [p,q]. Choose p for Worker after p is
released by Clerk, a legal serial schedule. initial0/queue1/start2(p)/
advance3(t1)/finish4(entry=ok,p)/join5/queue6/start7(p)/advance8(t2)/
finish9(x=ok,p). permitted first fails at7/time1; separation and
authority_basis first fail at9/time2. Three candidates in property order:
separation9, permitted7, authority_basis9. Stop each at its own first failure.
q gives no violation and cannot be substituted for p. No equal-length
failing executor alternative. entry line9 col5 byte390; x line10 col5
byte416. Clauses lines17/18/19 col5 bytes623/676/701. Permission line6
and Grant line7 are related positions, not property positions. Empty state
values. Replay verified for all3; root violated/1, complete/model, [].
