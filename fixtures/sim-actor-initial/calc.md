# F3: shared Actor initial state uses case=0

Independent printf %s -> sha256sum. u=first 16 hex/2^64. All n=1.

|full key|k|z|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=0\|site=init.actor.A.z\|n=1|8bda5db034293362|second|
|relca-sim-v1\|seed=0\|rep=2\|case=0\|site=init.actor.A.z\|n=1|9ad17a776c3b5ab0|second|
|relca-sim-v1\|seed=0\|rep=3\|case=0\|site=init.actor.A.z\|n=1|75b2682bb8401ac7|first|
|relca-sim-v1\|seed=0\|rep=4\|case=0\|site=init.actor.A.z\|n=1|4ed60c1140e8d9b0|first|

h2/u0/n4, gamma1/2,m1,alpha1/4. G2(p)=6p²-8p³+3p⁴.
Outward roots on Q=2^20 grid [254827/Q,793749/Q], adjacent grid points bracket alpha1/4.
Threshold1 gives satisfied. done4; cut/reject/deadlock0. No production helper used.
