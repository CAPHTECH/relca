# F01 — repeated addition

Hand oracle: 0 + 1 = 1, then 1 + 1 = 2. Two serial one-minute
tasks give elapsed = work = 2 min. The done guard is true; 0 ≤ 2 ≤ 3.
Replacing the effect by assignment gives 0 → 1 → 1, only wrong,
reachable done violated but state_in_range satisfied. No probability supplied.

## Supplemental hand matrix (bounded_integer_boundaries)

All comparisons are integer comparisons. Equality at2 is true, inequality at2
false; 1<2 true but 2<2 false; 2<=2 true but 3<=2 false; 2>1 true but
1>1 false; 2>=2 true but 1>=2 false. These same truth values are asserted
independently at if, requires, invariant finish and Authority scope consumers.
For n10, n<2 false and n>2 true (not lexical order), n<0 false, n<=13 true
even though upper12. Grouping (n>=2 AND n<=2) OR (n!=2 AND n<0) at n2 is true.

Unsigned domain cardinality is N+1, calculated exactly: u64::MAX domain has
18446744073709551616 values, but a fixed initial value costs no such enumeration.
MAX+1 attempts 18446744073709551616, MAX+MAX attempts 36893488147419103230;
neither wraps. 0-(-1)=1 legal, 0+(-1)=-1 violates. Singleton [0,0] accepts
assignment0 but rejects addition1. Huge nondeterministic domains must stop at
the computation budget before allocation; no received event for an uninitialized
case. Internal tuple/Hash/Room obligations require separate tests.

Distribution support {0:1,1:0} executes only0 and hit(h1)=0. Duplicate numeric
spellings 0/00 or 0/-0 do not cover1. Missing1 and mass99/100 are E-PROB-0;
value2 outside [0,1] is E-TYPE-0. Policy cannot read latent even in false AND,
true OR, dead suffix, unselected process or scope; analyst query/invariant can.

Mixed mass: enum chooser selects safe1/2, range1/4, clock1/4. Safe reaches
done; other continuations unknown, so P(done)=[1/2,1], never renormalized2/3.
seen=true was written before all three branches: P(seen)=[1,1]. Range and clock
flags both true, S violated. Replacing overflow +=1 by +=0 removes range stop,
but clock still leaves S inconclusive. No unreachable warnings on incomplete
exploration. Simulation four deterministic repetitions yields safe S inconclusive
(sample), or overflow S violated (witness), stopped count4, cut/deadlock0,
terminal0 for overflowing repetitions. No integer fields on old enum models.
