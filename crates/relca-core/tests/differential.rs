//! Independent ADR/IR oracles. No implementation helpers from engine are used.
//! H01..H22's ADR-based arithmetic witnesses below intentionally precede any
//! differential assertion; generated outputs are never used as fixed oracles.
//!
//! Sensitivity map (a proposed mutation is NOT a recorded kill).
//! e/w/r = elapsed/work/result_at; every normal input uses the same IR for
//! engine and report after the explicit provenance-only normalization below.
//! ID | input | proposed semantic error | observable field
//! S01 | H01; arithmetic abort test | tick != minute; wrap 1+MAX | e/w; U-SEM-0/null
//! S02 | H01 | continue a serial call without waiting | e=8, w=8 / abort
//! S03 | H23 | dispatch one branch before creating/checking the whole fork | cut, terminals
//! S04 | H02,H17 | idle despite eligible task; choose one cap1 order | e/w; r range1..5
//! S05 | resource-contention,H06 | permit cap1 overlap; stop occupancy at request | e8->5; w3..5
//! S06 | resource-contention,H02 | include queued wait; work=elapsed | w8 vs w11/5
//! S07 | H03 | take first result only | red/blue/green terminal set and reachable
//!       Internal preselection before finish is indistinguishable if all values
//!       remain enumerated and none is exposed to control before finish.
//! S08 | H22 | use a global member index instead of the binding's enum | done vs no_match
//! S09 | H26 | all requires all matching/successful values, or returns early | done,e3,w4
//! S10 | H28,H17 | adopt first finish even if nonmatch; lose earlier winner | r3,done; r extrema
//! S11 | H04,H05 | ignore tie permutation / drop nondet winner | indistinguishable:
//!       equal-time finishers are already terminal, winner ID is not in Outcome.
//!       Only reference counters observe the reference's choices (not engine's).
//! S12 | H27 | record adoption relative to fork, or clear at drain | r4 vs r1/null
//! S13 | H28,H21 | jump none on first nonmatch, or never jump after all | terminal names,e/r
//! S14 | H06,H20 | conflate drain and cancel_then_drain; don't drain | e/w ranges
//! S15 | H17 | start cancelled queued loser / count its planned duration | e/w minima1
//! S16 | H06,H07,H08 | shortest ack only; clamp to horizon | e.max4,w.max5; cut
//! S17 | H19 | let a later ack beat earlier normal finish | e2,w3,cut=false
//!       Dropping just normal OR ack at the SAME tick is indistinguishable:
//!       loser results are unavailable to valid control, and e/w/r coincide.
//! S18 | H17,any-drain | cancel forbidden running task | e.max6 / fixture e5,w8
//!       missing-cancel fixture: engine には届かない(lower の拒否で守られる)。
//!       engine の変異は差分検査では区別不能。lower/report E-CANCEL-0 を照合する。
//! S19 | H17,H18 | fix adoption-before-dispatch OR dispatch-before-adoption | e/w extrema
//! S20 | H14,H24,H25,H29 | execute end suffix; And<->Or; lose if continuation | e/w; end name
//! S21 | H23,H10,H15 | count only Running; >= instead of >; count finished | cut / terminals
//! S22 | H01,H02 | clock >= instead of > | exact-bound terminals,cut,reachable
//! S23 | H07,H08 | summarize partial cut paths, or erase completed witnesses | terminals,e/w,cut
//! S24 | H07,H08,quick-never-passes fixture | witness loses to cut; no witness=violated
//!       even with cuts; unconditionally satisfied | per-property verdicts
//! S25 | resource-contention,H07 | over-dispatch; satisfied despite cut | safety verdict,e
//!       Hardcoding satisfied vs correctly detecting *no* overcommit on accepted
//!       models is indistinguishable: guard prevents a counterexample. Synthetic
//!       Evidence tests cover reference aggregation, not an engine path/kill.
//! S26 | H11,H12 | turn stopped nonterminal into clock cut or fake terminal | W-DEADLOCK-0,cut
//! S27 | H12,H13,H30; zero-verify test | ignore deadlock, especially with cut/witness;
//!       treat verify0 as satisfied | overall violated / unsupported, unchanged properties
//! S28 | H11 | default cap0 to1 | terminal
//!       missing-capacity fixture: engine には届かない(lower の拒否で守られる)。
//!       engine の変異は差分検査では区別不能。lower/report E-CAP-0 を照合する。
//! S29 | H31 | reuse one extremal path for every metric; collapse min/max | e9..11,w12..17,r2..9
//! S30 | H24,H25,H03 | reorder/drop verify entries; duplicate end summaries | property IDs/order,
//!       verdicts, terminal uniqueness; Mixed/Twice separately reject ambiguous summaries
//! S31 | H24,H07,H12; rejected fixtures; Mixed; fold test | wrong precedence or evaluate
//!       properties after static rejection | report overall/properties; all ranks reference-only
//! S32 | all normal comparisons | spurious budget exhaustion on these small inputs | W-BUDGET-0,
//!       cut / loss of terminal. Exact DEFAULT_STATE_BUDGET threshold is indistinguishable
//!       in this bounded corpus: only explore is authorized, no budget adapter; reference
//!       path budget is different. No claim about engine's exact boundary/retained evidence.
//! S33 | Mixed direct IR + report static refusal; reference_abort_contract | merge adoption
//!       present/absent into a number, leak partial terminals, erase known witness | U-SEM-0,
//!       terminals=[], witness satisfied, report truncated=null. Retained deadlock/capacity
//!       at engine abort are indistinguishable without an observed pre-abort path order;
//!       the independently ordered reference's abort/evidence tests are not engine coverage.
//! S34 | all fixtures; H11..13,H30 | omit/invent code, wrong severity | exact code sets/severity
//! S35 | serial-time/cut-declared/missing-loses | Cut changes execution or warnings disappear |
//!       equal e8/w8; fixed W-CUT-0 / none / W-CUT-1 oracle and report/runtime comparisons
//! Static-refusal parts of S31/S34 (excluded-call, missing-member, configuration
//! selection missing, unknown terminal) and S33's report Mixed/Twice inputs:
//! engine には届かない(lower の拒否で守られる)。engine の変異は差分検査では区別不能。
//! missing-join is rejected even earlier by parse; it also cannot test an engine
//! mutation. Direct-IR Mixed/Twice abort checks are separate, not static coverage.

#[path = "support/conflict_population.rs"]
mod conflict_population;
use relca_core::{
    Verdict::{self, *},
    diag::{self, Diagnostic, Span},
    ir::*,
    outcome::{Outcome, Range, TerminalOutcome},
    reference::{self, Evidence, Limits, ReferenceRun, StopReason},
};
use std::{collections::BTreeSet, fmt::Write, fs, path::PathBuf, time::Instant};
#[path = "support/actor_observed_population.rs"]
mod aobs_population;
#[path = "support/authority_population.rs"]
mod authority_population;
#[path = "support/configuration_diff_population.rs"]
mod diff2_population;
#[path = "support/effect_standing_population.rs"]
mod effect_population;
#[path = "support/integer_acceptance.rs"]
mod integer_population;
#[path = "support/limits_population.rs"]
mod limits_population;
#[path = "support/common_cause_population.rs"]
mod lin_population;
#[path = "support/mutation_default_population.rs"]
mod mutation_default_population;
#[path = "support/percase_acceptance.rs"]
mod pcase_population;
#[path = "support/policy_population.rs"]
mod policy_population;
#[path = "support/record_population.rs"]
mod record_population;
#[path = "support/requires_acceptance.rs"]
mod req_population;
#[path = "support/resource_acceptance.rs"]
mod resource_population;
#[path = "support/responsibility_extension_population.rs"]
mod resp2_population;
use agency_viewpoints_population::responsibility_oracle;
#[path = "support/scenario_population.rs"]
mod scenario_population;
#[path = "support/state_invariant_population.rs"]
mod sinv_population;

const SPAN: Span = Span { start: 0, end: 0 };

const AGENCY_FIXTURES: [&str; 9] = [
    "agency-any",
    "agency-any.broken-single-valued-a",
    "agency-lineage",
    "agency-lineage.broken-missing-evidence",
    "agency-lineage.broken-reused-source",
    "agency-origin",
    "agency-origin.broken-no-direct-read",
    "agency-refusal",
    "agency-refusal.broken-cut-guard",
];

/// The invoice example in the Phase 0 grammar (v0.2 §21). Fixed separately
/// from the old 28 and the Agency 9; each runs its diff's two configurations.
const INVOICE_FIXTURES: [&str; 4] = [
    "invoice-phase0-ai",
    "invoice-phase0-ai.broken-unguarded-approval",
    "invoice-phase0-parallel",
    "invoice-phase0-parallel.broken-one-clerk",
];

const EXECUTOR_FIXTURES: [&str; 14] = [
    "exec-anonymous",
    "exec-anonymous.broken-no-executor",
    "exec-authority",
    "exec-authority.broken-missing-grant",
    "exec-cancel",
    "exec-cancel.broken-ignore-cancel",
    "exec-permitted",
    "exec-permitted.broken-narrow-permit",
    "exec-separation",
    "exec-separation.broken-shared-approver",
    "exec-shared-person",
    "exec-shared-person.broken-reused-person",
    "exec-vacuous",
    "exec-vacuous.broken-shared-approver",
];

// The seventh population is exercised independently by observed_latent_differential.
const STATE_FIXTURES: [&str; 16] = [
    "state-agency",
    "state-agency.broken-state-guard",
    "state-cancel",
    "state-cancel.broken-ignore-cancel",
    "state-invoice-write",
    "state-invoice-write.broken-failed-approval",
    "state-latent-product",
    "state-latent-product.broken-partial-assertion",
    "state-name-alias",
    "state-name-alias.broken-missing-write",
    "state-observed-range",
    "state-observed-range.broken-fixed-route",
    "state-policy",
    "state-policy.broken-latent-difficulty",
    "state-writer-order",
    "state-writer-order.broken-parallel-writers",
];

const CONTRACT_FIXTURES: [&str; 19] = [
    "contract-cancel",
    "contract-cancel.broken-ignore-cancel",
    "contract-cut",
    "contract-cut.broken-late-target",
    "contract-diff",
    "contract-diff.broken-zero-deadline",
    "contract-invariant",
    "contract-invariant.broken-cut-guard",
    "contract-once",
    "contract-once.broken-repeat-result",
    "contract-order",
    "contract-order.broken-parallel",
    "contract-requires",
    "contract-requires.broken-cut-guard",
    "contract-response",
    "contract-response.broken-late-finish",
    "contract-response.broken-short-clock",
    "contract-vacuous",
    "contract-vacuous.broken-enable-branch",
];

fn empty_agency(actors: &[Actor]) -> relca_core::agency::Agency {
    // Independent summary for the metadata-free handwritten IR corpus. Never
    // invoke production derive to construct an expected bridge summary.
    serde_json::from_value(serde_json::json!({
        "rules":"agency-p0-1",
        "origin":actors.iter().map(|a| serde_json::json!({"actor":a.name,"direct":[],"indirect":[]})).collect::<Vec<_>>(),
        "lineage":[],"lineage_groups":[],"refusal":[],
        "unchecked":["W-RESP-1","contest_path","fallback_path","executors","via_model"]
    })).unwrap()
}

fn stmt(kind: Stmt) -> Statement {
    Statement { kind, span: SPAN }
}
fn end(id: usize) -> Statement {
    stmt(Stmt::End(EndId(id)))
}
fn eq(binding: usize, value: usize) -> Cond {
    Cond::Eq(ResultMatch {
        binding: BindingId(binding),
        value: ValueId(value),
    })
}
fn branch(condition: Cond, then_body: Block, else_body: Block) -> Statement {
    stmt(Stmt::If {
        condition,
        then_body,
        else_body,
    })
}
fn range(min: u64, max: u64) -> Range {
    Range { min, max }
}
fn terminal(id: usize, e: (u64, u64), w: (u64, u64), r: Option<(u64, u64)>) -> TerminalOutcome {
    TerminalOutcome {
        probability: None,
        end: EndId(id),
        elapsed: range(e.0, e.1),
        work: range(w.0, w.1),
        result_at: r.map(|(a, b)| range(a, b)),
    }
}
fn point(id: usize, e: u64, w: u64, r: Option<u64>) -> TerminalOutcome {
    terminal(id, (e, e), (w, w), r.map(|r| (r, r)))
}

struct Builder(Model);
impl Builder {
    fn new(name: &str, capacity: u64, clock: u64) -> Self {
        Self(Model {
            case_priority: None,
            performances: Vec::new(),
            concurrency_resolutions: Vec::new(),
            resource_time: None,
            arrival_distribution: None,
            arrival_every: None,
            scenario: None,
            horizon: None,
            fallback_capabilities: Vec::new(),
            resources: Vec::new(),
            responsibility: None,
            simulation: None,
            metrics: None,
            quality: None,
            comparison: None,
            arrivals: None,
            start_policy: None,
            admission_queue: None,
            concurrent_cases: None,
            overflow_end: None,
            kernels: Vec::new(),
            initial_distributions: Vec::new(),
            probability_queries: Vec::new(),
            state_variables: Vec::new(),
            people: Vec::new(),
            authorities: Vec::new(),
            contracts: Vec::new(),
            name: name.into(),
            version: "0.2".into(),
            types: vec![EnumType {
                name: "T".into(),
                record: None,
                members: vec!["yes".into(), "no".into(), "other".into()],
                span: SPAN,
            }],
            actors: vec![Actor {
                executors: None,
                name: "A".into(),
                source: None,
                capacity: Some(capacity),
                metadata: ActorMetadata {
                    capacity_declared: true,
                    ..Default::default()
                },
                span: SPAN,
            }],
            capabilities: vec![],
            process: Process {
                degradations: Vec::new(),
                name: "P".into(),
                bindings: vec![],
                forks: vec![],
                body: vec![],
                span: SPAN,
            },
            ends: vec!["done".into(), "no_match".into()],
            bounds: Bounds {
                cases: 1,
                live_tasks: 2,
                clock,
            },
            verify: vec![
                Verify {
                    property: Property::NoResourceOvercommit,
                    span: SPAN,
                },
                Verify {
                    property: Property::ReachableEnd(EndId(0)),
                    span: SPAN,
                },
            ],
            assumptions: vec![],
            diagnostics: vec![Diagnostic {
                code: diag::W_CUT_0.into(),
                severity: diag::Severity::Warning,
                message: "undeclared Cut".into(),
                span: None,
            }],
            cut: None,
            agency: empty_agency(&[]),
        })
    }
    fn call(&mut self, duration: u64, actor: usize, results: &[usize], cancel: Cancel) -> Call {
        let id = self.0.capabilities.len();
        self.0.capabilities.push(Capability {
            qualified_executors: None,
            performance: None,
            version_check: None,
            read_variables: Vec::new(),
            atomic: None,
            uses: Vec::new(),
            duration_distribution: None,
            kernel: None,
            kernel_reads_latent: Vec::new(),
            kernel_writes: Vec::new(),
            writes: Vec::new(),
            requires: None,
            start_writes: Vec::new(),
            permission: None,
            name: format!("c{id}"),
            actor: ActorId(actor),
            return_type: TypeId(0),
            params: vec![],
            reads: vec![],
            commits: false,
            duration,
            results: results.iter().map(|&v| ValueId(v)).collect(),
            cancel,
            span: SPAN,
        });
        self.0.process.bindings.push(Binding {
            name: format!("b{id}"),
            ty: TypeId(0),
            span: SPAN,
        });
        Call {
            binding: BindingId(id),
            capability: CapabilityId(id),
            args: vec![],
            span: SPAN,
        }
    }
    fn all(&mut self, calls: Vec<Call>) -> Statement {
        self.fork(calls, Join::All)
    }
    fn fork(&mut self, branches: Vec<Call>, join: Join) -> Statement {
        let id = ForkId(self.0.process.forks.len());
        self.0.process.forks.push(format!("f{}", id.0));
        stmt(Stmt::Fork { id, branches, join })
    }
    fn finish(mut self, mut body: Block) -> Model {
        // Define every declared end in the IR itself, not only in the report
        // bridge. Extra definitions are unreachable after an unconditional end
        // or an if whose arms end. H12/H13 deliberately have fallthrough paths.
        let mut defined = BTreeSet::new();
        defined_ends(&body, &mut defined);
        for i in 0..self.0.ends.len() {
            if !defined.contains(&EndId(i)) && !matches!(self.0.name.as_str(), "H12" | "H13") {
                body.push(end(i));
            }
        }
        self.0.process.body = body;
        self.0.agency = empty_agency(&self.0.actors);
        self.0
    }
}
fn any(calls: &[Call], tie: Tie, remaining: Remaining) -> Join {
    Join::Any {
        matches: calls
            .iter()
            .map(|c| ResultMatch {
                binding: c.binding,
                value: ValueId(0),
            })
            .collect(),
        tie,
        remaining,
        if_none: EndId(1),
    }
}
fn pair_any(
    b: &mut Builder,
    durations: [u64; 2],
    cancel: Cancel,
    remaining: Remaining,
) -> Statement {
    let calls = vec![
        b.call(durations[0], 0, &[0], cancel),
        b.call(durations[1], 0, &[0], cancel),
    ];
    let join = any(
        &calls,
        Tie::By(calls.iter().map(|c| c.binding).collect()),
        remaining,
    );
    b.fork(calls, join)
}

