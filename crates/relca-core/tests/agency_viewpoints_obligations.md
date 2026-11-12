# ADR0034 private and integration obligations

Public integration tests cannot inject cfg(test)-private budgets. Include the
independent driver in support/agency_viewpoints_budget.rs in the new module's
unit tests and adapt its closure to the private limit injector. Identical DSL
controls A16/A17 do NOT claim to execute private faults.

## A16 new soft budgets

Use agy-soft-budget (A06 OR table) as a separate hand oracle. O/F/L have separate
limits. For each used depth/paths/contexts/cells/work dimension find minimum k;
k must equal the full hand table, k-1 and zero must drop only that entire block,
without payload. The driver checks unchanged report verdict, diagnostics, old
Agency and the other three blocks. Record exact measured k. O must not consume
unneeded paths/contexts. P is constant under every failure. Add direct overflow
and allocation-before-charge tests; searching k alone cannot prove these.

## A17 legacy budget

Keep agency::tests::injected_limits_accept_k_and_reject_k_plus_one_units_of_required_work
unchanged, including its five existing dimensions and original agency-origin
input. An old failure is U-SEM-0, not a viewpoints not_derived. Public report
failure is unsupported/4 with agency null and properties empty. Derive itself
does not prepend W-CUT-0; report does.

## Remaining private/integration seams

- Optional path recorder must charge zero extra legacy work in None mode.
- If_none-only EndId may be constructed in public IR; old DSL requires an
  explicit end label, so A12 registers it after End. Test the IR-only sink too.
- Empty state-guard arms taint F, but not equal L count functions.
- Unknown requires taints only its own count and dependent suffix calls.
- A2 integration: accepted Actor observed/per_case observed guards taint F/L;
  requires follows A2 symbolic substitution. Do not change symbolic here.
- B2 integration: resource-wait cancellation still has static sibling count 1
  and cancellation annotation true. Do not interpret this as runtime starts.
- C1 integration: recapture only new agency_viewpoints manifests with explicit
  binary/revision/job count, inspect C1 raw projector interaction, never rewrite
  any old manifest. Sources and report sites are independently frozen pre-C2.

No private injection, A2/B2/C1 integration or M01..M35 semantic mutation is
claimed executed merely by the existence of these obligations.
