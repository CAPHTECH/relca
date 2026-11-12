# ADR 0044 integration acceptance — hand basis

These inputs are tests, not additional fixture directories. The 140 Rule names
and historical numeric populations stay fixed. A failing test is not evidence
of implemented correctness or calibration.

## Sources of expectations

- ADR 0044: fixed whole-run values, bool/enum only, prior-version Authority,
  all-configuration checks, qualified state names and closed Rule judgement.
- ADR 0038: inline enums are accepted only in record fields and Capability results,
  not Actor state or decides; a named record has explicit
  record metadata and must not masquerade as an enum Rule value.
- ADR 0041: version checks/lost-update classification describe mutable shared
  Actor state, not a fixed Rule or its declared version.
- ADR 0042: configuration chooses the Rule, scenario chooses arrivals/horizon.
- Rule U is neither a process J, a person,
  a generation root nor Authority to reverse an effect.
- B6-to-A6 transfer: Rule models have absent or explicitly aborted quality;
  normal estimates must not be silently produced. Non-Rule bytes stay fixed.
  Aborted follows the existing ADR0045 cell schema: reason=aborted, numeric
  values null and checked empty; analysis remains exhaustive or simulation,
  not a newly invented aborted mode.

The oracle is written from these contracts and the input arithmetic, not from
new analyzer output. Supplemental positive controls use existing syntax.

## Finite calculations

| Input | Hand result |
|---|---|
| T21 open named Mode enum | approval 0..2; pass posts 2..3, fail refuses at2; posted3/work3, held2/work2, K satisfied |
| T21 default closed (single edit) | both approvals refuse at2; only held2/work2; K vacuous, exit5 |
| T21 inline decides (corrected negative) | U-SYN-0, unsupported, exit4; inline enums are not accepted at this position |
| S01 named record | good side adds only type R; changing only the decides type bool to R is U-SEM-0, one located rule-type error, no E-TYPE cascade |
| S22 scenario amend | U-SYN-0 at amend token, exit4 and exact old W-CUT-0; the diagnostic is the existing scenario item message |
| T24 AsIs/Light, AsIs/Later | arrival0/5, posted completes3/8 with elapsed/work3; held completes2/7 with elapsed/work2 |
| T24 ToBeB/Light, ToBeB/Later | only posted completes3/8 with elapsed/work3; K violated; effective version r2, value false |
| S26 version target | scratch is a valid shared observed writer; replacing just the checked name by fixed Rule enabled is unsupported at lower with a source span |
| Held + horizon3 | arrivals0,1,2; first task needs5, so all three cut_off, no fabricated completed time; not computational truncation |
| reject/overflow + horizon3 | same first task cut_off; receipts2/3 rejected or overflowed; those outcomes do not become cut_off |
| Held drain without horizon | first task0..5, second5..10, third10..15; clock20 does not cut them |
| supplementary process J | Reader1 + approval2 + post1 gives posted4/work4, held3/work3; original no-read Approver is not retroactively called J |

T24 lowers all four selections in both orders to catch value leaks. The explicit
limit controls replace the immutable Rule by a fixed observed-case constant,
retaining requires and its empty refusal inventory on both sides. Removing
requires entirely would legitimately change Some(empty) to None and is not a
byte-invariance oracle.

## Agency hand table

Use exactly rule-basic, its ToBeB selection (the broken-t01 invocation), and
rule-diff. Ledger.post E is actors[Ledger], people[], unattributed[Ledger],
permitted[]. Process J is empty. AsIs U is the r1 Rule item; ToBeB U is empty.
Post-review correction: single_actor=true in AsIs, false in ToBeB; both sides single_person=unknown,
single_generation_root=false, generation_roots[]. W-RESP-0 is ordered
ApprovalRule then Ledger in AsIs; Ledger alone in ToBeB.

F and L keep lexical support even if the fixed Rule makes requires true:
Approver.approve normal1..1 both sides; Ledger.post0..1 to1..1.
`rule_integration_inputs::differences()` now spells out the ten expected changed
cells in the ADR0043 table order, including known absent refusal=null rather
than not_comparable/state_guards. The new concentration.single_actor change is
specified below; no additional origin, lineage or E changes are inferred.

The positive wire tests independently build that Rule U cell, then require its
acceptance through public Agency/Differences deserialization and the CLI matcher.
Fault controls remove kind, add unknown fields, mix process fields, empty reads
or duplicate the Rule item. A negative test passing while the positive union is
RED in the core DTO is not claimed as calibrated enforcement of its future
union. The CLI checker includes the explicit Rule
arm in effects.u and the two named difference cells. Its positive wire and each
negative control are required to pass before freezing fixture_match.rs. It does
not broaden J or non-Rule wire and checks Rule Actor/effective-version identity.