struct Case {
    id: &'static str,
    model: Model,
    terminals: Vec<TerminalOutcome>,
    truncated: bool,
    properties: Vec<Verdict>,
    overall: Verdict,
    deadlock: bool,
}
fn case(id: &'static str, model: Model, terminals: Vec<TerminalOutcome>) -> Case {
    Case {
        id,
        properties: vec![Satisfied; model.verify.len()],
        model,
        terminals,
        truncated: false,
        overall: Satisfied,
        deadlock: false,
    }
}

fn handwritten() -> Vec<Case> {
    let mut cases = Vec::new();
    let unspecified = Cancel::Unspecified;
    let forbidden = Cancel::ForbiddenAfterStart;
    let coop = Cancel::Cooperative { ack_within: 3 };
    let cancel = Remaining::CancelThenDrain;
    let drain = Remaining::Drain;

    // H01: occupancy [0,3) + [3,8), at the exact clock boundary.
    let mut b = Builder::new("H01", 1, 8);
    let a = b.call(3, 0, &[0], unspecified);
    let c = b.call(5, 0, &[0], unspecified);
    cases.push(case(
        "H01",
        b.finish(vec![stmt(Stmt::Call(a)), stmt(Stmt::Call(c)), end(0)]),
        vec![point(0, 8, 8, None)],
    ));
    // H02: [0,3) and [0,5): elapsed=max=5, work=3+5=8.
    let mut b = Builder::new("H02", 2, 5);
    let calls = vec![
        b.call(3, 0, &[0], unspecified),
        b.call(5, 0, &[0], unspecified),
    ];
    let all = b.all(calls);
    cases.push(case(
        "H02",
        b.finish(vec![all, end(0)]),
        vec![point(0, 5, 8, None)],
    ));
    // H03: every result, including the third, selects its own nested end.
    let mut b = Builder::new("H03", 1, 1);
    b.0.ends = vec!["red".into(), "blue".into(), "green".into()];
    b.0.verify.extend([1, 2].map(|i| Verify {
        property: Property::ReachableEnd(EndId(i)),
        span: SPAN,
    }));
    let call = b.call(1, 0, &[0, 1, 2], unspecified);
    let select = branch(
        eq(0, 0),
        vec![end(0)],
        vec![branch(eq(0, 1), vec![end(1)], vec![end(2)])],
    );
    cases.push(case(
        "H03",
        b.finish(vec![stmt(Stmt::Call(call)), select]),
        (0..3).map(|i| point(i, 1, 1, None)).collect(),
    ));
    for (id, tie) in [
        ("H04", Tie::By(vec![BindingId(1), BindingId(0)])),
        ("H05", Tie::Nondet),
    ] {
        let mut b = Builder::new(id, 1, 1);
        b.0.actors.push(Actor {
            executors: None,
            name: "B".into(),
            source: None,
            capacity: Some(1),
            metadata: ActorMetadata {
                capacity_declared: true,
                ..Default::default()
            },
            span: SPAN,
        });
        let calls = vec![b.call(1, 0, &[0], forbidden), b.call(1, 1, &[0], forbidden)];
        let join = any(&calls, tie, drain);
        let fork = b.fork(calls, join);
        cases.push(case(
            id,
            b.finish(vec![fork, end(0)]),
            vec![point(0, 1, 2, Some(1))],
        ));
    }
    // H06..08: winner [0,1), loser [0,ack) with ack=2,3,4.
    // Reserving a far ack does NOT cut immediately; the unfinished tick does.
    for (id, clock) in [("H06", 4), ("H07", 3), ("H08", 1)] {
        let mut b = Builder::new(id, 2, clock);
        let fork = pair_any(&mut b, [1, 10], coop, cancel);
        let ts = if clock == 1 {
            vec![]
        } else {
            vec![terminal(0, (2, clock), (3, clock + 1), Some((1, 1)))]
        };
        let mut c = case(id, b.finish(vec![fork, end(0)]), ts);
        if clock < 4 {
            c.truncated = true;
            c.properties[0] = Inconclusive;
            c.overall = Inconclusive;
        }
        if clock == 1 {
            c.properties[1] = Inconclusive;
        }
        cases.push(c);
    }
    for (id, live) in [("H09", 1), ("H10", 2)] {
        let mut b = Builder::new(id, 2, 5);
        b.0.bounds.live_tasks = live;
        let calls = vec![
            b.call(3, 0, &[0], unspecified),
            b.call(5, 0, &[0], unspecified),
        ];
        let fork = b.all(calls);
        let mut c = case(
            id,
            b.finish(vec![fork, end(0)]),
            if live == 1 {
                vec![]
            } else {
                vec![point(0, 5, 8, None)]
            },
        );
        if live == 1 {
            c.truncated = true;
            c.properties = vec![Inconclusive; 2];
            c.overall = Inconclusive;
        }
        cases.push(c);
    }
    let mut b = Builder::new("H11", 0, 100);
    let call = b.call(1, 0, &[0], unspecified);
    let mut c = case(
        "H11",
        b.finish(vec![stmt(Stmt::Call(call)), end(0)]),
        vec![],
    );
    c.properties[1] = Violated;
    c.overall = Violated;
    c.deadlock = true;
    cases.push(c);
    let mut b = Builder::new("H12", 1, 100);
    b.0.verify.truncate(1);
    b.0.ends.clear();
    let mut c = case("H12", b.finish(vec![]), vec![]);
    c.overall = Violated;
    c.deadlock = true;
    cases.push(c);
    let mut b = Builder::new("H13", 1, 1);
    b.0.ends.truncate(1);
    let call = b.call(1, 0, &[0, 1], unspecified);
    let mut c = case(
        "H13",
        b.finish(vec![
            stmt(Stmt::Call(call)),
            branch(eq(0, 0), vec![end(0)], vec![]),
        ]),
        vec![point(0, 1, 1, None)],
    );
    c.overall = Violated;
    c.deadlock = true;
    cases.push(c);
    let mut b = Builder::new("H14", 1, 0);
    let call = b.call(100, 0, &[0], unspecified);
    cases.push(case(
        "H14",
        b.finish(vec![end(0), stmt(Stmt::Call(call))]),
        vec![point(0, 0, 0, None)],
    ));
    // H15: max(1,2)+max(3,4)=6; sum=10, live returns to zero at each join.
    let mut b = Builder::new("H15", 2, 6);
    let calls = vec![
        b.call(1, 0, &[0], unspecified),
        b.call(2, 0, &[0], unspecified),
    ];
    let first = b.all(calls);
    let calls = vec![
        b.call(3, 0, &[0], unspecified),
        b.call(4, 0, &[0], unspecified),
    ];
    let second = b.all(calls);
    cases.push(case(
        "H15",
        b.finish(vec![first, second, end(0)]),
        vec![point(0, 6, 10, None)],
    ));
    let mut b = Builder::new("H16", 2, 7);
    let first = pair_any(&mut b, [1, 3], forbidden, drain);
    let calls = vec![
        b.call(2, 0, &[0], unspecified),
        b.call(4, 0, &[0], unspecified),
    ];
    let second = b.all(calls);
    cases.push(case(
        "H16",
        b.finish(vec![first, second, end(0)]),
        vec![point(0, 7, 10, Some(1))],
    ));
    // H17: cap1 start orders: short first -> adopt at1; long first -> at5.
    // Cancel queued loser: e=w=1 or5. Start loser before adoption: e=w=6.
    // H18: short first, start loser before adoption -> ack2..4; long first cuts.
    for (id, contract, clock) in [("H17", forbidden, 6), ("H18", coop, 4)] {
        let mut b = Builder::new(id, 1, clock);
        let fork = pair_any(&mut b, [1, 5], contract, cancel);
        let mut c = case(
            id,
            b.finish(vec![fork, end(0)]),
            vec![terminal(
                0,
                (1, clock),
                (1, clock),
                Some((1, if id == "H17" { 5 } else { 1 })),
            )],
        );
        if id == "H18" {
            c.truncated = true;
            c.properties[0] = Inconclusive;
            c.overall = Inconclusive;
        }
        cases.push(c);
    }
    // H19: ack=2 ties finish=2; ack=3/4 loses to natural finish=2.
    let mut b = Builder::new("H19", 2, 2);
    let fork = pair_any(&mut b, [1, 2], coop, cancel);
    cases.push(case(
        "H19",
        b.finish(vec![fork, end(0)]),
        vec![point(0, 2, 3, Some(1))],
    ));
    let mut b = Builder::new("H20", 2, 5);
    let fork = pair_any(&mut b, [1, 5], coop, drain);
    cases.push(case(
        "H20",
        b.finish(vec![fork, end(0)]),
        vec![point(0, 5, 6, Some(1))],
    ));
    let mut b = Builder::new("H21", 2, 2);
    let calls = vec![b.call(1, 0, &[1], forbidden), b.call(2, 0, &[1], forbidden)];
    let join = any(&calls, Tie::Nondet, cancel);
    let fork = b.fork(calls, join);
    b.0.verify[1].property = Property::ReachableEnd(EndId(1));
    cases.push(case(
        "H21",
        b.finish(vec![fork, end(0)]),
        vec![point(1, 2, 3, None)],
    ));
    let mut b = Builder::new("H22", 1, 2);
    b.0.types[0].members = vec!["yes".into(), "no".into()];
    b.0.types.push(EnumType {
        name: "U".into(),
        record: None,
        members: vec!["no".into(), "yes".into()],
        span: SPAN,
    });
    let a = b.call(1, 0, &[0], unspecified);
    let c = b.call(1, 0, &[1], unspecified);
    b.0.capabilities[1].return_type = TypeId(1);
    b.0.process.bindings[1].ty = TypeId(1);
    let condition = Cond::Or(vec![
        Cond::And(vec![eq(0, 0), eq(1, 1)]),
        Cond::And(vec![eq(0, 1), eq(1, 0)]),
    ]);
    cases.push(case(
        "H22",
        b.finish(vec![
            stmt(Stmt::Call(a)),
            stmt(Stmt::Call(c)),
            branch(condition, vec![end(0)], vec![end(1)]),
        ]),
        vec![point(0, 2, 2, None)],
    ));
    // H23: two CREATED tasks exceed live=1 even though cap1 permits only one
    // running task. Counting Running only would incorrectly finish at 2/2.
    let mut b = Builder::new("H23", 1, 2);
    b.0.bounds.live_tasks = 1;
    let calls = vec![
        b.call(1, 0, &[0], unspecified),
        b.call(1, 0, &[0], unspecified),
    ];
    let fork = b.all(calls);
    let mut c = case("H23", b.finish(vec![fork, end(0)]), vec![]);
    c.truncated = true;
    c.properties = vec![Inconclusive; 2];
    c.overall = Inconclusive;
    cases.push(c);
    // H24/H25: true && false is false, true || false is true. Unlike
    // H22, replacing either operator changes the selected terminal.
    for (id, and) in [("H24", true), ("H25", false)] {
        let mut b = Builder::new(id, 1, 2);
        let a = b.call(1, 0, &[0], unspecified);
        let c = b.call(1, 0, &[1], unspecified);
        let operands = vec![eq(0, 0), eq(1, 0)];
        let condition = if and {
            Cond::And(operands)
        } else {
            Cond::Or(operands)
        };
        b.0.verify.push(Verify {
            property: Property::ReachableEnd(EndId(1)),
            span: SPAN,
        });
        let mut c = case(
            id,
            b.finish(vec![
                stmt(Stmt::Call(a)),
                stmt(Stmt::Call(c)),
                branch(condition, vec![end(0)], vec![end(1)]),
            ]),
            vec![point(usize::from(and), 2, 2, None)],
        );
        c.properties[if and { 1 } else { 2 }] = Violated;
        c.overall = Violated;
        cases.push(c);
    }
    // H26: all waits for failures too; the `no` value of one branch does not
    // mean join failure, and remains available to subsequent control.
    let mut b = Builder::new("H26", 2, 3);
    let calls = vec![
        b.call(1, 0, &[0], unspecified),
        b.call(3, 0, &[1], unspecified),
    ];
    let all = b.all(calls);
    cases.push(case(
        "H26",
        b.finish(vec![all, branch(eq(1, 1), vec![end(0)], vec![end(1)])]),
        vec![point(0, 3, 4, None)],
    ));
    // H27: absolute adoption follows a 3-minute prelude: 3+1=4,
    // drain ends at 3+5=8; work=3+1+5=9.
    let mut b = Builder::new("H27", 2, 8);
    let prelude = b.call(3, 0, &[0], unspecified);
    let fork = pair_any(&mut b, [1, 5], forbidden, drain);
    cases.push(case(
        "H27",
        b.finish(vec![stmt(Stmt::Call(prelude)), fork, end(0)]),
        vec![point(0, 8, 9, Some(4))],
    ));
    // H28: the first nonmatch at1 cannot jump to none: the match at3 wins.
    let mut b = Builder::new("H28", 2, 3);
    let calls = vec![b.call(1, 0, &[1], forbidden), b.call(3, 0, &[0], forbidden)];
    let join = any(&calls, Tie::Nondet, drain);
    let fork = b.fork(calls, join);
    cases.push(case(
        "H28",
        b.finish(vec![fork, end(0)]),
        vec![point(0, 3, 4, Some(3))],
    ));
    // H29: a selected nonterminal if-block must resume the outer suffix.
    // Occupancy [0,1)+[1,3)+[3,6)=6; the else's 4-minute call is not run.
    let mut b = Builder::new("H29", 1, 6);
    let a = b.call(1, 0, &[0], unspecified);
    let yes = b.call(2, 0, &[0], unspecified);
    let no = b.call(4, 0, &[0], unspecified);
    let tail = b.call(3, 0, &[0], unspecified);
    cases.push(case(
        "H29",
        b.finish(vec![
            stmt(Stmt::Call(a)),
            branch(
                eq(0, 0),
                vec![stmt(Stmt::Call(yes))],
                vec![stmt(Stmt::Call(no))],
            ),
            stmt(Stmt::Call(tail)),
            end(0),
        ]),
        vec![point(0, 6, 6, None)],
    ));
    // H30: one result deadlocks at1, another exceeds clock1. Deadlock
    // still forces overall violated; uncompleted paths yield no terminal.
    let mut b = Builder::new("H30", 1, 1);
    let a = b.call(1, 0, &[0, 1], unspecified);
    let long = b.call(2, 0, &[0], unspecified);
    // Like H13, only the declared done end is needed; no fake no_match suffix.
    b.0.ends.truncate(1);
    let mut c = case(
        "H30",
        b.finish(vec![
            stmt(Stmt::Call(a)),
            branch(eq(0, 0), vec![], vec![stmt(Stmt::Call(long)), end(0)]),
        ]),
        vec![],
    );
    c.truncated = true;
    c.properties = vec![Inconclusive; 2];
    c.overall = Violated;
    c.deadlock = true;
    cases.push(c);
    // H31: independent extrema. After serial1, choice A runs (8,8) in
    // parallel: e9/w17/r9. B runs (1,10): e11/w12/r2. No single path
    // supplies all minima (or all maxima): e9..11/w12..17/r2..9.
    let mut b = Builder::new("H31", 2, 11);
    let a = b.call(1, 0, &[0, 1], unspecified);
    let first = pair_any(&mut b, [8, 8], forbidden, drain);
    let second = pair_any(&mut b, [1, 10], forbidden, drain);
    cases.push(case(
        "H31",
        b.finish(vec![
            stmt(Stmt::Call(a)),
            branch(eq(0, 0), vec![first, end(0)], vec![second, end(0)]),
        ]),
        vec![terminal(0, (9, 11), (12, 17), Some((2, 9)))],
    ));
    assert_eq!(cases.len(), 31);
    cases
}

