# ADR 0048 survivor acceptance — arithmetic fixed before execution

These eight public inputs are inline in `waits_mutations.rs`; no fixture
directory or closed fixture population is added. P13 is separate private
callback data in `waits_private_oracles.rs`, not a public scheduling claim.
All times are minutes, all intervals half-open. S(x) means min=max=x,
reason=null; R(a,b) means exact exhaustive extrema, not a probability or mean.
Expectations below come from ADR 0048's priority and plan 2.4/3/4, not from
probe output. The mutation proposals are received observations only.

## F15a: lexical branch order

Rename wait-person's first branch a to z and second b to a. The independent
actors run z=[0,3), a=[0,5), End5; service5, no queue. The join inventory must
be [z:S(5-3)=S(2), a:S(5-5)=S(0)] in that order. Satisfied/exit0. Sorting
names is not alpha-renaming: it reverses both positions and their payloads.

## F26: exhaustive completion exactly at the horizon

Two simultaneous arrivals, A capacity1, run5, FIFO; Day horizon10 (control12).
Case1 runs [0,5); case2 queues [0,5), runs [5,10), End10. Both reach End at
or before D, so no actual horizon stop. Dwell absent for both. A queue total5,
maximum1, average5/10=1/2, neither observed quantity has a bound key. Overall
queue5; case services5, case queues0/5, leads5/10. Satisfied/exit0 in exhaustive
check, not the existing simulation-only exact-D test.

## A01a: service before concurrent contest route

Use wait-contest with wait5 replaced by fin=Approver.approve(). Initial
approval=[0,2); fail reaches returned2 without posting or contest. On pass,
Ledger.post=[2,3), commit3; fin=[3,5), End posted5. ADR 0040's commit management
step raises or skips the contest at3, before time advances. Clerk has its own
c1 (Approver has a1), no shared resource and a free capacity1, so if raised
recheck runs [3,4); its pass/fail results have the same duration. There is no
positive-time idle scheduling choice and neither Task blocks the other.
Thus route is wholly inside fin. posted lead5/service5, route0, intentional0,
all queue/external/unattributed0 on every posted leaf. returned lead/service2.
Both roots satisfied/exit0; route responds4 <= commit3+5. The probe's reported
service5/route0 agrees with this hand derivation; its formerly untraced route
start is now justified by commit management and the two disjoint executors.

## A01b: queued contest route before intentional wait

Two arrivals0, FIFO. Initial approvals: case1 [0,2), case2 [2,4). If an
approval fails, that case returns at2/4; otherwise post1=[2,3), post2=[4,5),
and waits are [3,8) / [5,10); business Ends8/10. Ledger has no contention at
these times. Clerk capacity1, recheck4, disjoint from both business actors.
If raised, route1=[3,7). Route2, raised at5, is [5,9) without route1 and
[7,11) with route1 (queue [5,7)). The actual prior case's completed history
still blocks Clerk during [5,7); End does not cancel its route.

For case1 posted: service3, queue0, route0 or4, intentional5 or1; End8.
For case2 posted, enumerate all possibilities, keeping initial approval queue:

| route1 | route2 | queue.actor | route before End | intentional | service | sum |
|---|---|---|---|---|---|---|
| absent | absent | 2 | 0 | 5 | 3 | 10 |
| present | absent | 2 | 0 | 5 | 3 | 10 |
| absent | present | 2 | 4 | 1 | 3 | 10 |
| present | present | 4 | 3 | 0 | 3 | 10 |

Thus case2 posted queue R(2,4), route R(0,4), intentional R(0,5), service3,
external/unattributed/other queue0. Case1 returned service2/queue0;
case2 returned service2/queue2. The range endpoints are correlated and must
not be summed as if they were a single leaf. These extrema agree with the
received probe values, now independently derived. Both routes present gives
response11 > deadline5+5=10 for case2: root violated/exit1 (safety alone is
satisfied, but contest_response is also a root obligation). No uniform
probability is assigned to any branch. Raising neither route is the control.

## A05b: combined is the sum of split positive intervals

