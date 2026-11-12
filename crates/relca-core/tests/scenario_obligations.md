# ADR0042 acceptance seams

Stage C: new semantic probes are authored but intentionally not linked, as
under the design decision. No production source was modified to make tests compile.

## Link after implementation

In `crates/relca-core/tests/scenario.rs`, add:

```rust
#[path = "support/scenario_selection_probes.rs"]
mod scenario_selection_probes;
```

In `crates/relca-core/src/lib.rs`, add under cfg(test):

```rust
#[cfg(test)]
#[path = "../tests/support/scenario_private_replay.rs"]
mod scenario_private_replay;
```

Planned API: `lower::lower_with_selection(&ast, Option<&str>, Option<&str>)`,
`report::check_with_selection(src, configuration, scenario)`,
`report::simulate_with_selection(src, configuration, scenario)`,
`explain::explain_with_scenario(src, scenario)`. Existing signatures stay intact.
The private replay test uses existing replay API and the planned Horizon kind.

The accepted horizon-pending wire extension permits unresolved plus complete
coverage only when bounds.truncated is false, a scenario horizon D is present,
the obligation deadline is strictly after D and its reason is horizon. This is
closed observation, not fulfillment: P04/P24 fix both the per-case inconclusive
verdict and the unresolved ledger row. The matcher calibration removes each of
the four conditions separately and also rejects old reports without a horizon.

## Finite obligations, not claims of completion

| Obligation | Authored witness | Verification at Stage C |
|---|---|---|
| Both exhaustive drivers independently match hand times, work, probabilities, metric counts, then each other | scenario_selection_probes, 77 check inputs | Unlinked, not executed |
| All malformed/unused scenario declarations fail before IR | static probe loop, P09/P19/P28 and P13 | CLI runtime RED; core unlinked |
| Sampled cutoff remains unknown, not a terminal miss | P06 replication audit plus CLI | Core unlinked; CLI runtime RED |
| Random key does not include scenario name | P27 each draw and replication in engine/reference | Unlinked |
| Public IR rejects zero/late D, missing/empty scenario, zero every, mismatched count/until/array | eight independently forged clones | Unlinked |
| Exact10000 expansion boundary is lower-only | scenario_limit_10000.relca | Unlinked; never enumerate10000 cases |
| Numeric/state budget before D remains computationally truncated | zero state-budget clone | Unlinked |
| Honest horizon trace replays; D-1, D+1 and omitted same-time finish fail | scenario_private_replay.rs | Unlinked |
| Fresh mutable lowering tables for each scenario, selected cases before Agency | P09/P19/P28; additional Agency call_order contrast owned by C5 | CLI RED; C5 contrast outside B5 |
| Private Eq/Hash includes horizon/stop reason without changing old states/Room | old byte/census guards plus implementation-side unit obligation | No new private state implementation inspected |

The last obligation cannot be discharged merely by frontend/CLI fixtures. The
internal transitions/keys need separate unit
checks; this file specifies obligations, not a passing test.

## Historical cfg(test) populations to exclude only by closed scn membership

Do not alter numeric expectations. These three broad source-local populations
will encounter scn inputs, unlike prefix-specific duration/state/exec censuses:

* `syntax::tests::every_fixture_obeys_its_expected_syntax_diagnostics`
* `engine::legacy_census::e_legacy_state_identity`
* `engine::simulation::simulation_tests::s_engine_migration_classification_preflight`

Use `tests/support/scenario_population.rs` for a separate closed exclusion.
Other source-local `read_dir` sites were read: duration/state/probability/exec
censuses and lower's metrics population are prefix-limited and do not include
scn names. The closed filter does not change numeric expectations.