fn defined_ends(body: &Block, set: &mut BTreeSet<EndId>) {
    for st in body {
        match &st.kind {
            Stmt::Response(_, _) => panic!("legacy definition oracle does not cover retry ends"),
            Stmt::End(e) => {
                set.insert(*e);
            }
            Stmt::If {
                then_body,
                else_body,
                ..
            } => {
                defined_ends(then_body, set);
                defined_ends(else_body, set);
            }
            _ => {}
        }
    }
}

fn codes(diagnostics: &[Diagnostic]) -> BTreeSet<&str> {
    for d in diagnostics {
        assert!(
            diag::CODES.contains(&d.code.as_str()),
            "unknown code {}",
            d.code
        );
        let expected = if d.code.starts_with("W-") {
            diag::Severity::Warning
        } else {
            diag::Severity::Error
        };
        assert_eq!(d.severity, expected, "severity for {}", d.code);
    }
    diagnostics.iter().map(|d| d.code.as_str()).collect()
}
fn normalized(out: &Outcome) -> Vec<TerminalOutcome> {
    let mut terminals = out.terminals.clone();
    terminals.sort_by_key(|t| t.end);
    for pair in terminals.windows(2) {
        assert_ne!(pair[0].end, pair[1].end, "duplicate terminal");
    }
    for t in &terminals {
        for r in [Some(t.elapsed), Some(t.work), t.result_at]
            .into_iter()
            .flatten()
        {
            assert!(r.min <= r.max);
        }
    }
    terminals
}
fn oracle(c: &Case, r: &ReferenceRun) {
    assert_eq!(r.stop, StopReason::Complete, "{} reference abort", c.id);
    assert_eq!(normalized(&r.outcome), c.terminals, "{} hand metrics", c.id);
    assert_eq!(r.truncated(), Some(c.truncated), "{} hand cut", c.id);
    assert_eq!(
        r.outcome
            .properties
            .iter()
            .map(|p| (property_name(&c.model, p.property), p.verdict))
            .collect::<Vec<_>>(),
        c.model
            .verify
            .iter()
            .zip(&c.properties)
            .map(|(request, &verdict)| (property_name(&c.model, request.property), verdict))
            .collect::<Vec<_>>(),
        "{} hand properties",
        c.id
    );
    assert_eq!(r.verdict(), c.overall, "{} hand fold", c.id);
    let mut expected_codes = if c.deadlock {
        BTreeSet::from([diag::W_CUT_0, diag::W_DEADLOCK_0])
    } else {
        BTreeSet::from([diag::W_CUT_0])
    };
    // Builder::finish appends missing end declarations as a dead suffix.
    // Keep a literal hand classification: never infer warnings from outcomes.
    if !c.truncated {
        let unreachable: &[&str] = match c.id {
            "H03" | "H12" | "H13" => &[],
            "H11" | "H14" => &[diag::W_UNREACH_0, diag::W_UNREACH_2],
            "H22" | "H24" | "H25" | "H26" => &[diag::W_UNREACH_1, diag::W_UNREACH_2],
            "H29" => &[diag::W_UNREACH_0, diag::W_UNREACH_1, diag::W_UNREACH_2],
            "H01" | "H02" | "H04" | "H05" | "H06" | "H10" | "H15" | "H16" | "H17" | "H19"
            | "H20" | "H21" | "H27" | "H28" | "H31" => &[diag::W_UNREACH_2],
            _ => panic!("unclassified handwritten warning {}", c.id),
        };
        expected_codes.extend(unreachable);
    }
    assert_eq!(
        r.outcome.unchecked,
        if c.truncated {
            vec!["unreachable"]
        } else {
            vec![]
        }
    );
    assert_eq!(
        codes(&r.outcome.diagnostics),
        expected_codes,
        "{} hand diagnostics",
        c.id
    );
}

/// Normal-corpus differential entry point. Intentional abort tests below also
/// use the public engine API, separately from this complete-run denominator.
/// Differences are accumulated so a failing first input cannot silently hide
/// the rest of the corpus denominator.
fn compare(id: &str, model: &Model, source: &str, r: &ReferenceRun) -> Vec<String> {
    compare_selected(id, model, source, None, r)
}

fn compare_selected(
    id: &str,
    model: &Model,
    source: &str,
    configuration: Option<&str>,
    r: &ReferenceRun,
) -> Vec<String> {
    assert_same_selected_model(id, model, source, configuration);
    let engine = relca_core::engine::explore(model);
    let report = check_selected(source, configuration);
    let mut errors = Vec::new();
    // Check each side against the REQUEST, not merely against one another.
    for out in [&r.outcome, &engine] {
        assert_eq!(
            out.properties.len(),
            model.verify.len(),
            "{id}: property count"
        );
        let mut ids = BTreeSet::new();
        for (p, request) in out.properties.iter().zip(&model.verify) {
            assert_eq!(p.property, request.property, "{id}: property id/order");
            assert!(
                ids.insert(format!("{:?}", p.property)),
                "{id}: duplicate property"
            );
        }
        for t in &out.terminals {
            assert!(t.end.0 < model.ends.len(), "{id}: unknown EndId");
        }
    }
    macro_rules! same {
        ($field:expr,$left:expr,$right:expr) => {
            if $left != $right {
                errors.push(format!(
                    "{id} {}: reference={:?}, engine={:?}",
                    $field, $left, $right
                ));
            }
        };
    }
    same!("properties", r.outcome.properties, engine.properties);
    same!("terminals", normalized(&r.outcome), normalized(&engine));
    same!("truncated", r.outcome.truncated, engine.truncated);
    // External warnings and runtime diagnostics are checked at BOTH seams.
    // Remove the exact copied model prefix, not a set of codes: a runtime code
    // equal to a model code must not disappear. No one-sided code is filtered.
    assert!(r.outcome.diagnostics.starts_with(&model.diagnostics));
    let runtime = &r.outcome.diagnostics[model.diagnostics.len()..];
    same!(
        "runtime diagnostics",
        codes(runtime),
        codes(&engine.diagnostics)
    );
    same!(
        "diagnostics",
        codes(&r.outcome.diagnostics),
        codes(&report.diagnostics)
    );
    if matches!(id, "H11" | "H12" | "H13") {
        for ds in [runtime, engine.diagnostics.as_slice()] {
            if !codes(ds).contains(diag::W_DEADLOCK_0) {
                errors.push(format!("{id}: required runtime W-DEADLOCK-0 absent"));
            }
        }
    }
    same!("overall", r.verdict(), report.verdict);
    same!("wire truncated", r.truncated(), report.bounds.truncated);
    let wire = serde_json::to_value(&report).unwrap();
    let expected_properties: Vec<_> = r
        .outcome
        .properties
        .iter()
        .map(|p| serde_json::json!({"id": property_name(model, p.property), "verdict": p.verdict}))
        .collect();
    same!(
        "wire properties",
        serde_json::json!(expected_properties),
        wire["properties"]
    );
    let mut wire_terminals = wire["terminals"].as_array().unwrap().clone();
    wire_terminals.sort_by_key(|t| t["end"].as_str().unwrap().to_owned());
    let mut expected_terminals: Vec<_> = r
        .outcome
        .terminals
        .iter()
        .map(|t| {
            serde_json::json!({"end": model.ends[t.end.0],
            "elapsed": {"min": t.elapsed.min, "max": t.elapsed.max},
            "work": {"min": t.work.min, "max": t.work.max},
            "result_at": t.result_at.map(|r| serde_json::json!({"min": r.min, "max": r.max}))})
        })
        .collect();
    expected_terminals.sort_by_key(|t| t["end"].as_str().unwrap().to_owned());
    same!("wire terminals", expected_terminals, wire_terminals);
    if codes(&engine.diagnostics).contains(diag::W_BUDGET_0)
        || codes(&engine.diagnostics).contains(diag::U_SEM_0)
    {
        errors.push(format!("{id}: unexpected engine budget/semantic abort"));
    }
    if !errors.is_empty() {
        errors.push(format!("{id} input bounds={:?}\nsource={source}\nIR={model:#?}\nreference={r:#?}\nengine={engine:#?}",model.bounds));
    }
    errors
}

fn property_name(model: &Model, p: Property) -> String {
    match p {
        Property::ProbabilityReached(_) => {
            unreachable!("legacy population has no probability queries")
        }
        Property::NoResourceOvercommit => "safety no_resource_overcommit".into(),
        Property::ReachableEnd(e) => format!("reachable end.{}", model.ends[e.0]),
        Property::Contract(_) => unreachable!("legacy population has no contracts"),
        Property::Contest(_) => unreachable!(),
        // ADR 0025 completion properties are not in the legacy population.
        _ => unreachable!(),
    }
}

/// Compare all executable fields, including lexical slots, complete control
/// trees (even unreachable suffixes), enum member order, Cut and verify order.
/// Provenance-only spans, explanatory diagnostic prose and the non-executable
/// default-assumption descriptions are not an execution-model difference.
/// These three exclusions do not compare or certify assumptions
/// to contain only expected defaults;
/// all executable fields and diagnostic codes/severities still compare exactly.
fn model_identity(mut m: Model) -> Model {
    // ADR 0034: the handwritten IR corpus does not model agency viewpoints;
    // they are derived output, compared by the agency_viewpoints acceptance.
    m.agency.viewpoints = None;
    fn block(body: &mut Block) {
        for s in body {
            s.span = SPAN;
            match &mut s.kind {
                Stmt::WaitUntil(_) => {
                    panic!("differential renaming walker does not handle condition waits")
                }
                Stmt::Response(c, r) => {
                    c.span = SPAN;
                    r.command_call.span = SPAN;
                    r.retry_span = r.retry_span.map(|_| SPAN);
                }
                Stmt::Call(c) => c.span = SPAN,
                Stmt::Wait(_) => {}
                Stmt::Fork { branches, .. } => {
                    for c in branches {
                        c.span = SPAN;
                    }
                }
                Stmt::If {
                    then_body,
                    else_body,
                    ..
                } => {
                    block(then_body);
                    block(else_body);
                }
                Stmt::End(_) => {}
            }
        }
    }
    for t in &mut m.types {
        t.span = SPAN;
    }
    for a in &mut m.actors {
        a.span = SPAN;
    }
    for c in &mut m.capabilities {
        c.span = SPAN;
        for p in &mut c.params {
            p.span = SPAN;
        }
    }
    m.process.span = SPAN;
    for b in &mut m.process.bindings {
        b.span = SPAN;
    }
    block(&mut m.process.body);
    for v in &mut m.verify {
        v.span = SPAN;
    }
    m.assumptions.clear();
    for d in &mut m.diagnostics {
        d.span = None;
        d.message.clear();
    }
    m
}

fn assert_same_model(id: &str, model: &Model, source: &str) {
    assert_same_selected_model(id, model, source, None);
}

fn lower_selected(source: &str, configuration: Option<&str>) -> Result<Model, Vec<Diagnostic>> {
    relca_core::syntax::parse(source).and_then(|a| match configuration {
        Some(name) => relca_core::lower::lower_configuration(&a, name).map(|s| s.model),
        None => relca_core::lower::lower(&a),
    })
}

fn check_selected(source: &str, configuration: Option<&str>) -> relca_core::report::Report {
    match configuration {
        Some(name) => relca_core::report::check_configuration(source, name),
        None => relca_core::report::check(source),
    }
}

fn assert_same_selected_model(id: &str, model: &Model, source: &str, configuration: Option<&str>) {
    // For configuration fixtures both `model` and `bridge` come from calling
    // lower_selected with the same selector: this IR identity check is therefore
    // tautological, not an independent test of report's internal selection.
    // report uses the same selection function by construction; tests observe
    // only agreement of the resulting reports (compare_selected). Handwritten
    // IR and the wrong-selection guard still provide non-tautological checks.
    let bridge = lower_selected(source, configuration)
        .unwrap_or_else(|ds| panic!("{id} report bridge rejected: {ds:?}\n{source}"));
    let mut expected = model.clone();
    // ADR 0043 (plan C03/C04) for the handwritten, viewpoint-free IR summary
    // (empty_agency): executors leaves unchecked only when the bridge's effects
    // viewpoint is derived (rest unchanged), and a model without people gets
    // responsibility_people_undeclared appended last.
    if expected.agency.viewpoints.is_none() {
        let effects = serde_json::to_value(&bridge.agency.viewpoints).unwrap();
        if effects["effects"]["status"] == "derived" {
            expected.agency.unchecked.retain(|x| x != "executors");
        }
        if expected.people.is_empty()
            && !expected
                .agency
                .unchecked
                .iter()
                .any(|x| x == "responsibility_people_undeclared")
        {
            expected
                .agency
                .unchecked
                .push("responsibility_people_undeclared".into());
        }
    }
    assert_eq!(
        model_identity(expected),
        model_identity(bridge),
        "{id}: report bridge model mismatch"
    );
}

