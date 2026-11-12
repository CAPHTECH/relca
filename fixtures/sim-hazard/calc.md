# F2: conditional hazards, not a start-time duration draw

Independent printf %s -> sha256sum, k is first 16 hex, u=k/2^64.
Positive support 1,3min. At 1min q=1/2, at 3min q=1. Final q=1 still draws.

|full key|k|branch|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=duration.P.a\|n=1|bbad16507b96282d|survive|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=duration.P.a\|n=1|f87320680b90291e|survive|
|relca-sim-v1\|seed=0\|rep=3\|case=1\|site=duration.P.a\|n=1|2e7af6878c92ea58|finish|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=duration.P.a\|n=1|5547fe5933e8c7ce|finish|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=duration.P.a\|n=2|d7580bbcbd5e6e47|finish(q=1)|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=duration.P.a\|n=2|4d87db4b71417a21|finish(q=1)|

Ended at (3,3,1,1). n=4 trials, h4/u0, done4. gamma1/2,m1,alpha1/4.
L4 solves p^4=1/4. Grid Q=2^20: 741455^4 <= Q^4/4 < 741456^4.
Outward interval [741455/Q,1]. >=0 satisfied. No cut/rejection/deadlock.
Independent unused-key observations (not draws): n2 rep3=257715183a50561f, rep4=9320cdcf60be5266.
Full strings are the corresponding n1 keys above with n=2. They must NOT be consumed in this fixture.

## Public three-candidate occurrence control (not the fixture population)

Support {1:1/4,2:1/4,3:1/2} gives hazards 1/4,1/3,1. The same n1/n2 keys give
finish times (3,2,1,3), hence within2 truth (miss,hit,hit,miss), h2/u0/n4.
Additional actual q=1 draws:

|full key|first16|branch|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=duration.P.a\|n=3|e1d8603fc04786a6|finish|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=duration.P.a\|n=3|3c88ef95b604ecad|finish|

Independently observed via printf %s and sha256sum. Rep1/4 consume n1,n2,n3;
rep2 n1,n2; rep3 n1. The final probability-one draw is mandatory.
