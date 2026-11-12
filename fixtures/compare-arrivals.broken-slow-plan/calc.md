# F2 single fault: Q also calls slow

Exactly one call replacement A.fast()→A.slow(). Each plan now has the four equiprobable
duration pairs (1,1),(1,3),(3,1),(3,3), mass 1/4 each. Each case reaches done with probability 1.
Case 1 completes in [1,3], case 2 in [2,6], each work [1,3], result_at null.
Both plans have hit within2 probabilities (1/2,1/4); each >=1/4, global safety satisfied.
No rejection, no truncation, root satisfied/exit0. Value changes, not verdict, detect this fault.
