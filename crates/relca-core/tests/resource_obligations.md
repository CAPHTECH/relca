# ADR 0033: finite implementation-side obligations

Stage C acceptance is deliberately RED. This is an obligation ledger, not a
claim that private states, malformed IR, or tampered replays were exercised.
Production src is owned by the separate implementation instance. Private types
cannot be reached from integration tests; inventing public APIs or a release DSL
to pretend otherwise would weaken the test boundary.

## Public evidence and hand arithmetic

`resources.rs` checks the 30 exact edits, old-language lower preflight, all60
reports, positive-time Complete50 / static9 / zero1 census, whole Outcome equality
after removing lower diagnostics from reference only, arrivals crossings,
distribution weights, naming, unused resources, simulation and deadlock explain.
`resource_crossings.rs` checks refusal/cancel counters, post-transition cancel
holders, global holders, exact static byte spans, range/budget cuts, unselected
uses, forward names and order invariance. `relca-cli/tests/resources.rs` invokes
the actual check/explain/simulate/compare/diff surfaces and calibrates the closed
wire table on synthetic resource rows derived from fixed arithmetic, not a new
implementation. Its compare test is not a forged projection test.

Additional arithmetic (not read from implementation):

* P03 discrete x3 with mass1 is O8/P3/W5/E3. x1 or3 at mass1/2,
  y2 and quantities2/1 gives O4..8/P3/W3..5/E2..3. No duration lookahead.
* P06 b normal finish3 and ack3 both permit c[3,4), E4/O7. Both transition
  orders must be retained privately; matching extrema alone cannot prove this.
* Cancel explain uses an intentionally false invariant at c.finish to obtain a
  full prefix. b request2 retains1; ack3 (cooperative) or2 (immediate) retains0;
  c starts at that time holding2 and finishes one minute later holding0.
* Global explain R2 has x(case1)/x(case2) holding1 each: occupied2, holders
  sorted case1 then case2. This is separate from Actor resource holders.
* Contest model is new, not copied from ctx-contest-resource. Judge work2 uses
  R1; two cases arrive0, Judge has two people, Route has a third person and
  duration1 usesR1. x1[0,2); at2 x2 competes with optional route1. Business
  case1 lead2/work2 freezes, case2 lead4..5/work2. Global O4..6/P1/U2/5..3/5
  includes zero, one, or two route invocations. Increasing only route duration
  to2 changes case2 lead4..6 and O4..8/P1/U2/5..4/5, never business work2.
* Range stop before a start effect completes withdraws every resource scalar
  with range_stopped; a state-budget cut uses truncated. Neither is deadlock.
* The serial half-open H16 control is O=2*2+1*1=5, P2 not3, U5/(3*10)=1/6.
  The internal due/start mixed snapshot below is still required independently.

P11 is a single selector edit B.y→A.y while declarations stay fixed (A.y and
B.y both exist). Exactly one unused Capability warning occurs on each side. The
model deliberately avoids a multi-location Actor declaration rewrite. Hand
paths: x1[0,2),y1[2,5),x2[5,7),y2[7,10) and
x1[0,2),x2[2,4),y1[4,7),y2[7,10). Same-Actor variant retains only the latter.
Full ordered terminal traces, not internal management-step counts, are required
in the implementation-private reference harness.

## Required private controls and single-fault pairs

All listed tests remain [U] without evidence of their actual execution.
Use independent engine legacy/arrivals and reference legacy/arrivals paths; a
shared helper or a public happy-path safety result is not evidence for all four.

