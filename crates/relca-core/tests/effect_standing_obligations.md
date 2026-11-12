# Effect Standing internal test obligations

Public tests in effect_standing.rs and effect_standing_transport.rs compile on
the PRE-C4 API and intentionally fail on new syntax. The following obligations
need crate-private state/trace access, the newly introduced target API, or a
focused post-implementation calibration probe. They
are visited/planned, **not verified**, and cannot be discharged by root JSON alone.

1. Link isolated public `Commit(CapabilityId(usize::MAX))` mutation after the
   target enum exists, keeping other IR references valid. Engine U-SEM-0 before
   exploration; reference SemanticAbort independently; neither may panic. Also
   forge a mismatched static certificate, duplicate target across Standing rows,
   two lexical calls in a fork/branch and target/route intersection with 0 calls.
2. Private Eq/Hash: same time10, otherwise equal RouteState target_at3 vs5,
   respond_by4, identical monitor bits → states differ, HashSet size2. Merely
   omitting Hash with correct Eq is not a killed fault or proof of aliasing.
3. end_open None/false/true belongs in state identity and differs per case. End
   snapshot is after same-tick terminal filing selection, before dispatch. Two
   contests count one case. Whole-path min/max is not sum of per-case extrema.
4. Room0/1/2 charges two filing successors **before** cloning either. Twelve
   simultaneous contests expand one choice at a time, never build 2^12 first.
5. Route queued_at survives ordinary Task removal/compaction and next Wait;
   queue indexes and BindingIds remain matched in engine and reference. A
   synthetic contest:1 is not ordinary binding contest1. P09/P29 are public seams.
6. Replay poison separately: trigger Capability, target timestamp, case_id,
   filed time, deadline, queue site and executor. Wrong commit/case/duplicate
   filing/early ExecutionClose must be rejected by independent reference, never
   reported verified. P02-short trace may end at advance6 before response finish;
   response result poison needs P27's later invariant violation trace.
7. Preserve all old private generated/visited/held census and budget values.
   New RouteState fields must not leak into frozen compatibility debug strings.
   Do not regenerate census snapshots to conceal a semantic change.
8. Standard joins/Any cancellation/if_none/drain inspect only current normal
   Wait tasks, never route Tasks; next Wait preserves route and its metrics.
9. BusinessEnd exactly once; route result never writes synthetic index into
   ordinary process.results or advances pc/adopted. Trace matches by BindingId,
   not zip after compaction. Sleep/wake/Stop still dispatch executable routes.
10. Zero-weight kernel outcomes cannot make a self decision candidate; add
    self candidate only for atomic or effective distinct positive-mass results.
    Restricted grants reject before Permission/eligibility filtering, including
    a restricted duplicate alongside an unrestricted grant for the same pair.
11. Metrics required-price inventory includes reachable lexical effect routes
    even on a declined path. Route occupation/cost uses normal per-task rules;
    business work/cost excludes route before and after End. No missing rate=0.
12. Integrate P14/27 and explicit-limit pairs only when prerequisites
    exist. Their absent syntax is not a silently excluded reference input.
13. Before claiming an F37 kill, add a focused route-only visibility/Authority
    inventory probe (not necessarily private). Current P25 asserts the public
    contest target/payload and intent warning, but is not a calibrated proof that
    every related route inventory row is preserved. Keep this gap explicit.

## Existing cfg(test) populations requiring append-only exclusion/linkage

Source inspection identified unfiltered new
sov input exposure at these tests (old numeric expectations must stay fixed):

- syntax.rs `tests::every_fixture_obeys_its_expected_syntax_diagnostics`,
  read_dir around3081: append closed sov classifier;
  valid targets parse, unsupported target pairs stay classified refusals.
- engine.rs `legacy_census::e_legacy_state_identity`
  (read_dir around1792): append exact new
  names rather than allowing them into the frozen legacy census.
- engine/simulation.rs `simulation_tests::s_engine_migration_classification_preflight`,
  read_dir around1057: the old legacy-simulation fixture
  population must not ingest new effect nondeterminism or change its counts.

Other read_dir tests were inspected: state-/exec-/prob-/duration- prefix censuses
and lower.rs met- scan do not include sov. The final workspace failure census
is authoritative for additional integration-test populations outside src.