Arrivals0,0,1; A capacity1/prep3; B capacity2/finish5 uses Desk1, capacity1.
A runs [0,3),[3,6),[6,9); B runs [0,5),[5,10),[10,15). Case2 queue.combined
is [0,3) (a actor-full, b resource-full); global boundary1 splits it into
lengths1+2=3, not max2. Case3 combined=[1,6), split at3 and5:2+2+1=5, not2.
Case2 service[3,10)=7, lead10; case3 service[6,9) union [10,15), length3+5=8,
and resource-only queue[9,10) of length1, lead14. Other case queue buckets0.
Task waits: a=0,3,5 actor_capacity; b=0,5,9 resource.
A integral8/max2/average8/15; B integral14/max2/average14/15; overall22.
Join waits: (2,0),(4,0),(6,0). Satisfied/exit0.

## A23: exactly enough resource is not a resource shortage

Desk capacity2, A capacity1, prep3 uses Desk1; arrivals0,0. Case1 [0,3)
uses1, leaving1, exactly the demand of case2. Case2 waits [0,3) for A only,
runs [3,6). Task2 actor_capacity3, resource/combined/executor/unattributed0;
case2 service3/queue.actor_capacity3, lead6. A queue3/max1/average1/2;
overall3. Satisfied/exit0. A used+need==capacity test must accept resource
availability (strict greater-than is shortage).

## A10: observation end, not last business End, divides the queue integral

Three arrivals0; A capacity1/run2; horizon3. Case1 [0,2), End2; case2 queue
[0,2), service[2,3), cutoff; case3 queue[0,3), cutoff. A integral2+3=5,
maximum2, average5/3, total/average lower. Overall5, observed_until3; no bound
on maximum. Dwell1 absent; dwell2 until3/service1/queue2, dwell3 until3/queue3,
both lower. Inconclusive/exit2. Last terminal2 must not produce5/2.

## A12b: rejected is not horizon cut_off

Three arrivals0; queue capacity1 on_overflow reject; A run5; horizon3. First
case runs [0,3) and is cut_off, dwell until3/service3 lower, no queue. Cases2/3
are rejected on receipt: no Tasks, no dwell, rejected terminal partition all
null/rejected. They are not future/unfinished/cut_off. A queue total0/max0/
average0 (total/average still lower because observation actually stopped).
Safety-only root inconclusive/exit2: actual horizon leaves work unfinished.

## P13 / F21: interval placement is information, not just a total

Artificial History: arrival/admitted0, End/T7, a ready/start0 stop4,
b ready/start6 stop7. Wait interval [3,5) has raw total2, overlaps service on
[3,4). Partition [0,4) service4, [4,5) intentional1, [5,6) unattributed1,
[6,7) service1. Therefore service5/intentional1/unattributed1, other buckets0.
Control moves only wait begin3 to4: raw total1, same exclusive partition.
Total-only F21 consumes its remaining wait budget in [5,6), incorrectly
intentional2/unattributed0 in the first history but not the control. Both
independent adapters must pass actual wait intervals AND their raw clipped sum
to their collectors, then select fields only. Callback data alone is not
executed coverage. Keep P01–P12 unchanged and hand off src linkage.

## Documentation discrepancies encountered

### Acceptance corrections authorized before rerunning

The initial baseline had six passing public tests and two authoring failures.
A01b's clock was extended from20 to40 but the
source builder had retained window20; lower.rs requires window>=bounds.clock,
so set window40 (input validity, not a runtime semantic change). Its route
ledger above, including root violated/exit1, is unchanged. Explicitly lower
this corrected input before invoking the report/interpreters.

A05b's first calculation incorrectly treated [6,9) and [10,15) as continuous.
The actual hand-listed intervals have a one-minute gap, yielding service8 and
resource queue1, not service9/resource0. This correction is derived from those
two endpoints, not copied from the observed output. The survivor-targeted
combined3/5 expectations are unchanged and remain exact assertions. The first
failed log is retained; it is not evidence of a production bug.