| ID | Exact control and fault that must fail |
|---|---|
| H01 | Seed a/b Running quantity1 each, Actor capacity2, no people, R2: safety satisfied. Change only R capacity1: safety violated, including clock/range cut flags. No new diagnostic. Four observers separately. |
| H02 | rsrc-release-fork keeps a Done sibling while the other waits. Include Done in occupancy: deadlock at2 or3, no end, O4..6; no never_started warning because each call starts in another path. Public E5/O10 must fail. |
| H03 | P06 request2: a Done, b Running ack3; occupied1. Excluding ack=Some makes0 and must fail the direct snapshot. Immediate request2 still holds1 before distinct ack2. |
| H04 | Drive immediate ack2 before cleanup: occupied0; quantity2 queued probe startable. Keeping cancelled b as holder yields1 and probe false. Final CLI alone may miss this defect. |
| H05 | S1 held by another Running call, R1 free; x needs R1/S1, y needs R1. x cannot start, R occupied0, y can start. Partial R reservation while x Queued must fail. Reverse uses order too. |
| H06 | rsrc-queued-cancel b queued with R0: occupancy0, starts0, cancelled_before_start1. Counting queued b as holder violates resource-only safety. |
| H07 | Lower P14 then independently mutate ResourceId out of bounds, amount0, duplicate use, empty name, duplicate name, Actor/resource collision. Engine explore/simulate/arrivals: U-SEM-0/unsupported; reference: SemanticAbort/U-SEM-0. Guard the replay entry too. Never fallback to ID0 or panic. |
| H08 | Legal MAX and1 wait without overflow: E2, O18446744073709551616, P18446744073709551615, U9223372036854775808/92233720368547758075. Private simultaneously Running MAX+1 must SemanticAbort/U-SEM-0 independently; wrapping/saturating is not deadlock or occupied0. |
| H09 | Zero-time x start holds1, finish releases0 at0; public fork E1/W1/O1/P1. Reference zero-time exclusion stays explicit. Done-holder fault must fail. |
| H10 | FIFO global seed same Actor: case1 queued uses2, case2 queued uses1, R1. Both start candidates absent. Excluding unstartable head incorrectly permits case2. Move only case2 Actor for positive other-Actor start control. |
| H11 | P14 shortage required2/available1/missing1, R1/occupied0/holders[]. Change missing1→0 and capacity1→2 separately: replay StateMismatch/IllegalTransition. From global R2 simultaneous holders change nonempty case_id or amount separately, reject independently in arrivals World. |
| H12 | Lower two-configuration P12 compare R1/O4/P1/U2/5. Forge one report row: remove/duplicate/rename/change capacity/empty resources/unknown field/change Scalar mode. Each projection rejects U-SEM-0/unsupported. The public CLI compare control is not evidence that forged rows are rejected. |
| H13 | FIFO simulation2 seed1 succeeds O4/P1/U2/5 as whole-replication samples. Remove FIFO: U-SEM-0, zero completed replications, no fabricated stats. Actor-crossing P11 refuses even with Actor FIFO. |
| H14 | Four reserved identifier Actor/variable resource/uses old stdout/stderr/exit hashes are in resource_compatibility.json; no projection. |
| H15 | Public tests cover duplicate declaration, resource rate, resource Cut member, human header, per_case/calendar/partial syntax and person named R. Resources never enter Actor name lookup. |
| H16 | Direct metric seed R3: x qty2/start0/finish2 is due at2, y qty1/start2/finish3 is newly started, prior peak2. Peak remains2, not3. Both engine due exclusion and reference independent interval sweep required. |

### Concrete internal entry targets

These are existing symbols or the new public IR types named by the approved
plan, not invented callable resource-helper APIs. The input, hand expectation
and killing fault for each ID are the corresponding row immediately above.