#[derive(serde::Deserialize)]
#[serde(tag = "command", rename_all = "snake_case", deny_unknown_fields)]
enum FixtureInvocation {
    Check { configuration: String },
    Diff { from: String, to: String },
}

fn fixture_selections<'a>(
    id: &str,
    invocation: Option<FixtureInvocation>,
    expected: &'a serde_json::Value,
) -> Vec<(Option<String>, &'a serde_json::Value)> {
    match invocation {
        Some(FixtureInvocation::Check { configuration }) => vec![(Some(configuration), expected)],
        Some(FixtureInvocation::Diff { from, to }) => {
            for (side, name) in [("from", &from), ("to", &to)] {
                assert_eq!(
                    expected[side]["configuration"].as_str(),
                    Some(name.as_str()),
                    "{id}: expected {side} configuration must match invocation"
                );
            }
            vec![
                (Some(from), &expected["from"]["report"]),
                (Some(to), &expected["to"]["report"]),
            ]
        }
        // No selector is NOT an invitation to choose the first process.
        None => vec![(None, expected)],
    }
}

#[test]
fn configuration_check_invocation() {
    // The on-disk corpus currently has diff invocations and one default check.
    // Exercise explicit check selection too, including the non-first process.
    let source = include_str!("../../../fixtures/config-diff-parallel/model.relca");
    for name in ["AsIs", "ToBeP"] {
        let invocation = serde_json::from_value(serde_json::json!({
            "command": "check", "configuration": name
        }))
        .unwrap();
        let expected = serde_json::json!({"verdict": "satisfied"});
        let selections = fixture_selections("explicit check", Some(invocation), &expected);
        assert_eq!(selections.len(), 1);
        let (configuration, expected_report) = &selections[0];
        assert_eq!(configuration.as_deref(), Some(name));
        assert_eq!(*expected_report, &expected);
        let model = lower_selected(source, configuration.as_deref()).unwrap();
        let r = reference::explore(&model);
        configuration_fixture_oracle("config-diff-parallel", name, &model, &r);
        let errors = compare_selected(name, &model, source, configuration.as_deref(), &r);
        assert!(errors.is_empty(), "{}", errors.join("\n"));
    }
}

#[test]
#[should_panic(expected = "report bridge model mismatch")]
fn configuration_bridge_guard_rejects_wrong_selection() {
    let source = include_str!("../../../fixtures/config-diff-parallel/model.relca");
    let model = lower_selected(source, Some("AsIs")).unwrap();
    assert_same_selected_model("wrong selection", &model, source, Some("ToBeP"));
}

#[test]
fn configuration_oracle_guards_properties_and_diagnostics() {
    let source = include_str!("../../../fixtures/config-diff-terminal-union/model.relca");
    let id = "config-diff-terminal-union";
    for name in ["AsIs", "ToBe"] {
        let model = lower_selected(source, Some(name)).unwrap();
        let r = reference::explore(&model);
        configuration_fixture_oracle(id, name, &model, &r);
        // The overall remains violated: checking only that fold would miss
        // losing either the reachable witness or the satisfied safety result.
        for index in [usize::from(name == "ToBe"), 2] {
            let mut changed = r.clone();
            changed.outcome.properties[index].verdict = Violated;
            assert_eq!(changed.verdict(), r.verdict());
            assert!(
                std::panic::catch_unwind(|| {
                    configuration_fixture_oracle(id, name, &model, &changed);
                })
                .is_err()
            );
        }
        let mut changed = r.clone();
        changed.outcome.diagnostics.push(Diagnostic {
            code: diag::W_CUT_0.into(),
            severity: diag::Severity::Warning,
            message: "injected spurious warning".into(),
            span: None,
        });
        assert_eq!(changed.verdict(), r.verdict());
        assert!(
            std::panic::catch_unwind(|| {
                configuration_fixture_oracle(id, name, &model, &changed);
            })
            .is_err()
        );
    }
}

#[test]
fn differential_corpus() {
    run_corpus(Limits::default(), true);
}

#[test]
fn agency_population_has_separate_hand_runtime_oracles_and_no_reference_exclusions() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut directories: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("agency-")
        })
        .collect();
    directories.sort();
    assert_eq!(
        directories
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        AGENCY_FIXTURES
    );
    let mut compared = 0;
    let mut errors = Vec::new();
    for directory in directories {
        let id = directory.file_name().unwrap().to_str().unwrap();
        let source = fs::read_to_string(directory.join("model.relca")).unwrap();
        let selections: &[Option<&str>] = if id.starts_with("agency-refusal") {
            &[Some("AsIs"), Some("ToBe")]
        } else {
            &[None]
        };
        for &selection in selections {
            let label = format!("{id}[{selection:?}]");
            let model =
                lower_selected(&source, selection).unwrap_or_else(|d| panic!("{label}: {d:?}"));
            let r = reference::explore(&model);
            assert_eq!(
                r.stop,
                StopReason::Complete,
                "{label}: no reference exclusions"
            );
            let engine = relca_core::engine::explore(&model);
            // Independent timing oracle copied from arithmetic in calc.md,
            // not expected.json nor either interpreter's generated results.
            let tuples: Vec<(&str, u64, u64, Option<u64>)> = if id.starts_with("agency-lineage") {
                vec![("rejected", 4, 4, None), ("done", 5, 5, None)]
            } else if id.starts_with("agency-origin") {
                vec![("rejected", 2, 2, None), ("done", 3, 3, None)]
            } else if id == "agency-any" {
                vec![("done", 3, 4, Some(2)), ("rejected", 2, 3, None)]
            } else if id == "agency-any.broken-single-valued-a" {
                vec![("done", 3, 4, Some(2))]
            } else if id == "agency-refusal.broken-cut-guard" && selection == Some("ToBe") {
                vec![("done", 4, 4, None)]
            } else {
                vec![("rejected", 3, 3, None), ("done", 4, 4, None)]
            };
            let mut expected = tuples
                .iter()
                .map(|(name, e, w, a)| {
                    point(
                        model
                            .ends
                            .iter()
                            .position(|n| n == name)
                            .expect("hand terminal name"),
                        *e,
                        *w,
                        *a,
                    )
                })
                .collect::<Vec<_>>();
            expected.sort_by_key(|t| t.end);
            for outcome in [&r.outcome, &engine] {
                assert_eq!(
                    normalized(outcome),
                    expected,
                    "{label}: interpreter vs hand oracle"
                );
                assert!(!outcome.truncated, "{label}");
                assert_eq!(outcome.properties.len(), 2);
                assert!(outcome.properties.iter().all(|p| p.verdict == Satisfied));
            }
            assert_eq!(r.verdict(), Satisfied);
            let mut expected_codes = BTreeSet::new();
            if selection.is_none() {
                expected_codes.insert("W-CUT-0");
            }
            if id == "agency-any" || (id == "agency-refusal" && selection == Some("ToBe")) {
                expected_codes.insert("W-AGY-1");
            }
            if id == "agency-any.broken-single-valued-a"
                || (id == "agency-refusal.broken-cut-guard" && selection == Some("ToBe"))
            {
                expected_codes.insert("W-AGY-0");
                // These faults remove every refusal path: rejected remains
                // lexical (if_none or suffix), but no business end reaches it.
                expected_codes.insert("W-UNREACH-2");
                if id == "agency-refusal.broken-cut-guard" {
                    expected_codes.insert("W-UNREACH-1");
                }
            }
            // ADR 0054 "範囲" / "意味": only independently derived U/E
            // carriers may extend this closed code set; require full rows too.
            let responsibility = responsibility_oracle::expected(&model);
            assert_eq!(
                r.outcome
                    .diagnostics
                    .iter()
                    .filter(|d| d.code == diag::W_RESP_0)
                    .collect::<Vec<_>>(),
                responsibility.iter().collect::<Vec<_>>(),
                "{label}: independent responsibility candidates"
            );
            if !responsibility.is_empty() {
                expected_codes.insert(diag::W_RESP_0);
            }
            assert_eq!(
                codes(&r.outcome.diagnostics),
                expected_codes,
                "{label}: exact warning prefix and runtime codes"
            );
            errors.extend(compare_selected(&label, &model, &source, selection, &r));
            if selection.is_none() {
                let rendered = render(&model);
                assert_same_model(&label, &model, &rendered);
            }
            compared += 1;
        }
    }
    assert_eq!(
        compared, 11,
        "9 Agency directories / 11 executable inputs, separate from old 28/39"
    );
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