## Post-review acceptance (2026-10-29 rulings)

The later ruling preserves aborted quality, but requires disclosure immediately
after the four quality assumptions. The closed row is field `quality.rule`, value
`unsupported`, affects `quality for models with Rule state is not reported because
the combination of ADR 0044 and ADR 0045 is unsupported`. It is absent without
Rule state (and without quality). Execution itself is not aborted: removing only
property/metric declarations leaves the entire Outcome unchanged after removing
its quality member. QUALITY finishes done at1/work1 in both configurations;
simulation completes all four replications. Check exits0; simulation exits2
(sample safety is not exhaustive proof).

Held quality uses the existing arrivals0,1,2, single active case, five-minute
service. At H3 all three are cut_off, without computational truncation. Without
horizon all three finish at5,10,15, elapsed5,9,13 and work5 each. Four simulation
replications finish their observation, including the horizon observations. Rule
quality has every cell aborted, null numeric values, checked empty in all four
backend/mode combinations. Replacing Rule by a fixed observed-case true gives
normal quality: the property is already true at receipt, count3/rate1 even at
H3; the drain control has end.done count3/rate1. No horizon unknown affects this
already-hit property.

For rule-partial.broken-t12, mode stays strict in AsIs AND ToBeB: the amendment
changes only approval_required. Both have zero posts, held2/work2, K vacuous,
and W-RESP-0 messages ApprovalRule then Ledger. Fixed-false reachability does
not erase the normative Rule U origin.

For the agy2-atomic-u crossing, append R(r1,required=true), include R, replace
only `v == pass` by `R.required == false || v == pass`. Clerk0..1, Approver1..2,
pass posts2..3, fail returns2. Ledger.keys is a state_refuser with
on_normal_path=false. Process J is exactly v/Approver.approve (a1, all_paths,
bypasses0). U is that process item followed by R/r1/reads[R.required]. The
required=false control always posts3, process J remains v but stops=none,
bypasses1; U is empty. Old refusal remains underived (empty rows with
state_guards unchecked), not known absence. W-RESP-0 is Ledger,R for
true and Ledger for false, in declaration order. Concentration is false/unknown/
true with sole generation root e/Clerk.enter in both; R is not a carrier/person/
generation root.

The m5 ruling supersedes the older basic concentration hand table above:
The definition requires a carrier Actor union of cardinality1 and nonempty U;
an unattributed carrier leaves the person unknown, and call roots are counted separately.
Apply it with the later m5 ruling excluding fixed Rule Actors from that union.
rule-basic AsIs has only Ledger as carrier, and Rule U exists, hence
single_actor=true; ToBeB has no U, hence false. single_person remains unknown,
single_generation_root=false and generation_roots=[] on both. This adds the
single_actor difference after effects.u and before refusal.judgements (ten
semantic changes, rather than the historical nine). Rule is not counted as a
second carrier that would dilute the concentration signal.

T21 inline parser negative (both version-first and decides-first) is exactly one U-SYN-0 error with message
`unsupported syntax: inline enum position is unsupported` and a source span.
The report appends exactly the historical W-CUT-0 (parse failed before resolving
the declared Cut); it is not an additional parser error. The atomic W-RESP-0
order is Ledger,R; the unchanged legacy suppression does not invent an Approver warning.
The Day check exit is2 at H3 (no completed safety proof),0 after drain; all
simulation invocations exit2 because safety is sampled. These exits follow
the horizon and sampled-safety rules, not the mere presence/absence of quality.
Both Rule forms must expose the type error and enum
span. Old non-Rule malformed-input compatibility remains separate and unchanged.

## Public/private IR and remaining verification

Engine and reference have separate tests for canonical record types, invalid
owner/variable/type indices, missing/duplicate metadata, invalid grants, invalid
under targets, Rule version_check, last_write_wins, concurrency resolution and
no_lost_update. The direct private record tests call each validate_rules without
letting the earlier generic record checker hide a missing Rule check. Existing
private candidate counters, actual census and replay poison/capture tests remain
linked and are run separately; public checks alone do not cover them.

Run from the repository root; keep temporary inputs in `.tmp`:

```sh
CARGO_NET_OFFLINE=true TMPDIR=.tmp cargo test -p relca-core --test rule_actor_integration --no-fail-fast
CARGO_NET_OFFLINE=true TMPDIR=.tmp cargo test -p relca-cli --test rule_actor_integration --no-fail-fast
CARGO_NET_OFFLINE=true TMPDIR=.tmp cargo test -p relca-core --lib rule_integration_tests --no-fail-fast
```

Runtime acceptance does not read temporary captures. Full workspace success and
production fault injection require separate execution evidence, not just these hand tables.