| ID | Function / type to drive directly |
|---|---|
| H01 | engine.rs `State`, `Lifecycle`, `Exploration::observe`, `run_initialized`; engine/arrivals.rs `Global` and its evidence `observe`; reference.rs `Frame`, `Phase`, `Interpreter::observe_capacity`; reference/arrivals.rs `World` and `observe`. Drive each observer separately. |
| H02 | engine.rs `step` / `State.tasks`, reference.rs `Frame.tasks` / `Phase`; retain Done sibling in fork while probing queued next call. |
| H03 | engine.rs `step`, `Lifecycle::Running`; reference.rs `Phase::Running`; drive actual CancelRequest then measure derived occupancy before ack, in legacy and global variants. |
| H04 | Same `step` / lifecycle types but drive actual immediate CancelAck, stop before task-array cleanup, then drive queued quantity2 probe through the next `step` / reference start-candidate gate. |
| H05 | engine.rs `step` with `State` seed and reference.rs `Interpreter::can_start` with `Frame` seed; global variants use `Global` / `World`. Do not substitute final elapsed for the candidate-set assertion. |
| H06 | `Lifecycle::Queued` / reference `Phase` plus actual cancellation transition and corresponding four overcommit observers. |
| H07 | Approved new `ir::ResourceId`, `ir::Resource`, `ir::ResourceUse`, `Model.resources`, `Capability.uses`; engine.rs `validate`, public `engine::explore` / `simulate`, reference.rs `validate` / public explore / simulate, and reference/replay.rs `replay`. Existing guards must independently reject each mutated IR. |
| H08 | `State` / `Global` derived occupancy, reference `Frame` / `World`, each observer/validation abort conversion; independently test actual Running MAX+1, not just the queued candidate MAX+1. |
| H09 | engine.rs `step` around zero-time Running→Done, plus `engine/metrics::Peak::observe`. |
| H10 | engine/arrivals.rs `Global::fifo_head` and start-candidate expansion; reference/arrivals.rs `World` start-candidate expansion. |
| H11 | engine/trace.rs `task_stage`, engine/arrivals.rs `global_context`, explain.rs `Stage` / `DeadlockDetail`; reference/replay.rs `replay` / `replay_choices` and reference/arrivals_replay.rs `replay`. Mutate generated nonempty holder records before independent replay. |
| H12 | comparison_projection.rs `validate_metrics` and `exhaustive` / `simulation`; change the report-side resource table while retaining the original `ir::Model`. |
| H13 | Public engine/reference `simulate`, also exercised by CLI acceptance. |
| H14 | `syntax::parse` and `report::check`, plus the four committed old-binary byte probes. |
| H15 | `syntax::parse`, `lower::lower`, public report gate. |
| H16 | engine/metrics.rs `Peak::observe` / `TaskHistory`; reference/metrics.rs `project` independent interval sweep. |

For K18 add repeated use of the *same Capability* in two fork occurrences: demand
sum counts both, not the set of Capability IDs. Overflow of proof demand means
proof unavailable, not runtime SemanticAbort. Public huge occupancy and actual
overflow must remain different obligations.

## Src test migration and closed old populations

For the closed resource population, migrate
`syntax::tests::unsupported_keywords_report_u_syn_even_before_lexically_invalid_payloads`:
model item `resource slots capacity 1` → `members [p]`; Capability item
`uses slots: 1` → `retry 2` (or another distinct unsupported item). Model-level
uses, Actor-level uses and reserved-name tests remain unsupported. Do not change
the numerical census values. Existing integration tests had no matching newly
accepted resource/uses rejection inputs (content search).

Inspect the broad readers at syntax.rs parse_all_fixtures,
lower.rs, engine.rs all_dirs/legacy census and engine/simulation.rs. Append a
closed resource name table, route runtime cases to the new acceptance, and keep
all old fixed counts/census. Do not count the new population as old or lower old
counts to avoid failures. Four older compatibility integration readers exclude
rsrc-* while preserving all old manifest rows and numerical expectations.

Observed src population failures (do not change their numerical oracle):

* `engine::legacy_census::e_legacy_state_identity`: 184 directories counted,
  expected old124. Add the explicit new60 resource classification, not 184.
* `engine::simulation::simulation_tests::s_engine_migration_classification_preflight`:
  (16,208), expected (16,148), current static tuple [80,80,79] includes the
  extra60 parse refusals. Preserve old [80,80,19] after separation.
* `syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics`: attempts
  to parse new runtime rsrc-* and currently gets resource U-SYN-0. Separate the
  closed new population without relaxing old syntax or changing old counts.

Parse failures cannot establish even a lexically later explicit Cut:
report.rs check_with returns on parse Err before setting Cut unresolved.
Consequently P16/P17 and syntax-limit probes retain the existing W-CUT-0 after
the primary syntax diagnostic. This is a correction to the planning table, not
a new resource warning or an exception to old byte compatibility. Lower errors
after a successful parse do know that Cut was declared and have no such warning.

## Review obligations (finite, not merge approval)

O1: all 25 P pairs materialize, exact nonempty edit, no DSL comments.
O2: old-language controls lower before new syntax is required.
O3: engine/reference each meet hand metrics and time before equality.
O4: reserved names and old602 raw jobs (rebuilt at 1fcaf03 after the ADR 0031 integration; first 564 at 6f790e6) have zero allowed difference.
O5: closed metric/explain table rejects unknown/missing/mode faults.
O6: private safety/atomicity/replay/projection obligations are handed off, not
marked completed by visiting this document. K01–K36 remain uncalibrated at RED.