fn run_corpus(reference_limits: Limits, fixtures: bool) {
    let started = Instant::now();
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut dirs = fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect::<Vec<_>>();
    dirs.sort();
    // New Agency inputs have their own fixed denominator/oracles below; do
    // not change the old 28 directories / 39 inputs / 31 accepted / 8 rejected.
    dirs.retain(|p| {
        let id = p.file_name().unwrap().to_str().unwrap();
        if scenario_population::fixture(id)
            || (include!("support/waits_names.rs") as &[&str]).contains(&id)
        {
            return false;
        }
        if sinv_population::NAMES.contains(&id) {
            return false;
        }
        if conflict_population::fixture(id)
            || authority_population::fixture(id)
            || aobs_population::fixture(id)
            || serde_json::from_str::<Vec<String>>(include_str!("support/caseset_names.json"))
                .unwrap()
                .iter()
                .any(|n| n == id)
            || resp2_population::fixture(id)
            || pcase_population::fixture(id)
            || integer_population::fixture(id)
            || diff2_population::fixture(id)
            || resource_population::fixture(id)
            || agency_viewpoints_population::fixture(id)
            || policy_population::fixture(id)
            || limits_population::fixture(id)
            || effect_population::fixture(id)
            || record_population::fixture(id)
            || mutation_default_population::fixture(id)
            || include!("support/quality_names.rs").contains(&id)
            || serde_json::from_str::<Vec<String>>(include_str!("support/rule_names.json"))
                .unwrap()
                .iter()
                .any(|n| n == id)
            || include!("support/evaluation_names.rs").contains(&id)
            || include!("support/quantile_names.rs").contains(&id)
            || include!("support/acceptance_gap_names.rs").contains(&id)
        {
            return false;
        }
        // ADR 0025 has its own independently enumerated 24-dir population.
        const COMPLETION_FIXTURES: [&str; 24] = [
            "done-arrivals",
            "done-arrivals.broken-arrival-after-clock",
            "done-branches",
            "done-branches.broken-enable-then",
            "done-call-kind",
            "done-call-kind.broken-call-unused",
            "done-clock",
            "done-clock.broken-short-clock",
            "done-end-vs-close",
            "done-end-vs-close.broken-pre-end-delay",
            "done-nondet",
            "done-nondet.broken-nondet-option",
            "done-people",
            "done-people.broken-remove-executor",
            "done-probability",
            "done-probability.broken-block-failure",
            "done-recovery",
            "done-recovery.broken-zero-capacity",
            "done-support",
            "done-support.broken-positive-support",
            "done-vacuity",
            "done-vacuity.broken-enable-registration",
            "done-within",
            "done-within.broken-tight-deadline",
        ];
        assert!(
            !id.starts_with("done-") || COMPLETION_FIXTURES.contains(&id),
            "unclassified completion fixture {id}"
        );
        if COMPLETION_FIXTURES.contains(&id) {
            return false;
        }
        // ADR 0023 has an independent 28-dir acceptance population.
        const ACTOR_FIXTURES: [&str; 28] = [
            "act-arrival-units",
            "act-arrival-units.broken-later-first",
            "act-composite",
            "act-composite.broken-team-work",
            "act-control",
            "act-control.broken-explicit-environment",
            "act-cut-context",
            "act-cut-context.broken-no-cut",
            "act-debtor",
            "act-debtor-actor",
            "act-debtor-actor.broken-omitted",
            "act-debtor-value",
            "act-debtor-value.broken-other-person",
            "act-debtor.broken-unknown",
            "act-defaults",
            "act-defaults.broken-explicit-boundary",
            "act-grain",
            "act-grain.broken-swapped",
            "act-horizon",
            "act-horizon.broken-short-horizon",
            "act-legacy",
            "act-legacy.broken-longer-task",
            "act-overlap",
            "act-overlap.broken-relation",
            "act-units",
            "act-units.broken-duration-day",
            "act-via",
            "act-via.broken-upstream-only",
        ];
        if ACTOR_FIXTURES.contains(&id) {
            return false;
        }
        if lin_population::fixture(id) || req_population::fixture(id) {
            return false;
        }
        // D1 has its own 12/9/3 oracle; retain old 28/39/31/8 unchanged.
        const D1: [&str; 24] = [
            "ctx-excludes",
            "ctx-excludes.broken-start",
            "ctx-nonwaivable",
            "ctx-nonwaivable.broken-waive",
            "ctx-obligation",
            "ctx-obligation.broken-excludes-target",
            "ctx-waive",
            "ctx-waive-authority",
            "ctx-waive-authority.broken-no-authority",
            "ctx-waive-names",
            "ctx-waive-names.broken-unknown-response",
            "ctx-waive.broken-after-deadline",
            "ctx-contest",
            "ctx-contest-branches",
            "ctx-contest-branches.broken-zero-capacity",
            "ctx-contest-clock",
            "ctx-contest-clock.broken-short-clock",
            "ctx-contest-metrics",
            "ctx-contest-metrics.broken-missing-route-rate",
            "ctx-contest-origin",
            "ctx-contest-origin.broken-arrival-clock",
            "ctx-contest-resource",
            "ctx-contest-resource.broken-slow-route",
            "ctx-contest.broken-late",
        ];
        if D1.contains(&id) {
            return false;
        }
        if id.starts_with("explain-") {
            const EXPLAIN_FIXTURES: [&str; 12] = [
                "explain-deadlock",
                "explain-deadlock.broken-zero-capacity",
                "explain-executors",
                "explain-executors.broken-shared-worker",
                "explain-invariant",
                "explain-invariant.broken-cut-guard",
                "explain-once",
                "explain-once.broken-repeat-result",
                "explain-requires",
                "explain-requires.broken-cut-guard",
                "explain-response",
                "explain-response.broken-late-finish",
            ];
            assert!(
                EXPLAIN_FIXTURES.contains(&id),
                "unclassified explain fixture {id}"
            );
            return false;
        }
        if id.starts_with("mut-") {
            const MUTATION_FIXTURES: [&str; 12] = [
                "mut-empty",
                "mut-empty.broken-invalid",
                "mut-evidence",
                "mut-evidence.broken-no-independent-read",
                "mut-gate",
                "mut-gate.broken-unsupported",
                "mut-guard",
                "mut-guard.broken-no-contract",
                "mut-limit",
                "mut-limit.broken-many-sites",
                "mut-origin",
                "mut-origin.broken-route-only",
            ];
            assert!(
                MUTATION_FIXTURES.contains(&id),
                "unclassified mutation fixture {id}"
            );
            return false;
        }
        if id.starts_with("resp-") {
            const RESPONSIBILITY_FIXTURES: [&str; 19] = [
                "resp-contest",
                "resp-contest.broken-no-authority",
                "resp-contest.broken-no-permission",
                "resp-contest.broken-route-excluded",
                "resp-contest.broken-zero-deadline",
                "resp-cut",
                "resp-cut.broken-no-explicit-cut",
                "resp-diff",
                "resp-diff.broken-no-authorized-decision",
                "resp-missing-duty",
                "resp-missing-duty.broken-no-duty",
                "resp-origin",
                "resp-origin.broken-no-direct-read",
                "resp-refusal",
                "resp-refusal.broken-cut-guard",
                "resp-standing",
                "resp-standing.broken-not-party",
                "resp-state",
                "resp-state.broken-no-direct-read",
            ];
            assert!(
                RESPONSIBILITY_FIXTURES.contains(&id),
                "unclassified responsibility fixture {id}"
            );
            false
        } else if id.starts_with("agency-") {
            assert!(
                AGENCY_FIXTURES.contains(&id),
                "unclassified Agency fixture {id}"
            );
            false
        } else if id.starts_with("invoice-phase0-") {
            assert!(
                INVOICE_FIXTURES.contains(&id),
                "unclassified invoice fixture {id}"
            );
            false
        } else if id.starts_with("contract-") {
            assert!(
                CONTRACT_FIXTURES.contains(&id),
                "unclassified contract fixture {id}"
            );
            false
        } else if id.starts_with("exec-") {
            assert!(
                EXECUTOR_FIXTURES.contains(&id),
                "unclassified executor fixture {id}"
            );
            false
        } else if id.starts_with("state-") {
            assert!(
                STATE_FIXTURES.contains(&id),
                "unclassified state fixture {id}"
            );
            false
        } else if id.starts_with("compare-") {
            const COMPARE_FIXTURES: [&str; 16] = [
                "compare-arrivals",
                "compare-arrivals.broken-slow-plan",
                "compare-case-diff",
                "compare-case-diff.broken-same-kernel",
                "compare-cut",
                "compare-cut.broken-other-cut",
                "compare-legacy",
                "compare-legacy.broken-longer-task",
                "compare-random",
                "compare-random.broken-alias-process",
                "compare-reject",
                "compare-reject.broken-zero-queue",
                "compare-terminal",
                "compare-terminal.broken-other-end",
                "compare-unlock",
                "compare-unlock.broken-excluded-call",
            ];
            assert!(
                COMPARE_FIXTURES.contains(&id),
                "unclassified compare fixture {id}"
            );
            false
        } else if id.starts_with("met-") {
            const METRICS_FIXTURES: [&str; 24] = [
                "met-cancel",
                "met-cancel.broken-drain",
                "met-compare",
                "met-compare.broken-longer-task",
                "met-diff",
                "met-diff.broken-free-start",
                "met-exclusions",
                "met-exclusions.broken-larger-queue",
                "met-fifo",
                "met-fifo.broken-shorter-second-step",
                "met-legacy",
                "met-legacy.broken-enabled",
                "met-missing-rate",
                "met-missing-rate.broken-free-rate",
                "met-parallel",
                "met-parallel.broken-one-slot",
                "met-person",
                "met-person.broken-shared-person",
                "met-positive-support",
                "met-positive-support.broken-positive-early-time",
                "met-sample",
                "met-sample.broken-short-clock",
                "met-serial",
                "met-serial.broken-longer-task",
            ];
            assert!(
                METRICS_FIXTURES.contains(&id),
                "unclassified metrics fixture {id}"
            );
            false
        } else if id.starts_with("sim-") {
            const SIM_FIXTURES: [&str; 24] = [
                "sim-actor-initial",
                "sim-actor-initial.broken-missing-distribution",
                "sim-contract",
                "sim-contract.broken-short-response",
                "sim-cut",
                "sim-cut.broken-short-clock",
                "sim-deadlock",
                "sim-deadlock.broken-zero-capacity",
                "sim-fifo",
                "sim-fifo.broken-no-policy",
                "sim-hazard",
                "sim-hazard.broken-short-clock",
                "sim-keys",
                "sim-keys.broken-flipped-query",
                "sim-range",
                "sim-range.broken-zero-replications",
                "sim-reachable",
                "sim-reachable.broken-unseen-end",
                "sim-rejection",
                "sim-rejection.broken-positive-queue",
                "sim-required",
                "sim-required.broken-missing-simulation",
                "sim-statistical",
                "sim-statistical.broken-tight-threshold",
            ];
            assert!(
                SIM_FIXTURES.contains(&id),
                "unclassified simulation fixture {id}"
            );
            false
        } else if id.starts_with("arrivals-") {
            const ARRIVALS_FIXTURES: [&str; 24] = [
                "arrivals-contracts",
                "arrivals-contracts.broken-short-response",
                "arrivals-count",
                "arrivals-count.broken-case-count",
                "arrivals-initial-product",
                "arrivals-initial-product.broken-fixed-initial",
                "arrivals-order",
                "arrivals-order.broken-descending",
                "arrivals-reject-mass",
                "arrivals-reject-mass.broken-larger-queue",
                "arrivals-relative-within",
                "arrivals-relative-within.broken-tight-deadline",
                "arrivals-required",
                "arrivals-required.broken-missing-arrivals",
                "arrivals-same-time-slot",
                "arrivals-same-time-slot.broken-later-arrival",
                "arrivals-shared-fixed",
                "arrivals-shared-fixed.broken-unset-shared",
                "arrivals-simultaneous",
                "arrivals-simultaneous.broken-no-policy",
                "arrivals-staggered",
                "arrivals-staggered.broken-extra-capacity",
                "arrivals-zero-queue",
                "arrivals-zero-queue.broken-positive-queue",
            ];
            assert!(
                ARRIVALS_FIXTURES.contains(&id),
                "unclassified arrivals fixture {id}"
            );
            false
        } else if id.starts_with("duration-") {
            const DURATION_FIXTURES: [&str; 18] = [
                "duration-cancel",
                "duration-cancel.broken-ignore-cancel",
                "duration-clock",
                "duration-clock.broken-short-clock",
                "duration-contention",
                "duration-contention.broken-extra-capacity",
                "duration-duplicate",
                "duration-duplicate.broken-duplicate-value",
                "duration-no-lookahead",
                "duration-no-lookahead.broken-late-choice",
                "duration-parallel",
                "duration-parallel.broken-tight-upper",
                "duration-serial",
                "duration-serial.broken-tight-upper",
                "duration-sum",
                "duration-sum.broken-mass",
                "duration-zero",
                "duration-zero.broken-late-positive",
            ];
            assert!(
                DURATION_FIXTURES.contains(&id),
                "unclassified duration fixture {id}"
            );
            false
        } else if id.starts_with("prob-") {
            const PROB_FIXTURES: [&str; 16] = [
                "prob-cancel",
                "prob-cancel.broken-ignore-cancel",
                "prob-cut",
                "prob-cut.broken-short-clock",
                "prob-finish-read",
                "prob-finish-read.broken-late-writer",
                "prob-invoice",
                "prob-invoice.broken-ai-transfer",
                "prob-mdp",
                "prob-mdp.broken-tight-upper",
                "prob-rows",
                "prob-rows.broken-missing-difficult",
                "prob-sum",
                "prob-sum.broken-mass",
                "prob-zero",
                "prob-zero.broken-positive-write",
            ];
            assert!(
                PROB_FIXTURES.contains(&id),
                "unclassified probability fixture {id}"
            );
            false
        } else {
            true
        }
    });
    if !fixtures {
        dirs.clear();
    }
    let fixture_count = dirs.len();
    let mut fixture_inputs = 0;
    let mut configuration_counts = (0, 0);
    let mut rejected = 0;
    let mut accepted = 0;
    let mut compared = 0;
    let mut passed = 0;
    let mut limits = Vec::new();
    let mut semantic = Vec::new();
    let mut errors = Vec::new();
    for dir in dirs {
        let id = dir.file_name().unwrap().to_str().unwrap();
        for file in ["model.relca", "calc.md", "expected.json"] {
            assert!(dir.join(file).is_file(), "{id}: missing {file}");
        }
        let source = fs::read_to_string(dir.join("model.relca")).unwrap();
        let invocation_path = dir.join("invocation.json");
        let has_configuration = relca_core::syntax::parse(&source).is_ok_and(|a| {
            a.items
                .iter()
                .any(|i| matches!(i, relca_core::ast::Item::Configuration(_)))
        });
        if invocation_path.is_file() || has_configuration {
            let expected: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(dir.join("expected.json")).unwrap())
                    .unwrap();
            let invocation = if invocation_path.is_file() {
                let invocation: FixtureInvocation =
                    serde_json::from_str(&fs::read_to_string(invocation_path).unwrap())
                        .unwrap_or_else(|e| panic!("{id}: invalid invocation: {e}"));
                Some(invocation)
            } else {
                None
            };
            let selections = fixture_selections(id, invocation, &expected);
            fixture_inputs += selections.len();
            for (configuration, expected_report) in selections {
                let label = format!("{id}[{}]", configuration.as_deref().unwrap_or("default"));
                let expected_verdict: Verdict =
                    serde_json::from_value(expected_report["verdict"].clone())
                        .unwrap_or_else(|e| panic!("{label}: unreadable expected verdict: {e}"));
                let selector = configuration.as_deref();
                match lower_selected(&source, selector) {
                    Err(ds) => {
                        rejected += 1;
                        configuration_counts.1 += 1;
                        assert!(
                            matches!(expected_verdict, Invalid | Unsupported),
                            "{label}: unexpected lower rejection {ds:?}"
                        );
                        // Read the SIDE's rejection, never the diff root's
                        // verdict (which may reject only intent or Cut matching).
                        let expected_diagnostics = expected_report["diagnostics"]
                            .as_array()
                            .unwrap_or_else(|| panic!("{label}: missing rejection diagnostics"));
                        let expected_codes: BTreeSet<_> = expected_diagnostics
                            .iter()
                            .map(|d| d["code"].as_str().expect("expected diagnostic code"))
                            .collect();
                        assert_eq!(codes(&ds), expected_codes, "{label}: lower rejection");
                        if selector.is_none() {
                            assert!(expected_codes.contains(diag::E_CONF_0), "{label}");
                        }
                        let report = check_selected(&source, selector);
                        assert_eq!(
                            report.verdict, expected_verdict,
                            "{label}: rejection verdict"
                        );
                        assert_eq!(
                            codes(&report.diagnostics),
                            expected_codes,
                            "{label}: report rejection"
                        );
                        assert!(report.properties.is_empty(), "{label}: rejected properties");
                        assert!(report.terminals.is_empty(), "{label}: rejected terminals");
                        assert_eq!(report.bounds.truncated, None, "{label}: rejected cut");
                    }
                    Ok(model) => {
                        accepted += 1;
                        configuration_counts.0 += 1;
                        if matches!(expected_verdict, Invalid | Unsupported) {
                            errors.push(format!(
                                "{label}: expected rejection but lowered; still comparing"
                            ));
                        }
                        let r = reference::explore_with_limits(&model, reference_limits);
                        match r.stop {
                            StopReason::EnumerationLimit => {
                                limits.push(label);
                                continue;
                            }
                            StopReason::SemanticAbort => {
                                semantic.push(label);
                                continue;
                            }
                            StopReason::Complete => {}
                        }
                        configuration_fixture_oracle(
                            id,
                            selector.expect("accepted selected fixture"),
                            &model,
                            &r,
                        );
                        assert_eq!(r.verdict(), expected_verdict, "{label}: side verdict");
                        let diffs = compare_selected(&label, &model, &source, selector, &r);
                        compared += 1;
                        if diffs.is_empty() {
                            passed += 1;
                        }
                        errors.extend(diffs);
                    }
                }
            }
            continue;
        }
        fixture_inputs += 1;
        let expected_rejection = match id {
            "parallel-time.broken-missing-join" => Some(diag::E_SYN_0),
            "serial-time.broken-missing-capacity" => Some(diag::E_CAP_0),
            "any-drain.broken-missing-cancel" => Some(diag::E_CANCEL_0),
            "cut-declared.broken-excluded-call" | "cut-declared.broken-missing-member" => {
                Some(diag::E_CUT_1)
            }
            _ => None,
        };
        let lowered = relca_core::syntax::parse(&source).and_then(|a| relca_core::lower::lower(&a));
        match lowered {
            Err(ds) => {
                rejected += 1;
                if let Some(code) = expected_rejection {
                    assert!(codes(&ds).contains(code), "{id}: expected {code}");
                } else {
                    errors.push(format!("{id}: unexpected parse/lower rejection {ds:?}"));
                }
                let report = relca_core::report::check(&source);
                assert_eq!(report.verdict, Invalid, "{id} static rejection verdict");
                // calc.md also requires external W-CUT-0 for undeclared Cut.
                // A parse error has no lowered Model whose warnings could be
                // compared to report; use the explicit external oracle instead.
                let code = expected_rejection.expect("classified rejected fixture");
                let expected = if id.starts_with("cut-declared.") {
                    BTreeSet::from([code])
                } else {
                    BTreeSet::from([code, diag::W_CUT_0])
                };
                assert_eq!(
                    codes(&report.diagnostics),
                    expected,
                    "{id} calc static diagnostic codes"
                );
                assert!(
                    report.properties.is_empty(),
                    "{id}: static rejection evaluates no properties"
                );
                assert_eq!(report.bounds.truncated, None, "{id}: static rejection cut");
                let wire = serde_json::to_value(report).unwrap();
                assert_eq!(wire["terminals"], serde_json::json!([]));
            }
            Ok(model) => {
                accepted += 1;
                if expected_rejection.is_some() {
                    errors.push(format!(
                        "{id}: expected rejection but lowered; still comparing"
                    ));
                }
                let r = reference::explore_with_limits(&model, reference_limits);
                match r.stop {
                    StopReason::EnumerationLimit => {
                        limits.push(id.to_string());
                        continue;
                    }
                    StopReason::SemanticAbort => {
                        semantic.push(id.to_string());
                        continue;
                    }
                    StopReason::Complete => {}
                }
                fixture_oracle(id, &model, &r);
                let diffs = compare(id, &model, &source, &r);
                compared += 1;
                if diffs.is_empty() {
                    passed += 1;
                }
                errors.extend(diffs);
            }
        }
    }
    let mut cases = handwritten();
    if !fixtures {
        cases.truncate(1);
    }
    for c in &cases {
        let r = reference::explore_with_limits(&c.model, reference_limits);
        match r.stop {
            StopReason::EnumerationLimit => {
                limits.push(c.id.into());
                continue;
            }
            StopReason::SemanticAbort => {
                semantic.push(c.id.into());
                continue;
            }
            StopReason::Complete => {}
        }
        oracle(c, &r);
        let source = render(&c.model);
        let diffs = compare(c.id, &c.model, &source, &r);
        compared += 1;
        if diffs.is_empty() {
            passed += 1;
        }
        errors.extend(diffs);
    }
    eprintln!(
        "fixture total={fixture_count}, inputs={fixture_inputs}, accepted={accepted}, rejected={rejected}, configuration accepted/rejected={configuration_counts:?}; handwritten={}; comparisons={compared}, passed={passed}, reference_limit_exclusions={}, unresolved_semantics_exclusions={}; duration={:?}",
        cases.len(),
        limits.len(),
        semantic.len(),
        started.elapsed()
    );
    assert_eq!(
        fixture_count,
        if fixtures { 28 } else { 0 },
        "update explicit fixture classification/oracles"
    );
    assert_eq!(
        (accepted, rejected),
        if fixtures { (31, 8) } else { (0, 0) }
    );
    assert_eq!(fixture_inputs, if fixtures { 39 } else { 0 });
    assert_eq!(accepted + rejected, fixture_inputs);
    assert_eq!(
        configuration_counts,
        if fixtures { (20, 3) } else { (0, 0) }
    );
    assert_eq!(
        compared + limits.len() + semantic.len(),
        accepted + cases.len()
    );
    assert_no_limit_exclusions(&limits);
    assert_eq!(semantic.len(), 0, "unexpected U-SEM-0: {semantic:?}");
    assert!(
        started.elapsed().as_secs() < 60,
        "debug corpus exceeded 60 seconds"
    );
    assert!(
        errors.is_empty(),
        "{} differences:\n{}",
        errors.len(),
        errors.join("\n")
    );
    assert_eq!(compared, passed, "differential comparison failures");
}

