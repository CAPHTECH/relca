# F12: fixed-key reproducibility and statistical coverage are different obligations

Analytical rate 1/2. Independently observed BEFORE running simulate: hits=4956, misses=5044,
unknown0, n10000. SHA loop executed printf %s KEY piped to sha256sum for every r=1..10000.
The full key for each integer r (ordinary decimal without zero padding) is exactly:

    relca-sim-v1|seed=0|rep=r|case=1|site=kernel.P.a|n=1

Here r is substituted, not a literal. For example the first four full keys and first16 digits are:

|key|first16|selected branch|
|---|---|---|
|relca-sim-v1\|seed=0\|rep=1\|case=1\|site=kernel.P.a\|n=1|aefe043a58100d00|hit=false|
|relca-sim-v1\|seed=0\|rep=2\|case=1\|site=kernel.P.a\|n=1|c29df2dac711d230|hit=false|
|relca-sim-v1\|seed=0\|rep=3\|case=1\|site=kernel.P.a\|n=1|01aeb446fbdcaceb|hit=true|
|relca-sim-v1\|seed=0\|rep=4\|case=1\|site=kernel.P.a\|n=1|3304734885e3586c|hit=true|

At every r, n=1 and first branch iff unsigned first16 < 8000000000000000; otherwise second.
gamma=999999999999/1000000000000; m3; alpha_before=1/2000000000000; alpha=1/6000000000000.
Independent integer polynomial tails use sum C(10000,j) a^j (d-a)^(10000-j) / d^10000.
At p=3/4 F(4956;p)<alpha, at p=1/4 G(4956;p)<alpha. At p=1/2 both tails exceed alpha.
For p=1/2 exact tails are sums of C(10000,j)/2^10000, not normal approximations.
Hence verdicts S/S/I, root I. done10000, no cuts/reject/deadlock.

Outward CP grid Q=1048576: lower=481522/Q=240761/524288,
upper=557860/Q=139465/262144. Direct exact polynomial bisection gives
G(4956;481522/Q)<=alpha<G(4956;481523/Q),
F(4956;557859/Q)>alpha>=F(4956;557860/Q).
The interval includes 1/2. No product statistics helper used.

Assuming ideal independent Bernoulli(1/2), simultaneous CP noncoverage <=10^-12;
P(abs(h/10000-1/2)>.1)<=2 exp(-200)<2.8e-87. This is not a proof of SHA independence.
Quantization bias for this particular dyadic distribution is zero; general bound is 2^-64 per category.
