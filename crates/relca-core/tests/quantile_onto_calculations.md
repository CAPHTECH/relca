# C7 onto A7/B7: hand expectations before executing the new acceptance

Sources are the already-authored `support/quantile_deferred.json`, not product
output. Inline crossings do not increase the closed qtl inventory of 67.

## c7fix2 review controls (2026-10-31, before running them)

R1 moves only the four contiguous quality rows before simulation.random. The
six old simulation/estimation rows retain all values and their relative order.
This is an invalid order, with or without quantile declarations. R2 replaces
only structural refusal quality with the independently wire-valid normal
simulation quality: root attempted/completed remain zero, so this must fail
the root/quality withdrawal relation, not the quality wire schema. Before
quality projection every quantity must be withdrawn: reason aborted, zero
hits/unknown/trials/samples/observations/censored, null point/mean/min/max/LB,
and quantiles unchecked. No production change is required for these controls.

### R3/m2: real comparison table

All new inline comparisons use distinct P/Q process names, identical bodies,
One/Two in declaration order and a common Cut C. No random numeric oracle is
inferred from process names. Expectations below precede their first execution.

| source | arithmetic and nested value | evaluation/admission |
|---|---|---|
| sample_censor, simulation | FIFO capacity1, fixed2, arrivals[0,0], clock3, R1: Hit2 and Censor earliest4. N2/C1, F(2)=1/2. Median point2; P95 null/truncated/LB4 | objective Median point2 and P95 pointnull/truncated; no LB field; reachable case2 inconclusive, both undetermined; pareto not_computed/simulation_estimates, not_evaluated both |
| zero, both modes | observed hit=false forever, work2 then done; Done=reached(hit==true). D=0. Given median null/zero_denominator/LBnull. Sample N1/C0 | reachable done satisfied, both admissible; exhaustive computed with One->Two missing_value; simulation not_computed/simulation_estimates |
| maybe, exhaustive | latent case hit:bool has two nonprobabilistic initial values; work2 and done in both. Dlo0,Dhi1, given median null/denominator_may_be_zero/LBnull | reachable done satisfied, both admissible, missing_value (never not_evaluated) |
| horizon, both modes | fixed2, arrival0, horizon1, clock10. no hit, earliest2, Dlo0. All median null/undetermined/LB2; given median null/undetermined/LBnull; sample N1/C1 | reachable done inconclusive; both undetermined; exhaustive not_computed/no_admissible_plans, simulation not_computed/simulation_estimates; not_evaluated both, never missing_value pair |
| reached positive control, both modes | same initial hit=false->true single edit: Hit0, D=1. Given median0; sample N1/C0 | both admissible; exhaustive tie, no missing pair; simulation dominance not computed |
| Rule quantile objective, both modes | reuse rule_source, change only cohort.RequiredRate objective to quantile.All. Rule means ALL quality withdrawn: two quantile rows aborted/LBnull/unchecked, quality.rule present | objective null/aborted; cohort hard unsupported; both undetermined and not_evaluated, no fabricated numeric value |

The objective-free version of each source keeps the same verify and workload;
root verdict and each nested report must be identical. This isolates objective
transport from computation, rather than teaching a new root classification.
Only source SHA256 is masked in that equality, because the source text changes.
Authoring correction after the first run: objectives {} is invalid syntax;
the control now blanks the entire optional objectives clause, retaining width.
An effective horizon must be in a scenario (Day, explicitly under [Day]), not
at model scope. The first draft's resulting refusals are input-construction
errors, not production RED or changes to the numeric expectations above.
The Day input is passed explicitly to compare_with_scenario / CLI --scenario
Day; under is an allowed-scenario list, not an implicit selector.

### m1: closed shared-observation table

Shared source substrate: fixed2, arrival0, no censor, both configurations finish
at2. Done=reached(end.done), Other=reached(hit==true) with initial hit=true.
Q(.5,Done)=2, Q95(.95,Done)=2, OtherQ(.5,Other)=0. Numerator cohort
DoneRate=E[Done]/arrivals=1; denominator cohort Inverse=E[Other]/E[Done]=1;
OtherRate=E[Other]/arrivals=1. All plans admissible, all purposes tied. In both
modes compare.pareto.affects must name exactly the declared ordered pair for
(Q,Q95), (Q,DoneRate), (Q,Inverse), and the reverse (Inverse,Q). This includes
the denominator-only intersection and reverse dispatch. Controls (Q,OtherQ),
(Q,OtherRate), (Q,overall.work) do not match the ruled closed table and keep
the full existing none sentence verbatim. Direction maximize is used for the
second objective in these inputs to test labels without using a weighted sum.