#[path = "support/agency_viewpoints_acceptance.rs"]
mod agency_viewpoints_population;

fn assert_no_limit_exclusions(limits: &[String]) {
    assert_eq!(limits.len(), 0, "reference limit exclusions: {limits:?}");
}

#[test]
#[should_panic(expected = "reference limit exclusions")]
fn reference_limit_exclusion_guard_rejects() {
    // The SAME corpus loop must classify the aborted run, count its exclusion,
    // and reject it at the final gate. Removing that gate makes this test fail.
    run_corpus(
        Limits {
            frames: 0,
            ..Limits::default()
        },
        false,
    );
}

fn fixture_oracle(id: &str, model: &Model, r: &ReferenceRun) {
    // Fixed from fixtures/*/calc.md, NOT expected.json or engine output.
    let (terminals, overall, cut) = match id {
        "serial-time"
        | "resource-contention"
        | "cut-declared"
        | "cut-declared.broken-missing-loses" => (vec![point(0, 8, 8, None)], Satisfied, false),
        "parallel-time" => (vec![point(0, 5, 8, None)], Satisfied, false),
        "any-drain" => (vec![point(0, 5, 8, Some(3))], Satisfied, false),
        "coop-cancel" => (vec![point(0, 4, 7, Some(3))], Satisfied, false),
        "coop-cancel.broken-short-clock" => (vec![], Inconclusive, true),
        "resource-contention.broken-zero-capacity" => (vec![], Violated, false),
        // End IDs follow lexical definitions: rejected, fast, slow_ok.
        "nondet-race" => (
            vec![
                point(0, 5, 8, None),
                terminal(1, (4, 5), (7, 8), Some((3, 3))),
                point(2, 5, 8, Some(5)),
            ],
            Satisfied,
            false,
        ),
        "nondet-race.broken-quick-never-passes" => (
            vec![point(0, 5, 8, None), point(2, 5, 8, Some(5))],
            Violated,
            false,
        ),
        _ => panic!("unclassified accepted fixture {id}"),
    };
    assert_eq!(normalized(&r.outcome), terminals, "{id} calc metrics");
    assert_eq!(r.verdict(), overall, "{id} calc verdict");
    assert_eq!(r.truncated(), Some(cut), "{id} calc cut");
    let expected_codes: BTreeSet<_> = match id {
        "cut-declared" => BTreeSet::new(),
        "cut-declared.broken-missing-loses" => BTreeSet::from([diag::W_CUT_1]),
        "resource-contention.broken-zero-capacity" => {
            // Both called capabilities are queued forever at capacity zero.
            BTreeSet::from([
                diag::W_CUT_0,
                diag::W_DEADLOCK_0,
                diag::W_UNREACH_0,
                diag::W_UNREACH_2,
            ])
        }
        "any-drain" | "coop-cancel" => {
            BTreeSet::from([diag::W_CUT_0, diag::W_UNREACH_1, diag::W_UNREACH_2])
        }
        // void is absent in both versions; the fault additionally removes pass
        // from quick. The code set stays the same, the number of arms differs.
        "nondet-race" => BTreeSet::from([diag::W_CUT_0, diag::W_UNREACH_1]),
        "nondet-race.broken-quick-never-passes" => {
            BTreeSet::from([diag::W_CUT_0, diag::W_UNREACH_1, diag::W_UNREACH_2])
        }
        _ => BTreeSet::from([diag::W_CUT_0]),
    };
    assert_eq!(
        codes(&r.outcome.diagnostics),
        expected_codes,
        "{id} calc diagnostics"
    );
    // Named expectations are independent of lower's EndId allocation. In
    // particular fast must remain fast, not an accidentally swapped EndId.
    let expected_names: &[&str] = match id {
        "coop-cancel.broken-short-clock" | "resource-contention.broken-zero-capacity" => &[],
        "nondet-race" => &["rejected", "fast", "slow_ok"],
        "nondet-race.broken-quick-never-passes" => &["rejected", "slow_ok"],
        _ => &["done"],
    };
    assert_eq!(
        r.outcome
            .terminals
            .iter()
            .map(|t| model.ends[t.end.0].as_str())
            .collect::<Vec<_>>(),
        expected_names,
        "{id} calc terminal names"
    );
    let expected: Vec<(&str, Verdict)> = match id {
        "resource-contention" => vec![
            ("safety no_resource_overcommit", Satisfied),
            ("reachable end.done", Satisfied),
        ],
        "resource-contention.broken-zero-capacity" => vec![
            ("safety no_resource_overcommit", Satisfied),
            ("reachable end.done", Violated),
        ],
        "coop-cancel.broken-short-clock" => vec![("reachable end.done", Inconclusive)],
        "nondet-race" | "nondet-race.broken-quick-never-passes" => vec![
            ("safety no_resource_overcommit", Satisfied),
            (
                "reachable end.fast",
                if id == "nondet-race" {
                    Satisfied
                } else {
                    Violated
                },
            ),
            ("reachable end.slow_ok", Satisfied),
            ("reachable end.rejected", Satisfied),
        ],
        _ => vec![("reachable end.done", Satisfied)],
    };
    assert_eq!(
        r.outcome
            .properties
            .iter()
            .map(|p| (property_name(model, p.property), p.verdict))
            .collect::<Vec<_>>(),
        expected
            .into_iter()
            .map(|(n, v)| (n.to_owned(), v))
            .collect::<Vec<_>>(),
        "{id} calc properties"
    );
}

fn configuration_fixture_oracle(id: &str, configuration: &str, model: &Model, r: &ReferenceRun) {
    // Fixed ONLY from each configuration fixture's calc.md (P1..P5/B1..B7),
    // not expected.json, diff output, or either interpreter's generated values.
    // Static B2/B7 never execute, so their potential timings are not oracles.
    let (end, elapsed, work, overall) = match (id, configuration) {
        ("config-diff-parallel" | "config-diff-parallel.broken-intent-only", "AsIs") => {
            ("done", 8, 8, Satisfied)
        }
        ("config-diff-parallel" | "config-diff-parallel.broken-intent-only", "ToBeP") => {
            ("done", 5, 8, Satisfied)
        }
        (
            "config-diff-add-actor"
            | "config-diff-add-actor.broken-missing-intent"
            | "config-diff-add-actor.broken-other-cut",
            "AsIs",
        ) => ("done", 3, 3, Satisfied),
        (
            "config-diff-add-actor"
            | "config-diff-add-actor.broken-missing-intent"
            | "config-diff-add-actor.broken-other-cut",
            "ToBe",
        ) => ("done", 5, 5, Satisfied),
        ("config-diff-retire" | "config-diff-retire.broken-missing-loses", "AsIs") => {
            ("done", 5, 5, Satisfied)
        }
        ("config-diff-retire" | "config-diff-retire.broken-missing-loses", "ToBe") => {
            ("done", 3, 3, Satisfied)
        }
        (
            "config-diff-unreachable" | "config-diff-unreachable.broken-intent-only",
            "AsIs" | "ToBe",
        ) => ("done", 1, 1, Satisfied),
        ("config-diff-terminal-union", "AsIs") => ("old", 3, 3, Violated),
        ("config-diff-terminal-union", "ToBe") => ("new", 3, 3, Violated),
        _ => panic!("unclassified configuration oracle {id}[{configuration}]"),
    };
    let end = model
        .ends
        .iter()
        .position(|name| name == end)
        .expect("calc terminal name");
    assert_eq!(
        normalized(&r.outcome),
        vec![point(end, elapsed, work, None)],
        "{id}[{configuration}] calc metrics"
    );
    assert_eq!(r.verdict(), overall, "{id}[{configuration}] calc verdict");
    assert_eq!(r.truncated(), Some(false), "{id}[{configuration}] calc cut");
    // P1..P4 and their dynamic B variants retain reachable done and safety;
    // P5 declares both terminals but each side reaches only its own terminal.
    // D5/D6 put intent/Cut-mismatch diagnostics at the diff root, never here.
    let expected_properties = if id == "config-diff-terminal-union" {
        vec![
            (
                "reachable end.old",
                if configuration == "AsIs" {
                    Satisfied
                } else {
                    Violated
                },
            ),
            (
                "reachable end.new",
                if configuration == "ToBe" {
                    Satisfied
                } else {
                    Violated
                },
            ),
            ("safety no_resource_overcommit", Satisfied),
        ]
    } else {
        vec![
            ("reachable end.done", Satisfied),
            ("safety no_resource_overcommit", Satisfied),
        ]
    };
    assert_eq!(
        r.outcome
            .properties
            .iter()
            .map(|p| (property_name(model, p.property), p.verdict))
            .collect::<Vec<_>>(),
        expected_properties
            .into_iter()
            .map(|(id, verdict)| (id.to_owned(), verdict))
            .collect::<Vec<_>>(),
        "{id}[{configuration}] calc properties"
    );
    // Cuts remain declared/complete. Their included but unused capabilities
    // now warn within the selected process; Gate.pick only returns pass.
    let expected_codes = match (id, configuration) {
        (
            "config-diff-add-actor"
            | "config-diff-add-actor.broken-missing-intent"
            | "config-diff-add-actor.broken-other-cut",
            "AsIs",
        )
        | ("config-diff-retire" | "config-diff-retire.broken-missing-loses", "ToBe")
        | ("config-diff-unreachable" | "config-diff-unreachable.broken-intent-only", "AsIs") => {
            BTreeSet::from([diag::W_UNREACH_0])
        }
        ("config-diff-unreachable" | "config-diff-unreachable.broken-intent-only", "ToBe") => {
            BTreeSet::from([diag::W_UNREACH_0, diag::W_UNREACH_1])
        }
        _ => BTreeSet::new(),
    };
    assert_eq!(
        codes(&r.outcome.diagnostics),
        expected_codes,
        "{id}[{configuration}] calc diagnostics"
    );
}

