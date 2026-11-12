# D06: closed misses transported into declared compare

Hand calculations fixed before running the new acceptance. Authority: ADR 0049
R1/M1 and the c7 review-1 / c7 review-2 / c7fix rulings in r7-adr-rulings.
The inputs are derived from `support/quantile_review_inputs.rs` CLOSED and DESK;
the existing closed fixture populations and their expectations are unchanged.

## Inputs and single-edit positive controls

- Ledger: arrivals 0,2 min, add duration 1 min. n is 0 until1, 1 until3,
  then2. For property Two=(n==2), case1 closes at1 with a miss; case2
  hits at3 (elapsed1). The later add cannot reopen case1. The predicate-only
  control n==1 instead gives Hit(1),Hit(0), hence two reached cases.
- Desk: arrivals 0,5 min. first writes seen at1 and case1 closes; second
  writes flag at6. Flag therefore has the same miss/hit pair. Replacing only
  the property's Desk.flag with Desk.seen gives Hit(1),Hit(0).

For each source duplicate P's body as Q and add Left/Right configurations of
P/Q under C (distinct names keep simulation operation keys separate), define
cohort.Rate=expected(count(case.Two or case.Flag))/arrivals, and declare
hard Rate<=0.75 plus objective minimize Rate. Increase the existing simulation
clause to four replications (seed1, confidence1/2). Run both modes, both plan
orders, with original quantile declarations and with just those declarations
removed. Execution and the cohort property do not depend on quantile presence.

## Exact and statistical arithmetic

Both configurations have two arrivals and no unknown quality observations.
Exhaustive verify is satisfied. In the miss input, expected reached count=1, denominator=2,
Rate=1/2 exact. Exhaustive hard [1/2,1/2]<=3/4 is satisfied; objective is
[1/2,1/2] exact. Both plans are admissible; compare is satisfied/exit0.
In the positive control count=2, Rate=1, hard is violated and plans excluded;
compare is violated/exit1, objective remains1 (objectives do not alter hard).

Simulation: every replication has the same deterministic path. Miss input
hits=[0,4], unknown=[0,0], trials=[4,4]; reached count mean/min/max=1,
Rate point=1/2. Positive control hits=[4,4], count2, Rate point1.
One hard row x two cases x two plans gives Bonferroni m=4. The one-tail
alpha=(1-confidence)/(2m)=(1/2)/8=1/16. At four trials the endpoint coin1/2
has tail (1/2)^4=1/16 exactly, already on the outward dyadic grid. Thus
CP(0/4)=[0,1/2], CP(4/4)=[1/2,1]. Summing case endpoints then dividing
by the fixed arrival denominator2 gives:

| Input | hard interval | point/objective | hard verdict | plan / root |
|---|---|---|---|---|
| closed miss | [1/4,3/4] | 1/2 | satisfied (inclusive upper equality) | undetermined / inconclusive, exit2 |
| positive control | [1/2,1] | 1 | inconclusive | undetermined / inconclusive, exit2 |

Simulation support is not an exhaustive proof. Its objective uses the cohort
point, not the CP hard endpoints. Exhaustive objectives retain the exact value.
The existing last_write_wins declarations imply no_lost_update safety checks;
Ledger also has an implicit integer range check. These stay inconclusive under
simulation, so verify, overall plan status and root are inconclusive even when
hard is satisfied. They must not be erased just to obtain exit0.

## Fault sensitivity fixed in advance

Temporarily bypass ONLY the quality_closed early continue in engine arrivals
simulation. Its execution, RNG, exhaustive reducer and reference are unchanged.
Case1 is then falsely hit by the later writer: hits=[4,4], count2, Rate point1.
For the miss inputs the hard interval becomes [1/2,1] and hard changes from
satisfied to inconclusive (root stays inconclusive because of the independent
sample safety gate). Check hard cells BEFORE nested
quality/objective cells so a failing calibration witnesses the hard transport,
not merely a lower B6 or quantile assertion. The true-positive controls and all
exhaustive cases should remain green. Restore the single guard immediately,
require an empty production src diff, then rerun the two new targets.
