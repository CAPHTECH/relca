# F2: FIFO, two simultaneous cases

ADR 0015 supplies admission and case-local time; ADR 0014 supplies independent duration hazards.
P uses slow, Q uses fast. Both cases arrive at 0; FIFO case 1 starts first on capacity 1.
P paths (D1,D2)=(1,1),(1,3),(3,1),(3,3), each mass (1/2)(1/2)=1/4.
Case 1 completion is 1,1,3,3; case 2 completion is 2,4,4,6.
Thus case 1 elapsed/completed_at min1 max3; case 2 min2 max6; work each min1 max3.
hit within 2 holds in two paths for case 1: 1/4+1/4=1/2; in one for case 2: 1/4.
There is no unresolved scheduling effect on these values, so min=max for each probability.
Q finishes at 1,2, work1 each, hit probabilities 1,1. Both done probabilities are 1 in both plans.
No any adoption, so result_at null. No queue limit, rejected count min=max=0.
Safety is global satisfied; the query >=1/4 is satisfied for each case, hence both plans/root satisfied.
diff must read case_outcomes and property.cases, not the identical empty top-level terminals.