// This is syntax serialization, not an interpreter or an outcome aggregator.
fn render(m: &Model) -> String {
    fn cond(m: &Model, c: &Cond) -> String {
        match c {
            Cond::StateEq(_) | Cond::IntCompare { .. } | Cond::Contains { .. } => {
                unreachable!("legacy IR printer has no state conditions")
            }
            Cond::Eq(r) => format!(
                "{} == {}",
                m.process.bindings[r.binding.0].name,
                m.types[m.process.bindings[r.binding.0].ty.0].members[r.value.0]
            ),
            Cond::And(cs) => format!(
                "({})",
                cs.iter()
                    .map(|c| cond(m, c))
                    .collect::<Vec<_>>()
                    .join(" && ")
            ),
            Cond::Or(cs) => format!(
                "({})",
                cs.iter()
                    .map(|c| cond(m, c))
                    .collect::<Vec<_>>()
                    .join(" || ")
            ),
        }
    }
    fn call(s: &mut String, m: &Model, c: &Call) {
        let cap = &m.capabilities[c.capability.0];
        writeln!(
            s,
            "{} = {}.{}({})",
            m.process.bindings[c.binding.0].name,
            m.actors[cap.actor.0].name,
            cap.name,
            c.args
                .iter()
                .map(|b| m.process.bindings[b.0].name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
        .unwrap();
    }
    fn block(s: &mut String, m: &Model, body: &Block) {
        for statement in body {
            match &statement.kind {
                Stmt::WaitUntil(_) => {
                    panic!("differential conversion walker does not handle condition waits")
                }
                Stmt::Response(_, _) => {
                    unreachable!("legacy printer has no receipt or retry syntax")
                }
                Stmt::Call(c) => call(s, m, c),
                Stmt::End(e) => writeln!(s, "end {}", m.ends[e.0]).unwrap(),
                Stmt::Wait(_) => unreachable!("legacy printer has no wait"),
                Stmt::If {
                    condition,
                    then_body,
                    else_body,
                } => {
                    writeln!(s, "if {} {{", cond(m, condition)).unwrap();
                    block(s, m, then_body);
                    s.push_str("} else {\n");
                    block(s, m, else_body);
                    s.push_str("}\n");
                }
                Stmt::Fork { id, branches, join } => {
                    let name = &m.process.forks[id.0];
                    writeln!(s, "fork {name} {{").unwrap();
                    for c in branches {
                        call(s, m, c);
                    }
                    s.push_str("}\n");
                    match join {
                        Join::All => writeln!(s, "join all {name}").unwrap(),
                        Join::Any {
                            matches,
                            tie,
                            remaining,
                            if_none,
                        } => {
                            let first = &matches[0];
                            let ty = m.process.bindings[first.binding.0].ty;
                            write!(
                                s,
                                "join any {name} where result == {} ",
                                m.types[ty.0].members[first.value.0]
                            )
                            .unwrap();
                            match tie {
                                Tie::Nondet => s.push_str("tie nondet "),
                                Tie::By(order) => write!(
                                    s,
                                    "tie by [{}] ",
                                    order
                                        .iter()
                                        .map(|b| m.process.bindings[b.0].name.as_str())
                                        .collect::<Vec<_>>()
                                        .join(", ")
                                )
                                .unwrap(),
                            }
                            writeln!(
                                s,
                                "remaining {} if none -> {}",
                                if *remaining == Remaining::Drain {
                                    "drain"
                                } else {
                                    "cancel_then_drain"
                                },
                                m.ends[if_none.0]
                            )
                            .unwrap();
                        }
                    }
                }
            }
        }
    }
    let mut s = format!("model {} version \"0.2\" {{\n", m.name);
    for ty in &m.types {
        writeln!(s, "type {} = enum {{ {} }}", ty.name, ty.members.join(", ")).unwrap();
    }
    for (i, a) in m.actors.iter().enumerate() {
        writeln!(s, "actor {} {{", a.name).unwrap();
        if let Some(cap) = a.capacity {
            writeln!(s, "capacity {cap}").unwrap();
        }
        if let Some(source) = &a.source {
            writeln!(s, "source {}", format!("{source:?}").to_lowercase()).unwrap();
        }
        for cap in m.capabilities.iter().filter(|c| c.actor == ActorId(i)) {
            writeln!(
                s,
                "capability {}({}) -> {} {{\nduration fixed {}min\nresult nondet {{ {} }}",
                cap.name,
                cap.params
                    .iter()
                    .map(|p| format!("{}:{}", p.name, m.types[p.ty.0].name))
                    .collect::<Vec<_>>()
                    .join(", "),
                m.types[cap.return_type.0].name,
                cap.duration,
                cap.results
                    .iter()
                    .map(|v| m.types[cap.return_type.0].members[v.0].as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
            .unwrap();
            if !cap.reads.is_empty() {
                writeln!(
                    s,
                    "reads [{}]",
                    cap.reads
                        .iter()
                        .map(|a| m.actors[a.0].name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
                .unwrap();
            }
            if cap.commits {
                s.push_str("commits\n");
            }
            match cap.cancel {
                Cancel::Unspecified => {}
                Cancel::ForbiddenAfterStart => s.push_str("cancel forbidden_after_start\n"),
                Cancel::Immediate => s.push_str("cancel immediate\n"),
                Cancel::Cooperative { ack_within } => {
                    writeln!(s, "cancel cooperative ack_within {ack_within}min").unwrap()
                }
            };
            s.push_str("}\n");
        }
        s.push_str("}\n");
    }
    writeln!(s, "process {} {{", m.process.name).unwrap();
    block(&mut s, m, &m.process.body);
    writeln!(
        s,
        "}}\nbounds {{ cases {}; live_tasks {}; clock {}min }}",
        m.bounds.cases, m.bounds.live_tasks, m.bounds.clock
    )
    .unwrap();
    for v in &m.verify {
        match v.property {
            Property::Contest(_) => unreachable!(),
            Property::ProbabilityReached(_) => {
                unreachable!("legacy printer has no probability queries")
            }
            Property::Contract(_) => unreachable!("legacy renderer has no contracts"),
            Property::NoResourceOvercommit => s.push_str("verify safety no_resource_overcommit\n"),
            Property::ReachableEnd(e) => {
                writeln!(s, "verify reachable end.{}", m.ends[e.0]).unwrap()
            } // ADR 0025 completion properties are not in the legacy printer.
            _ => unreachable!(),
        }
    }
    s.push_str("}\n");
    s
}

#[test]
fn reference_choice_observations() {
    let cases = handwritten();
    let h04 = reference::explore(&cases[3].model);
    let h05 = reference::explore(&cases[4].model);
    assert_eq!(
        h04.statistics.adoptions.keys().copied().collect::<Vec<_>>(),
        vec![BindingId(1)]
    );
    assert_eq!(
        h05.statistics.adoptions.keys().copied().collect::<Vec<_>>(),
        vec![BindingId(0), BindingId(1)]
    );
    assert_eq!(
        h05.statistics.terminal_paths,
        2 * h04.statistics.terminal_paths
    );
    let h19 = reference::explore(&cases[18].model);
    assert!(h19.statistics.normal_completions > 0);
    assert!(h19.statistics.cancellation_completions > 0);
    assert_eq!(h19.statistics.simultaneous_normal_choices, 2);
    assert_eq!(h19.statistics.simultaneous_ack_choices, 2);
    // These assertions observe only the reference's candidates, never engine's.
}

#[test]
fn reference_abort_contract() {
    let cases = handwritten();
    for limits in [
        Limits {
            frames: 0,
            ..Limits::default()
        },
        Limits {
            steps_per_path: 0,
            ..Limits::default()
        },
        Limits {
            tasks_per_path: 0,
            ..Limits::default()
        },
        Limits {
            control_depth: 0,
            ..Limits::default()
        },
        Limits {
            branch_depth: 0,
            ..Limits::default()
        },
    ] {
        let r = reference::explore_with_limits(&cases[0].model, limits);
        assert_eq!(r.stop, StopReason::EnumerationLimit);
        assert_eq!(r.truncated(), None);
        assert!(r.outcome.terminals.is_empty());
        assert!(codes(&r.outcome.diagnostics).contains(diag::U_SEM_0));
    }
    // Find a finite frame budget just AFTER evidence, before complete traversal.
    // The budget is not guessed from another interpreter's traversal order.
    for (index, witness, deadlock) in [(2, true, false), (12, false, true)] {
        let mut m = cases[index].model.clone();
        if deadlock {
            m.capabilities[0].results.reverse();
        }
        let r = (1..200)
            .map(|frames| {
                reference::explore_with_limits(
                    &m,
                    Limits {
                        frames,
                        ..Limits::default()
                    },
                )
            })
            .find(|r| {
                r.stop == StopReason::EnumerationLimit
                    && (!witness || r.outcome.properties[1].verdict == Satisfied)
                    && (!deadlock || codes(&r.outcome.diagnostics).contains(diag::W_DEADLOCK_0))
            })
            .expect("abort after evidence must be exercised");
        assert!(r.outcome.terminals.is_empty());
        assert_eq!(r.truncated(), None);
        assert_eq!(r.outcome.properties[0].verdict, Unsupported);
        if index == 2 {
            assert!(
                r.outcome
                    .properties
                    .iter()
                    .any(|p| p.verdict == Unsupported)
            );
        }
        if deadlock {
            assert_eq!(r.verdict(), Violated);
        }
    }
    // Independent aggregation injection, not a reachable valid-IR overcommit.
    let evidence = Evidence {
        overcommitted: true,
        truncated: true,
        ..Evidence::default()
    };
    assert_eq!(
        evidence.verdict(Property::NoResourceOvercommit, StopReason::SemanticAbort),
        Violated
    );
    assert_eq!(
        evidence.verdict(Property::ReachableEnd(EndId(0)), StopReason::SemanticAbort),
        Unsupported
    );
}

#[test]
fn reference_validation_depth_limits() {
    // H01 exercises the root block guard; H22 has a depth-2 Eq inside
    // Or/And but only depth-1 branch blocks, isolating condition recursion.
    let cases = handwritten();
    for (index, depth) in [(0, 0), (21, 2)] {
        let r = reference::explore_with_limits(
            &cases[index].model,
            Limits {
                control_depth: depth,
                ..Limits::default()
            },
        );
        assert_eq!(r.stop, StopReason::EnumerationLimit);
        assert_eq!(r.statistics.frames, 0, "validation depth {depth}");
        assert_eq!(r.truncated(), None);
        assert!(r.outcome.terminals.is_empty());
    }
}

#[test]
fn reference_unresolved_domains_fail_closed() {
    let mut b = Builder::new("Zero", 1, 1);
    let call = b.call(0, 0, &[0], Cancel::Unspecified);
    let mut m = b.finish(vec![stmt(Stmt::Call(call)), end(0)]);
    assert_eq!(reference::explore(&m).stop, StopReason::SemanticAbort);
    m.process.body.insert(0, end(0));
    assert_eq!(
        reference::explore(&m).stop,
        StopReason::Complete,
        "unreached zero suffix is allowed"
    );
    let mut b = Builder::new("Twice", 2, 10);
    let first = pair_any(
        &mut b,
        [1, 2],
        Cancel::ForbiddenAfterStart,
        Remaining::Drain,
    );
    let second = pair_any(
        &mut b,
        [1, 2],
        Cancel::ForbiddenAfterStart,
        Remaining::Drain,
    );
    let r = reference::explore(&b.finish(vec![first, second, end(0)]));
    assert_eq!(r.stop, StopReason::SemanticAbort);
    assert!(r.outcome.terminals.is_empty());
    assert_eq!(r.truncated(), None);
    // One serial result chooses adoption or no adoption, both reaching done.
    let mut b = Builder::new("Mixed", 2, 5);
    let call = b.call(1, 0, &[0, 1], Cancel::Unspecified);
    let fork = pair_any(
        &mut b,
        [1, 2],
        Cancel::ForbiddenAfterStart,
        Remaining::Drain,
    );
    let m = b.finish(vec![
        stmt(Stmt::Call(call)),
        branch(eq(0, 0), vec![fork, end(0)], vec![end(0)]),
    ]);
    let r = reference::explore(&m);
    assert_eq!(r.stop, StopReason::SemanticAbort);
    assert!(r.outcome.terminals.is_empty());
    assert_eq!(r.outcome.properties[1].verdict, Satisfied);
    assert_eq!(r.truncated(), None);
    let source = render(&m);
    // This is NOT a report/engine differential comparison: the hand IR reaches
    // the runtime guard, whereas source lower rejects this domain statically.
    let engine = relca_core::engine::explore(&m);
    assert!(
        codes(&engine.diagnostics).contains(diag::U_SEM_0),
        "{engine:?}"
    );
    assert!(
        engine.terminals.is_empty(),
        "mixed adoption has no numeric summary"
    );
    assert_eq!(
        engine.properties[1].verdict, Satisfied,
        "discovered witness survives abort"
    );
    let report = relca_core::report::check(&source);
    assert!(
        codes(&report.diagnostics).contains(diag::U_SEM_0),
        "{report:?}"
    );
    assert_eq!(report.bounds.truncated, None);
    let wire = serde_json::to_value(report).unwrap();
    assert_eq!(wire["terminals"], serde_json::json!([]));
    assert_eq!(wire["verdict"], "unsupported");
    assert_eq!(
        wire["properties"],
        serde_json::json!([]),
        "static rejection evaluates no properties"
    );
}

#[test]
fn report_abort_null_is_observed_separately() {
    // Both any-joins have a defined if-none end. This is
    // intentionally outside the normal 62-input comparison denominator.
    let mut b = Builder::new("Twice", 2, 10);
    let first = pair_any(
        &mut b,
        [1, 2],
        Cancel::ForbiddenAfterStart,
        Remaining::Drain,
    );
    let second = pair_any(
        &mut b,
        [1, 2],
        Cancel::ForbiddenAfterStart,
        Remaining::Drain,
    );
    let m = b.finish(vec![first, second, end(0)]);
    let src = render(&m);
    // Source lower rejects multiple adoption statically. This separately checks
    // the external abort schema, not equality to the direct-IR runtime outcome.
    let report = relca_core::report::check(&src);
    assert!(
        codes(&report.diagnostics).contains(diag::U_SEM_0),
        "{report:?}\n{src}"
    );
    assert!(
        report.properties.is_empty() || report.properties.iter().all(|p| p.verdict == Unsupported)
    );
    let json = serde_json::to_value(report).unwrap();
    assert!(json["bounds"]["truncated"].is_null());
    assert_eq!(json["terminals"], serde_json::json!([]));
    assert_eq!(reference::explore(&m).truncated(), None);
}

#[test]
fn reference_rejects_malformed_ir() {
    // Each case targets one documented IR invariant (some local guards overlap,
    // as documented in validate). Never call
    // engine on malformed IR: only the standalone reference boundary is tested.
    let base = &handwritten()[0].model;
    let any_model = &handwritten()[3].model;
    let mut mutations: Vec<(&str, Model)> = Vec::new();
    macro_rules! invalid {
        ($id:literal, $base:expr, $m:ident, $body:block) => {{
            let mut $m = $base.clone();
            $body
            mutations.push(($id, $m));
        }};
    }
    // Name checks must cover every namespace, not just enum values. Rename
    // existing sites where possible so an unused-slot guard cannot mask them.
    invalid!("duplicate type name", base, m, {
        m.types.push(m.types[0].clone());
    });
    invalid!("empty type name", base, m, {
        m.types[0].name.clear();
    });
    invalid!("duplicate actor name", base, m, {
        m.actors.push(m.actors[0].clone());
    });
    invalid!("empty actor name", base, m, {
        m.actors[0].name.clear();
    });
    invalid!("duplicate end name", base, m, {
        m.ends[1] = m.ends[0].clone();
    });
    invalid!("empty end name", base, m, {
        m.ends[0].clear();
    });
    invalid!("duplicate binding name", base, m, {
        m.process.bindings[1].name = m.process.bindings[0].name.clone();
    });
    invalid!("empty binding name", base, m, {
        m.process.bindings[0].name.clear();
    });
    invalid!("duplicate fork name", &handwritten()[14].model, m, {
        m.process.forks[1] = m.process.forks[0].clone();
    });
    invalid!("empty fork name", any_model, m, {
        m.process.forks[0].clear();
    });
    invalid!("empty enum member name", base, m, {
        m.types[0].members[0].clear();
    });
    invalid!("duplicate actor capability name", base, m, {
        m.capabilities[1].name = m.capabilities[0].name.clone();
    });
    invalid!("empty capability name", base, m, {
        m.capabilities[0].name.clear();
    });
    invalid!("unused binding", base, m, {
        let mut binding = m.process.bindings[0].clone();
        binding.name = "unused".into();
        m.process.bindings.push(binding);
    });
    invalid!("unused fork", base, m, {
        m.process.forks.push("unused".into());
    });
    for location in ["include", "exclude", "defer"] {
        let mut m = base.clone();
        let entry = CutEntry {
            target: CutTarget::Actor(ActorId(99)),
            reason: "outside actor table".into(),
            loses: Some("invalid reference".into()),
        };
        m.cut = Some(Cut {
            horizon: Vec::new(),
            name: "C".into(),
            question: None,
            include: if location == "include" {
                vec![ActorId(99)]
            } else {
                vec![]
            },
            exclude: if location == "exclude" {
                vec![entry.clone()]
            } else {
                vec![]
            },
            defer: if location == "defer" {
                vec![entry]
            } else {
                vec![]
            },
            externality: vec![],
            reentry: vec![],
        });
        mutations.push((location, m));
    }
    invalid!("empty results", base, m, {
        m.capabilities[0].results.clear();
    });
    invalid!("duplicate results", base, m, {
        m.capabilities[0].results.push(ValueId(0));
    });
    invalid!("result outside enum", base, m, {
        m.capabilities[0].results[0] = ValueId(99);
    });
    invalid!("empty enum", base, m, {
        // Keep capability result indices valid: a used empty enum would also
        // fail the result-domain guard and hide removal of the enum guard.
        m.types.push(EnumType {
            name: "UnusedEmpty".into(),
            record: None,
            members: vec![],
            span: SPAN,
        });
    });
    invalid!("duplicate enum member", base, m, {
        m.types[0].members[1] = "yes".into();
    });
    invalid!("bad actor, unused capability", base, m, {
        let mut cap = m.capabilities[0].clone();
        cap.name = "unused".into();
        cap.actor = ActorId(99);
        m.capabilities.push(cap);
    });
    invalid!("bad return type, unused capability", base, m, {
        let mut cap = m.capabilities[0].clone();
        cap.name = "unused".into();
        cap.return_type = TypeId(99);
        m.capabilities.push(cap);
    });
    invalid!("bad actor", base, m, {
        m.capabilities[0].actor = ActorId(99);
    });
    invalid!("bad return type", base, m, {
        m.capabilities[0].return_type = TypeId(99);
    });
    invalid!("missing capacity", base, m, {
        m.actors[0].capacity = None;
    });
    invalid!("missing capacity, unused declaration", base, m, {
        m.actors.push(Actor {
            executors: None,
            name: "Unused".into(),
            source: None,
            capacity: None,
            metadata: Default::default(),
            span: SPAN,
        });
        let mut c = m.capabilities[0].clone();
        c.actor = ActorId(1);
        m.capabilities.push(c);
    });
    invalid!("cases zero", base, m, {
        m.bounds.cases = 0;
    });
    invalid!("cases multiple", base, m, {
        m.bounds.cases = 2;
    });
    invalid!("other version", base, m, {
        m.version = "0.1".into();
    });
    invalid!("bad binding type", base, m, {
        m.process.bindings[0].ty = TypeId(99);
    });
    invalid!("wrong binding type", base, m, {
        m.types.push(EnumType {
            name: "U".into(),
            record: None,
            members: vec!["x".into()],
            span: SPAN,
        });
        m.process.bindings[0].ty = TypeId(1);
    });
    invalid!("bad capability", base, m, {
        if let Stmt::Call(c) = &mut m.process.body[0].kind {
            c.capability = CapabilityId(99);
        }
    });
    invalid!("bad binding", base, m, {
        if let Stmt::Call(c) = &mut m.process.body[0].kind {
            c.binding = BindingId(99);
        }
    });
    invalid!("duplicate lexical slot", base, m, {
        if let Stmt::Call(c) = &mut m.process.body[1].kind {
            c.binding = BindingId(0);
        }
        // No unused slot: otherwise full-slot coverage masks this guard.
        m.process.bindings.pop();
    });
    invalid!("bad end", base, m, {
        m.process.body[2] = end(99);
    });
    invalid!("bad reachable end", base, m, {
        m.verify[1].property = Property::ReachableEnd(EndId(99));
    });
    invalid!("duplicate verify", base, m, {
        m.verify.push(m.verify[0].clone());
    });
    invalid!("nonwarning model diagnostic", base, m, {
        m.diagnostics[0].severity = diag::Severity::Error;
    });
    invalid!("unknown diagnostic", base, m, {
        m.diagnostics[0].code = "W-INVENTED".into();
    });
    invalid!("zero ack", base, m, {
        m.capabilities[0].cancel = Cancel::Cooperative { ack_within: 0 };
    });
    invalid!("duplicate lexical fork", &handwritten()[14].model, m, {
        if let Stmt::Fork { id, .. } = &mut m.process.body[1].kind {
            *id = ForkId(0);
        }
        m.process.forks.pop();
    });
    invalid!("match outside fork", any_model, m, {
        let mut binding = m.process.bindings[0].clone();
        binding.name = "outside".into();
        m.process.bindings.push(binding);
        m.process.body.push(stmt(Stmt::Call(Call {
            binding: BindingId(2),
            capability: CapabilityId(0),
            args: vec![],
            span: SPAN,
        })));
        if let Stmt::Fork {
            join: Join::Any { matches, .. },
            ..
        } = &mut m.process.body[0].kind
        {
            matches[0].binding = BindingId(2);
        }
    });
    for (name, mutate) in [
        ("tie incomplete", 0),
        ("tie duplicate", 1),
        ("tie foreign", 2),
        ("matches incomplete", 3),
        ("matches duplicate", 4),
        ("match foreign", 5),
        ("match bad value", 6),
        ("bad none end", 7),
        ("bad fork id", 8),
        ("tie extra duplicate", 9),
    ] {
        let mut m = any_model.clone();
        let Stmt::Fork { id, join, .. } = &mut m.process.body[0].kind else {
            panic!()
        };
        let Join::Any {
            matches,
            tie,
            if_none,
            ..
        } = join
        else {
            panic!()
        };
        match mutate {
            0 => *tie = Tie::By(vec![BindingId(1)]),
            1 => *tie = Tie::By(vec![BindingId(0), BindingId(0)]),
            2 => *tie = Tie::By(vec![BindingId(0), BindingId(99)]),
            3 => {
                matches.pop();
            }
            4 => matches.push(matches[0]),
            5 => matches[0].binding = BindingId(99),
            6 => matches[0].value = ValueId(99),
            7 => *if_none = EndId(99),
            8 => *id = ForkId(99),
            9 => *tie = Tie::By(vec![BindingId(0), BindingId(1), BindingId(1)]),
            _ => unreachable!(),
        }
        mutations.push((name, m));
    }
    let mut b = Builder::new("EmptyFork", 1, 1);
    let fork = b.all(vec![]);
    mutations.push(("empty fork", b.finish(vec![fork, end(0)])));
    invalid!(
        "missing cancellation contract",
        &handwritten()[5].model,
        m,
        {
            m.capabilities[1].cancel = Cancel::Unspecified;
        }
    );
    for (name, condition) in [
        ("unknown result", eq(1, 0)),
        ("bad condition binding", eq(99, 0)),
        ("bad result match", eq(0, 99)),
        ("empty And", Cond::And(vec![])),
        ("single Or", Cond::Or(vec![eq(0, 0)])),
    ] {
        let mut m = base.clone();
        m.process.body.insert(1, branch(condition, vec![], vec![]));
        mutations.push((name, m));
    }
    // A cancelled result must not be hidden by boolean short circuit.
    let mut b = Builder::new("UnknownCancelled", 2, 4);
    let fork = pair_any(
        &mut b,
        [1, 10],
        Cancel::Cooperative { ack_within: 1 },
        Remaining::CancelThenDrain,
    );
    mutations.push((
        "cancelled read",
        b.finish(vec![
            fork,
            branch(
                Cond::Or(vec![eq(0, 0), eq(1, 0)]),
                vec![end(0)],
                vec![end(1)],
            ),
        ]),
    ));
    for (name, m) in &mutations {
        let r = reference::explore(m);
        assert_eq!(r.stop, StopReason::SemanticAbort, "{name}: {r:?}");
        // These two cases are deliberately runtime-only: unspecified cancel
        // depends on an actual request; lexical availability does not guarantee
        // a value after cancellation. All other corrupt IR must fail validation
        // before even the initial frame is reserved, not at a fallback guard.
        if !matches!(*name, "missing cancellation contract" | "cancelled read") {
            assert_eq!(
                r.statistics.frames, 0,
                "{name}: must reject before execution"
            );
        }
        assert!(r.outcome.terminals.is_empty(), "{name}");
        assert_eq!(r.truncated(), None, "{name}");
        assert!(
            codes(&r.outcome.diagnostics).contains(diag::U_SEM_0),
            "{name}"
        );
        assert!(
            r.outcome
                .properties
                .iter()
                .all(|p| p.verdict == Unsupported),
            "{name}: {r:?}"
        );
    }
    eprintln!("malformed standalone IR cases={}", mutations.len());
    // Same capability name on DIFFERENT actors is legal: uniqueness is scoped
    // to (actor, name), not global. Unused capabilities are also legal (unlike
    // unused lexical binding/fork slots), provided their contracts are valid.
    let mut legal = base.clone();
    let mut actor = legal.actors[0].clone();
    actor.name = "Other".into();
    legal.actors.push(actor);
    let mut capability = legal.capabilities[0].clone();
    capability.actor = ActorId(1);
    legal.capabilities.push(capability);
    assert_eq!(reference::explore(&legal).stop, StopReason::Complete);
}

#[test]
#[should_panic(expected = "report bridge model mismatch")]
fn bridge_identity_guard_rejects_changed_model() {
    let m = &handwritten()[0].model;
    let mut changed = m.clone();
    changed.bounds.live_tasks += 1;
    assert_same_model("mutated report bridge", m, &render(&changed));
}

#[test]
fn reference_arithmetic_and_lazy_enumeration_limits() {
    // At tick1, 1+MAX cannot be represented: this is arithmetic failure, not
    // model truncation or a reference enumeration limit.
    let mut b = Builder::new("Overflow", 1, u64::MAX);
    let a = b.call(1, 0, &[0], Cancel::Unspecified);
    let c = b.call(u64::MAX, 0, &[0], Cancel::Unspecified);
    let m = b.finish(vec![stmt(Stmt::Call(a)), stmt(Stmt::Call(c)), end(0)]);
    let r = reference::explore(&m);
    assert_eq!(r.stop, StopReason::SemanticAbort);
    assert!(r.outcome.terminals.is_empty());
    assert_eq!(r.truncated(), None);
    let source = render(&m);
    assert_same_model("Overflow", &m, &source);
    let engine = relca_core::engine::explore(&m);
    assert!(codes(&engine.diagnostics).contains(diag::U_SEM_0));
    assert!(engine.terminals.is_empty());
    let report = relca_core::report::check(&source);
    assert!(codes(&report.diagnostics).contains(diag::U_SEM_0));
    assert_eq!(report.bounds.truncated, None);
    assert_eq!(
        serde_json::to_value(report).unwrap()["terminals"],
        serde_json::json!([])
    );
    // A huge delay domain is traversed lazily, never allocated as a vector or
    // eagerly expanded product before the small frame budget is checked.
    let mut b = Builder::new("HugeAck", 2, 10);
    let fork = pair_any(
        &mut b,
        [1, 10],
        Cancel::Cooperative {
            ack_within: u64::MAX,
        },
        Remaining::CancelThenDrain,
    );
    let r = reference::explore_with_limits(
        &b.finish(vec![fork, end(0)]),
        Limits {
            frames: 30,
            ..Limits::default()
        },
    );
    assert_eq!(r.stop, StopReason::EnumerationLimit);
    assert_eq!(r.statistics.frames, 30);
    assert!(r.outcome.terminals.is_empty());
    assert_eq!(
        r.outcome.properties[1].verdict, Satisfied,
        "pre-limit witness is retained"
    );
    // Long deterministic time is a loop, not recursive tick calls.
    let mut b = Builder::new("Long", 1, 100_000);
    let call = b.call(100_000, 0, &[0], Cancel::Unspecified);
    let r = reference::explore(&b.finish(vec![stmt(Stmt::Call(call)), end(0)]));
    assert_eq!(r.stop, StopReason::EnumerationLimit);
    assert_eq!(r.statistics.frames, 2);
}

#[test]
fn independent_fold_precedence_and_zero_verify() {
    let mut r = reference::explore(&handwritten()[13].model);
    r.outcome.diagnostics.clear();
    let ranked = [
        Invalid,
        Violated,
        Unsupported,
        Inconclusive,
        Vacuous,
        Satisfied,
    ];
    for (i, &left) in ranked.iter().enumerate() {
        for (j, &right) in ranked.iter().enumerate() {
            r.outcome.properties = vec![
                relca_core::outcome::PropertyOutcome {
                    cases: None,
                    probability: None,
                    property: Property::NoResourceOvercommit,
                    verdict: left,
                },
                relca_core::outcome::PropertyOutcome {
                    cases: None,
                    probability: None,
                    property: Property::ReachableEnd(EndId(0)),
                    verdict: right,
                },
            ];
            assert_eq!(r.verdict(), ranked[i.min(j)]);
        }
    }
    let mut m = handwritten()[11].model.clone();
    m.verify.clear();
    let r = reference::explore(&m);
    assert_eq!(r.verdict(), Unsupported);
    assert!(codes(&r.outcome.diagnostics).contains(diag::W_DEADLOCK_0));
    let report = relca_core::report::check(&render(&m));
    assert_eq!(report.verdict, Unsupported);
    // The contract fixes only the overall verdict when
    // verify is empty. A report may skip exploration; the reference need not.
    // Therefore diagnostics for this extra input are NOT a differential oracle.
}

#[test]
fn invoice_population_matches_hand_oracle_in_both_interpreters() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures");
    let mut directories: Vec<_> = fs::read_dir(&root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("invoice-phase0-")
        })
        .collect();
    directories.sort();
    assert_eq!(
        directories
            .iter()
            .map(|p| p.file_name().unwrap().to_str().unwrap())
            .collect::<Vec<_>>(),
        INVOICE_FIXTURES
    );
    let mut compared = 0;
    let mut errors = Vec::new();
    for directory in directories {
        let id = directory.file_name().unwrap().to_str().unwrap();
        let source = fs::read_to_string(directory.join("model.relca")).unwrap();
        let to = if id.starts_with("invoice-phase0-ai") {
            "ToBeA"
        } else {
            "ToBeP"
        };
        for selection in ["AsIs", to] {
            let label = format!("{id}[{selection}]");
            let model = lower_selected(&source, Some(selection))
                .unwrap_or_else(|d| panic!("{label}: {d:?}"));
            let r = reference::explore(&model);
            assert_eq!(
                r.stop,
                StopReason::Complete,
                "{label}: no reference exclusions"
            );
            let engine = relca_core::engine::explore(&model);
            // (end, elapsed, work) copied from the arithmetic in calc.md.
            let tuples: [(&str, u64, u64); 2] = match (selection, id) {
                ("ToBeP", "invoice-phase0-parallel") => [("rejected", 11, 14), ("done", 12, 15)],
                ("ToBeA", _) => [("rejected", 7, 10), ("done", 8, 11)],
                _ => [("rejected", 14, 14), ("done", 15, 15)],
            };
            let mut expected = tuples
                .iter()
                .map(|(name, e, w)| {
                    point(
                        model
                            .ends
                            .iter()
                            .position(|n| n == name)
                            .expect("hand terminal name"),
                        *e,
                        *w,
                        None,
                    )
                })
                .collect::<Vec<_>>();
            expected.sort_by_key(|t| t.end);
            for outcome in [&r.outcome, &engine] {
                assert_eq!(
                    normalized(outcome),
                    expected,
                    "{label}: interpreter vs hand oracle"
                );
                assert!(!outcome.truncated, "{label}");
                assert_eq!(outcome.properties.len(), 3);
                assert!(outcome.properties.iter().all(|p| p.verdict == Satisfied));
            }
            assert_eq!(r.verdict(), Satisfied);
            let mut expected_codes = BTreeSet::new();
            // The AI fixture declares Extractor.extract and Clerk.enter in
            // the same Cut. AsIs/P never call extract; A never calls enter.
            expected_codes.insert("W-UNREACH-0");
            if id == "invoice-phase0-ai.broken-unguarded-approval" && selection == "ToBeA" {
                expected_codes.insert("W-AGY-0");
            }
            // ADR 0054 "範囲" / "意味": selected-IR candidates, not a
            // blanket W-RESP-0 permission or an observed output's warning set.
            let responsibility = responsibility_oracle::expected(&model);
            assert_eq!(
                r.outcome
                    .diagnostics
                    .iter()
                    .filter(|d| d.code == diag::W_RESP_0)
                    .collect::<Vec<_>>(),
                responsibility.iter().collect::<Vec<_>>(),
                "{label}: independent responsibility candidates"
            );
            if !responsibility.is_empty() {
                expected_codes.insert(diag::W_RESP_0);
            }
            assert_eq!(
                codes(&r.outcome.diagnostics),
                expected_codes,
                "{label}: exact warning prefix and runtime codes"
            );
            errors.extend(compare_selected(
                &label,
                &model,
                &source,
                Some(selection),
                &r,
            ));
            compared += 1;
        }
    }
    assert_eq!(compared, 8, "4 invoice directories / 8 executable inputs");
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}