### m3: fae19c62 private evaluation oracle

evaluation::quantile_objective_tests::source starts with eval-contention and
adds Q(.5,OnTime), where OnTime is done within4. One: FIFO A capacity1/fixed3,
arrivals[0,0], done times3/6: first hits are3 and infinity (second misses within4).
F(2)=0, F(3)=1/2 and p=1/2, so Q is exactly3, not midpoint or average.
Simulation R1 has the same deterministic Hit3/Never pair, N2/C0, giving point3.
Two: B capacity2/fixed4, both finish4, both meet the inclusive deadline4;
F(3)=0,F(4)=1, so Q=4 in both modes. The private exact3 and point3 expectations
are therefore arrival-relative first-hit arithmetic, not copied producer output.

## A7 / F35

FIFO A.run duration 3, arrivals [0,0], capacity 1: ends 3 and 6 minutes,
arrival-relative first hits 3 and 6, F(3)=1/2, median 3. Queue wait 0 and 3;
service 3 each. One simulation replication has N=2, C=0 and median 3.
Remove FIFO: scheduling is unresolved and simulation refuses, not uniform.
For a genuinely late abort retain FIFO and add a second result to the Task:
dispatch is deterministic, the unresolved result is encountered at time 3.
The whole quantile result is aborted, not a finite value from an unfinished run.

All OLD assumption rows (including quality.scheduler / quality.estimation)
precede metrics.waiting, which precedes quality.quantile, then
probability.scheduler if present. Removing only the two new disclosure rows
must recover the ordered old disclosure, compared to the same-length mask.
The missing-simulation-clause and structural-completion refusal have no
metrics, thus no metrics.waiting, but preserve four old quality disclosure rows
and the quantile aborted disclosure. They are not exceptions to the A7 ruling.
The non-vacuous probability.scheduler control removes FIFO and adds independent
case coin initialization (false/true each1/2) and probability coin==true <=1.
Chance and unresolved scheduling coexist. The coin never changes work, so each
case can finish at3 or6 and the per-case-relaxed median is[3,6]. The actual
four-row order must end with probability.scheduler; a vacuous optional-row
check on the deterministic FIFO model would not cover this crossing.

With waiting and quantile declarations, metrics.unchecked remains exactly:
quality, business_value, queue_length_quantiles, quantiles, exact_expectations,
metric_constraints, metric_intervals, anonymous_human_work, calendar,
rework_loss, introduction_cost, terminal_meaning. Quantile computation does not
claim that BusinessMetrics computes quantiles. Old quality fields remain equal
to the same-length quantile-masked control. Both collectors are checked against
the hand values before their mutual comparison.

## B7 objectives (actual existing ObjectiveCell Exhaustive envelope)

Cells are configuration/lower/upper/basis/reason, never a new parallel schema.
The quantile lower_bound is diagnostic information, NOT a finite cell value.
The existing B7 row label includes the direction, e.g. objective is
minimize metrics.overall.work as well as direction=minimize. Metric Scalars use
basis=path_range even if min=max; quantile values retain their own exact or
per_case_relaxation basis. These are the actual B7 wire conventions, not an
alternate hypothetical envelope. Raw per-replication waiting metrics are Range
Scalars (min/max), whereas report-level simulation metrics summarize Samples.

| input | values / constraints | admissible frontier / undetermined pair |
|---|---|---|
| T08 FIFO | One median 3, Two 4; hard ratios 1/2,1 >=1/2 | One; none |
| T08 no FIFO | One median [3,6] per_case_relaxation, hard [0,1]; Two 4, hard1 | only Two evaluated; One undetermined |
| T09 normal | lead 8/5, work 8 each, cost 8*100+2+3=805 each | ToBeP; none |
| T09 fastlong fee 300 | only parallel fee changes, cost 800+2+300=1102 | AsIs,ToBeP; none (tradeoff) |
| T10 normal | lead 8/5, cost805 each | ToBeP; none |
| T10 missing long fee | both costs null/not_configured, never0 | both; AsIs->ToBeP missing_value |
| T21 Maybe | possible miss: null/beyond_reached_mass, LB2; Sure4 | both; Maybe->Sure missing_value |
| T21 pass only | Maybe2 exact, Sure4 | Maybe; none |
| T21 interval | Maybe[3,6], Sure4 | both; Maybe->Sure overlapping_intervals |
| T21 FIFO interval | Maybe3 exact, Sure4 | Maybe; none |

