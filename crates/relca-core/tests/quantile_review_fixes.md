# C7 review acceptance — hand calculations fixed before execution

Authority: ADR 0049, adopted plan 2.3–2.8, and the two c7 rulings of
2026-10-29. Inline sources do not enlarge the closed 67-name qtl population.

## R1 / M1 / driver lifetime

Ledger arrivals are 0 and 2 minutes; both adds take one minute. The shared
integer is 0 at 0, 1 at 1, and 2 at 3. Case 1 ends and has no route at 1:
its observation is closed before case 2 is received. For n==2 its first hit
is Never; case 2 first hits at elapsed 3−2=1. Reached mass is 1/2 < p=1.
All therefore has no finite value, beyond_reached_mass and no lower bound;
Given has value 1 (exact in exhaustive). Simulation N=2,C=0 for BOTH forms.
The single predicate edit n==1 gives Hit(1),Hit(0), hence both p1 values=1.
The reception of case 2 sees n==1 even though case 1 already finished.

Desk: at 0 seen=false,flag=false; first writes seen=true at 1, then case 1
ends without an open route. At 5 case 2 selects second; at 6 flag becomes
true. The same population Never,Hit(1) and p1 rows follow, not Hit(6),Hit(1).
B6 property rows must be case1 probability0/hits0 and case2 probability1/hits1;
expected count=1, no unknowns. Quantile declarations must not change any old report statistics,
diagnostic spans, root verdict or selected-path draw audit keys relative to
the same-length masked source. This invariance is within each interpreter;
it does not legitimize a disagreement between old B6 rows across interpreters.

The selected-successor pair is one arrival at0, one task, no nondeterminism.
The positive control has duration fixed4; its single-edit mate has distribution
2:1/2,4:1/2. The key seed2/rep1/case1/site duration.P.job/n1 has a SHA256 first
u64 in the upper half, so the documented categorical rule selects survival at2
and completion at4. Both sampled forms have point4,N1,C0; observing the
unselected early-completion successor instead gives2 only in the distribution
member. Exhaustive p1/2=2 and p1=4 exact independently witness that the early
successor exists; this is not a JSON world carrying an invented selected flag.

## R2 wire

The hand scaffold has reached_only point4,N8,C0. Changing ONLY C to8 implies
H=0, so a finite point cannot be established. Eight known misses permit
zero_denominator, but changing C from0 to1 means a positive denominator is
still possible, and that wire must be rejected too. Neither prohibition bans
T07: Hit2,Hit2,Censor(>=4),Censor(>=4), p=1/2 identifies point2,N4,C2.

## m2 / m3

No completed replications means no_samples, never checked quantiles. Test the
real engine and reference empty reducers through comparison_projection's
inventory check, rather than synthesizing a wire that merely looks empty.
Every existing public-IR mutant is supplied after lower to both simulate
entrances, with the same rejection as explore. Valid and huge valid rational
controls must still be Complete without U-SEM-0.

## n1

Cut the Ledger model at clock2 instead of5. Case1 is a closed miss; case2 is
received but has not completed the add. Its maximum possible reached mass
is one, so D_hi/n=1/2 < p1 even with that censor optimistically reaching.
All must be null/beyond_reached_mass/LBnull, not truncated. Simulation N2,C1.
Given remains null/truncated: its denominator has not been established.
