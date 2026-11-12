# M-sample / M-keys

ADR 0016 key is relca-sim-v1|seed=41020|rep=r|case=1|site=duration.P.job|n=j.
No newline. Independently observed with printf and sha256sum; first16:
r1 n1=5bfb4527c5df896e; r2 n1=ce340db047edc48c,n2=e4d6cb1e80c28287;
r3 n1=75d284aacbfbe6d8; r4 n1=801080fe22aaf3e4,n2=decba0888acca7c7.
At2 hazard1/2: r1/3 finish, r2/4 survive; final hazard at4 is1 and still draws.
Six used keys. Paths [0,2),[0,4),[0,2),[0,4); lead/work2,4,2,4.
Sum12/4=mean3, min2,max4,n4. Wait0/peak1/starts1 every rep.
Utilization1/5,2/5,1/5,2/5 -> mean3/10, extrema1/5..2/5,n4.
Costs17,31,17,31 -> sum96/4=24, extrema17..31,n4; occupancy mean21,
start fee3. Completed total4, no excluded cases. Safety makes root inconclusive.
The same first3 reps give lead8/3 and cost65/3; not integer truncation.
