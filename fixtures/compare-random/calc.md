# F3: independently reobserved SHA and exact CP arithmetic

ADR 0016 key: relca-sim-v1|seed=0|rep=R|case=C|site=S|n=1, UTF-8, no trailing newline.
Observation command is printf with format %s piped to sha256sum. The first 16 hex digits
are interpreted as k, with u=k/2^64. Type order first,second and exact k<2^63 select first.

|rep|init.case.z C1|init.actor.A.z C0|kernel.P.a C1|kernel.Q.a C1|duration.P.a|duration.Q.a|
|---|---|---|---|---|---|---|
|1|3cc4caa41f1f0220|8bda5db034293362|aefe043a58100d00|b610963c12b20e62|bbad16507b96282d|4a99d315c3c8a5d3|
|2|53ad98eb84e4e959|9ad17a776c3b5ab0|c29df2dac711d230|af57dc99b0bcf142|f87320680b90291e|7867a4b3b66ea36a|
|3|6633fefeaffe12b1|75b2682bb8401ac7|01aeb446fbdcaceb|c649c3250a0087d2|2e7af6878c92ea58|19c3e01b3c76b273|
|4|77c436e71ee481e8|4ed60c1140e8d9b0|3304734885e3586c|90e2ba6187db292f|5547fe5933e8c7ce|8fce637435feb51c|

The first 20 entries (excluding duration.P.a) matched an earlier observation in an independent rerun.
Case z is first in all four reps; Actor z second,second,first,first.
P result second,second,first,first; Q always second. Each distributed duration draws once even
with probability1, completing at 1; all cases admitted at 0. There are 4 draws per plan/rep,
32 records total, only the two initial keys shared. Kernel/duration keys include P or Q.
The duration hash never chooses between two times: the sole time has mass1.
P no2/yes2, Q no4/yes0; unfinished=deadlocked=0 and totals per case/plan are 4.
Queries (case z, Actor z, observed y) hits P=(4,2,2), Q=(4,2,0), unknown=0,trials=4.
gamma=5/8; m=3 within EACH plan; alpha_before=3/16, alpha_after=1/16 (not divided by plans).
G4(p)=p^4: at p=1/2, G4=1/16 exactly, so lower endpoint=1/2 and >=1/2 is satisfied.
G2(p)=6p²−8p³+3p⁴. On denominator Q=1048576, exact integer evaluation shows
G2(115452/Q)<=1/16<G2(115453/Q). The outward lower endpoint is
115452/Q=28863/262144, upper by symmetry 933124/Q=233281/262144.
h4 interval [1/2,1], h2 interval [28863/262144,233281/262144], h0 interval [0,1/2].
The other queries have upper threshold1 and are satisfied. Both plans/root satisfied, exit0.
Intervals concern each plan only, not cross-plan differences or simultaneous coverage.
