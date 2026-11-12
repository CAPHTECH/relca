# ADR 0049 mutation survivors: hand oracles

Baseline for this acceptance: 38e88cd. These are inline integration/private
inputs, not additions to the closed 67-name qtl population. Sources are shared
by core and the real CLI in support/quantile_mutation_inputs.rs. Existing fixture
sources are reused only as input text, never as a source of newly expected values.

## Independent calculations

| Fault | Test/input | Derivation before running the implementation |
|---|---|---|
| F16a | f16a | One arrival, duration 2 or 4 with mass 1/2 each; within3 excludes4. F(2)=1/2, D=1/2. Median2 exact, P95 beyond_reached_mass/no bound, conditional P95=2 exact. S/0. |
| F18l, F18s1 | live legacy, check/simulate | Two fork calls at time0 exceed live_tasks1. This is an unfinished tick, earliest0, not1. All median null/truncated LB0; sample N1 C1. I/2. |
| F18r, F18s | live arrivals, check/simulate | Same arithmetic, separate arrivals driver and independent reference leaf. Both must retain LB0; sample N1 C1. I/2. |
| F19sa, X-censor-never-sa | f19sa_censor_never | The fixed seed41020 sample has two hits2 and two late durations4 (existing hand sample oracle). Clock3 gives censor earliest4, not last event2. N4 C2; median2 identified, P95 null/truncated LB4. I/2. |
| F25s | f25s | Arrivals[0,0], concurrency1 external FIFO, duration3. Receipt[0,3], ends[3,6], arrival-relative sample[3,6]. p1 both forms6, N2 C0. S/0. |
| F24i | f24i | Legacy initial hit=true at0, within0 inclusive. Both medians0 exact. S/0. |
| F40b | f40b | Legacy capacity0 is a deadlock, not an unknown continuation. All median beyond_reached_mass, conditional zero_denominator, no bounds. W-DEADLOCK-0, V/1. |
| X-within-excl-c | within_censor_legacy/arrivals | Clock3/within4 yields earliest=latest=4. F(2)=1/2; optimistic F(4)=1, pessimistic=1/2. Median2 exact; P95 null/truncated LB4. Conditional CDF at2 ranges[1/2,1], so conditional P95 null/truncated LB2. I/2. |
| X-float-p | float_p_exhaustive | p=50000000000000001/10^17 is strictly larger than1/2, so F(2)=1/2 does not suffice; F(4)=1. Median4 exact. S/0. |
| X-float-p-s | float_p_sample | Pooled[2,2,4,4], p just above1/2 requires rank3, hence4; N4 C0. Safety remains unproved in simulation, I/2. |
| X-never-censor-r | never_censor_reference | Case1 returned at2 is a closed miss; case2 arrives5 and is clock-censored at6, earliest2. D_hi/n=1/2. Q25 null/truncated LB2; Q75 beyond_reached_mass/no bound; given Q50 truncated/no bound because D_lo=0. V/1: case1 reachable done is definitively violated. The initial I/2 calculation was corrected after observing the quantile-blank control (V/1) with the earlier binary. |
| X-arith-diag | arithmetic_cut... | Nineteen continuation coins and a final coin use exact 63-digit thirds. The all-yes probability denominator grows beyond4096 bits (20*log2(10^63)>4185); other paths retain deadlock and numeric diagnostics. Quantile null/truncated must not erase old W-DEADLOCK-0,W-BUDGET-0 or V/1. A same-length blank declaration control independently freezes every old report position. |
| F34c | quantile_wire::f34c... | A legitimate reason-bearing null with optional LB4 decodes; changing only reason to bogus must fail invalid missing quantile reason. Test exhaustive and sample, with and without LB, not an unrelated shape error. |

## Private obligations

- F15b: scan reference.rs and its private modules for engine reducer identifiers
  sample_quantile/inverse_quantile/first_curves/first_reduce. Shared observation
  types are permitted. The dependency contract catches aliased imports too;
  it is not a numerical-independence proof or a general Rust call-graph verifier.
- F38: a production first_curves input contains a chance hit2 and a clock censor
  earliest100001. Breakpoints are exactly[0,2,100001]. Count actual calls to the
  production reduction through its root action callback: D once, A/B three times
  each, total7. No timeout and no linear-clock benchmark. Existing graph-build
  counts alone could not detect this fault; no new src counter is necessary.
- World adapter: the T24 oracle attributes transitions11/12 to case1 and gives
  case2_trace=[]. Assert adapter views at11 are (true,true,Some(11)) for case1
  and (true,true,None) for case2, then actual observer first hits[11,1]. The current
  src adapter fabricates Some(11) for both. This test intentionally remains RED
  for the implementation owner. After repair, injecting a self-trace-only
  observer that skips None must kill the existing engine/reference world tests.

## Deferred and limits

X-unident-g is not executed here: source and hand null/undetermined LB2 oracle
are appended to quantile_deferred.json for B6 horizon integration. F35 remains
an A7 integration obligation for old quality rows -> metrics.waiting ->
quality.quantile -> probability.scheduler, including refusal/abort paths.

Received equivalent classifications, not re-proved here: F30 (reason subsumes D
unknown), F35 (no waiting row on this base), F37r (accepted production views
always have an event time), F34e (StrictValue rejects floats earlier), X-pop-prefix
(cfg(test), disk set unchanged), X-unselected (no multi-candidate action observed;
not a proof). The F37r synthetic adapter weakness is exposed above, not silently
counted as calibrated coverage. Actual runs/calibration exits and restored-src
checks are execution observations, separate from the hand expectations here.
