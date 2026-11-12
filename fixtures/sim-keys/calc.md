# F1: two independent sites, ADR 0016 and its addendum

Independent observations: printf with format %s and no newline, piped to sha256sum.
Each k is the first 16 hexadecimal digits, big endian; u=k/2^64.
All occurrences n=1. Initial support is TYPE order, not textual distribution order.

|full key|k|selected branch|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=init.case.z\|n=1|3cc4caa41f1f0220|first|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=init.case.z\|n=1|53ad98eb84e4e959|first|
|relca-sim-v1\|seed=0\|rep=3\|case=1\|site=init.case.z\|n=1|6633fefeaffe12b1|first|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=init.case.z\|n=1|77c436e71ee481e8|first|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=kernel.P.a\|n=1|aefe043a58100d00|second/no|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=kernel.P.a\|n=1|c29df2dac711d230|second/no|
|relca-sim-v1\|seed=0\|rep=3\|case=1\|site=kernel.P.a\|n=1|01aeb446fbdcaceb|first/yes|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=kernel.P.a\|n=1|3304734885e3586c|first/yes|

n=4; (hits,unknown)=(4,0),(2,0). m=2, gamma=3/4, alpha_before=1/8, alpha=1/16.
G(4;1/2)=1/16 gives exact L4=1/2. Query 1 is satisfied at equality.
For h2, G(2;p)=6p²-8p³+3p⁴. Direct rational bisection on grid Q=1048576 gives
L2 floor=115452/Q=28863/262144 and U2 ceil=933124/Q=233281/262144.
At the adjacent lower grid point the polynomial is <=1/16; at the next point it is >1/16.
Terminal counts yes2/no2; no cut/reject/deadlock. No production helper used.

## Serial second binding public control (separate from the 24 fixtures)

Same capability/kernel, new lexical binding b. Independently observed keys:

|full key|first16|branch|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=kernel.P.b\|n=1|b70da42f94f667f7|second|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=kernel.P.b\|n=1|47e6d86cc006a43b|first|
|relca-sim-v1\|seed=0\|rep=3\|case=1\|site=kernel.P.b\|n=1|dc863fcadd33b2f7|second|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=kernel.P.b\|n=1|b27d55082fe67e8c|second|

u=first16/2^64, cumulative1/2. The reached y==first flags after a and b are
(miss,hit,hit,hit), h3/u0/n4. r3/4 remain hits after b writes second: reached is sticky.