All objectives minimize. The nested quantile rows are separately checked against
the authored hand oracle; objective transport must equal their value, not LB.
T12 quantile hard is still unsupported U-SYN-0 with the exact message
`hard constraints apply to cohort ratios only` and exit4, not a quantile hard.

Rule comparison uses the same Before/After configuration substrate as the
existing C7 intake. Both quantile rows stay aborted, value/LB null, quality.rule
is disclosed. A cohort hard constraint on the same model stays unsupported,
both plans undetermined (B7 D04); no ordinary Rule numeric value is resurrected.
For simulation the Flow body is copied as Flow2, with ToBeB selecting Flow2:
B7 intentionally rejects two plans sharing a process RNG namespace. The body
and the hand Rule withdrawal values do not change. The waiting projection
control likewise uses identical bodies with distinct P/Q process names.

An additional GREEN objective-side calibration control retains the T09 pricier
source and quantile reports, but minimizes the already-supported overall.work
instead of quantile.Lead. Work is 3+5=8 in BOTH plans, cost805 versus1102, so
AsIs certainly dominates ToBeP. This executes the real B7 objective/Pareto path
at the initial 2f6c2afe RED checkpoint. It was not a substitute for T21's LB
acceptance. Correction (2026-10-31): fae19c62 connected quantile objectives;
the parser-blocked description is historical, not the current implementation.

The initial acceptance draft corrected two API-only mistakes (Outcome is not
Serialize and SimulationOutcome exposes per-replication metrics) before semantic
classification. queue is the nested actor_capacity/resource/executor/combined
object, not a scalar. The structural syntax is verify terminating end, not
verify terminating end.done. These are acceptance mistakes, not production RED.

Correction history (2026-10-31, R1/M1 ruling): the original intake paragraph
claimed that the pre-existing simulation 5 rows + metrics.estimation followed
the four quality rows. That premise was wrong: PRE-C7 put those six rows BEFORE
quality. 2f6c2afe accidentally accepted the c13a4d66 regression order; fae19c62
restored the producer order. The review negative control first observed that
the obsolete matcher branch still accepted the wrong order, then removed it.
Missing, inverted, extra AND quality-before-simulation rows must each fail.
On structural or
revocable_by no-metrics refusals the five OLD metric siblings remain in their
exact order; no metrics.waiting is accepted. Only these no-metrics siblings
record old behavior; A7's ADR0048 intake addendum leaves those disclosures as a
Phase 1 limitation. Both the missing/reversed/extra controls and waiting leak
control are authored before executing the matcher change.
The compatibility exception permits only the exact U-SEM-0 message
simulation cannot check structural completion questions with known lowered
bounds and zero sampled population: metrics/waiting absent, properties and
case_outcomes empty, attempt0/completed0. Review R2 adds full quality withdrawal
BEFORE projection (aborted, zero samples/trials/observations, null quantities,
quantiles unchecked); a standalone-valid sampled quality must fail specifically
at the root/quality relation. An unrelated U-SEM-0 message, attempt1,
one valid sampled case row or one waiting row must each be Err. The ordinary
runtime-abort population gate stays unchanged. This shape was observed once via
the real CLI before the matcher extension; it is not a fabricated result form.

## Compatibility composition / C6

The B7 frozen 56 kind-7 job permissions and two CutOff supplements are composed
first using B7's original position tables. C7 identity is then applied with
ZERO allowed positions. Neither all quality nor mutation subtrees are erased.
Old manifests remain byte-identical; latest quantile manifest alone is recaptured
at integrated PRE-C7. The historical ee9e3c6 79-job census is not recaptured.
Existing C6 six-kind scenario/no-scenario, horizon-child and quantile-only
comparison acceptance is rerun unchanged; no new detection meaning is added.
