# A7 review acceptance: hand calculations before execution

These review inputs do not add any fixture directory or change P01–P08.
Times are minutes, intervals half-open. `S(x)` is the exact Range scalar
min=max=x, reason=null. Numbers below are derived from the input intervals,
not captured from either interpreter. Rulings in the separate design review record
take precedence over the original review suggestions and ADR draft conflicts.

## m1: explicit comparison exclusion

Use T17's declared model and its same-width whitespace mask. Both compare and
diff remain satisfied/exit0, their metric tables equal the masked controls.
Following `obligation_tables` and `contest_runtime_tables`, the new root
`unchecked` token is **`waiting_tables`**: exactly one occurrence in a declared
model, zero in the masked control. It is not a metrics.unchecked token.

## P09 / m2: simultaneous known and unknown causes

Target case: arrival/admitted0, x/A ready1/start3/stop4,
y/B ready1/start3/stop4, business End4. Another case occupies A with
blocker ready0/start0/stop3 and End3. A and B have capacity1, no resources,
no named executors. Both callbacks must see *both cases*, including the
completed blocker, but a blocker is not target-case service.

- [0,1): target has no task yet, so case.unattributed=1.
- [1,3): x waits on full A; y has no blocker. Plan 2.4 classifies this
  mixed known/unknown interval as case.queue.combined=2.
- [3,4): x/y run simultaneously, case.in_service=1 (not2).
- x.actor_capacity=2, x.unattributed=0; y.actor_capacity=0,
  y.unattributed=2. The Task unknown value is not absorbed into combined.
- Total case partition1+2+1=4. Each Task wait=3-1=2.

ADR0048's blanket wording not to move unattributed to another bucket conflicts
with the combined case rule. The checkpoint-approved plan and review ruling
explicitly retain the latter; Task-level unattributed remains observable.

## P10 / m3: contradictory point rejection

Start from the normal wait-person point (both Task.wait=0 and all causes=0).
Change only task0.wait to null/unfinished while keeping the five numeric zero
causes. Each *point* contradicts the null-population rule, even though the
numeric causes happen to be zero. Reject rather than erase the evidence.

Required actual seams: reference Envelope finish with this single point;
reference Envelope merging it with the valid point (both orders); engine merge
with the valid point (both orders). `bad_first` is retained for every seam so
the callback cannot silently omit an input. For `envelope_finish`, a single
point is enough; its order flag does not change the result.

Positive control replaces only wait with numeric0: accepted, causes unchanged.
The callback must detect an actual production rejection, not return the oracle
and not treat an input-adapter panic as a successful rejection. In particular,
reject *before* merging populations can hide the contradictory point.

## P11 / m6: negative predecessor is an invariant error

arrival0/admitted2; Task ready1/start3/stop4, End/T4. ready-admitted=-1,
which is outside the nonnegative metric domain. The private collector must
reject in debug **and release** rather than saturating to0. Changing only
admitted2 to1 gives predecessor0 and must be accepted. This is intentionally
malformed history, not a claim that public scheduling produces it.

## P12 / R3: all executor candidates, not final assignee

T=5, one case, A capacity2/executors[p,q]/no uses (capacity equals the
candidate count, E-CAP-0; A runs no Task, so capacity never binds); target ready0/start4/stop5,
eventual assignee p. B/p occupies [0,4); C/q occupies [2,4).
A itself is free throughout [0,4).

- [0,2): q is free, so no executor shortage; artificial target wait is
  unattributed2. Looking only at eventual p would incorrectly count executor2.
- [2,4): both p and q busy, so executor2.
- Target.wait=4; actor_capacity/resource/combined=0. Queue A integral=4,
  maximum1, time average4/5. Case service from sibling B masks this Task wait
  at case partition level; this test deliberately asserts the Task view.
- Control changes only C's ready/start2 to0, preserving stop4: executor4,
  unattributed0, same queue integral4/maximum1/average4/5.

This is an artificial History, not an assertion about public work-conserving
scheduling. Both independent callbacks must preserve the whole executor list.

## m5 + R2: nontrivial duration draws

One case, one Task, capacity1, no resource/executor/intentional/external wait;
duration2 or4 with equal probabilities. hit=true is written at completion.
Simulation has four replications, seed1, clock/window10. No business cutoff.
Clone the same IR and change only breakdown_waits true to false.

For each first duration hazard, SHA256 of the fixed versioned draw key supplies
k from its first64 bits. Half/half means k<2^63 selects duration2, otherwise
duration4 and a certain finish at the second endpoint. Independently hashed
first bytes for the four fixed keys are hex60, bb, ef, 8b, hence durations
2,4,4,4 and mean14/4=7/2. The test derives d from this fixed key/hash ledger,
not collector output or production hashing code. It asserts that the
nontrivial first hazard actually appears before comparing on/off keys/choices.

Each repetition: lead=d, in_service=d, all other partition entries0, all five
Task causes0, predecessor0, queue integral/max/average0, no cut, hit query true.
Sample service mean is sum(d)/4 in reduced rational form, not a fixed3.
On/off full non-metric Outcomes (including non-serialized fields) and old
metrics must match separately in engine and reference; then compare the two
independent histories. The single-fault calibration changes only opt-in-on
draw keys in each interpreter, one at a time, then restores src.

## R1: production projection defense

1. wait-any: short branch finishes3, long branch drains to5. Regardless of
   stop difference, every any.join_wait is null/join_any. Change just one to
   numeric0 while retaining mode any: validate_metrics must Err.
2. wait-horizon-queue with actual horizon3: case1 service[0,3)=3; case2
   queued[0,3)=3 on A. Both cut_off count1, dwell.until3/bound lower.
   A wait_total3 lower, average3/3=1 lower, max1. Keep actual cut_off counts
   and all other fields, remove both dwells and both A lower flags:
   validate_metrics must Err. A mere horizon declaration is not the witness.

The test-owned `caseset_projection_internal` module supplies existing
crate-private linkage for these two tests. This executes the real production
validator, not the CLI fixture validator, without changing production src.

## Internal linkage

`review_probes` and `run_review` are acceptance data/callback assertions only.
The existing two-Task P01–P08 callbacks cannot consume P09–P12. Variable-length
and cross-case adapters are required. At 4cec795 both
`engine_waits_review_p09_p12` and `reference_waits_review_p09_p12` call
`run_review`; the formerly unlinked statement is obsolete. This source reading
does not itself claim a new execution. Keep P01–P12 expectations unchanged.
P13's new, separately unlinked callback is documented in waits_mutation_calc.md.