The first CLI run also rejected A01a merely for nonempty stderr. A closed warning
inventory is required rather than deleting the stderr check.
For A01a/A01b, exactly one W-RESP-0 warning is required, for Ledger.
W-RESP-0 applies to an Actor without responsibility. The candidate rule
version `agency-p0-1` requires at least one actual argument for J. Here
Approver.approve has no arguments, so is not a judgement even though its result
controls the branch. Ledger.post is the reachable effect, with no responsible
person declared. Standing/contest supplies responsibility metadata and people
are present, activating the warning on this baseline. The extra fin call is
not a judgement either. Auxiliary witnesses are agency.rs's candidates filter
and responsibility.rs's derive, not the source of the expected count.

The first warning-inventory attempt incorrectly assumed two warnings by treating
the argument-free Approver as J. The corrected count follows the rule above. ADR 0043
**文脈** documents this pre-C5 candidate/warning restriction; its later decisions
change surrounding Agency behavior. Recheck both counts after C5 integration
rather than treating this baseline count as a cross-version guarantee.
Standing has a route in Cut and reversible authority, so no W-CONTEST warning;
all other inputs have no people/responsibility requirement and no diagnostics.
Assert the exact code multiset and warning severity in JSON and the same code
multiset in stderr, with no extra non-diagnostic stderr lines.

- Plan F15's T10 has no fork; existing sorted branch names cannot exercise F15a.
- Plan F26's T13 exact-D is simulation-only; add an exhaustive input here.
- Plan F36/6.1 assumes B6 quality rows not present at 4cec795: defer calibration
  until integration, without inventing supported quality syntax on this base.
- ADR draft says Stage B; assigned task and checkpoint plan are Stage C.
- README's core table still says only Verdict, and design v0.2 says no
  implementation; existing interpreter/collector code, not those stale status
statements, governs these tests.
- waits_obligations.md / waits_review_calc.md still describe P09–P12 as unlinked,
  but both real source test adapters call run_review at 4cec795. Those linkage
  statements are corrected without changing old oracle values.

## Observed calibration on 4cec795 plus these tests

These are individual observations, not exhaustive mutation coverage. After the
authorized input/arithmetic/diagnostic authoring corrections above, core target
`waits_mutations` passed8/8 once; CLI target passed72/72 once (including eight
shared core cases, one real-binary loop over eight inputs and63 matcher tests).
P13 inventory passed once but its collector is unlinked and uncalibrated.

For each row below, the corresponding specs.json old/new edit was temporarily
applied at its current source location, the named test was run alone with
`cargo test -p relca-core --test waits_mutations <name> -- --exact`, and sources
were restored. Every executed fault compiled and exited101 due to the semantic
assertion shown, not syntax or compiler failure. All eight post-restore
`git diff --exit-code -- crates/relca-core/src crates/relca-cli/src` calls exited0.
All cargo runs were offline with a temporary directory inside the source tree.

| Fault | Test suffix after `survivor_` | GREEN | RED witness |
|---|---|---|---|
| F15a | f15a_lexical_branch_order | yes | [a:0,z:2] != [z:2,a:0] |
| F26 | f26_exhaustive_exact_horizon | yes | unexpected lower on total5/average1/2 |
| A01a | a01a_service_precedes_route | yes | posted service[4,5],route[0,1] != service5,route0 |
| A01b | a01b_route_queue_precedes_wait | yes | reference case2 intentional min1,queue max2 !=0,4 |
| A05b | a05b_combined_sums_split_intervals | yes | case2 combined2 !=3 |
| A23 | a23_exactly_enough_resource | yes | Task2 combined3/actor0 != combined0/actor3 |
| A10 | a10_observation_end_not_last_terminal | yes | reference average5/2 !=5/3 |
| A12b | a12b_rejected_never_gets_dwell | yes | reference rejected case has forbidden dwell |
| F21/P13 | run_mutation (two private histories) | unlinked | not injected; implementation-side linkage required |

Execution logs and exit records were retained separately for the baseline,
inventory, injected faults and restorations. Empty restoration logs recorded
successful source diff checks. These are execution observations, not hand expectations.
Tests never read those logs. F36 remains a B6 integration obligation
in deferred.md, not an extra calibrated survivor here.
