//! ADR0056 inputs authored before implementation; never generated from reports.
#![allow(dead_code)]

pub const UD: &str = "response_loss sequence [unknown, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
pub const UU: &str = "response_loss sequence [unknown, unknown] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
pub const DD: &str = "response_loss sequence [delivered, delivered] retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
pub const NONE: &str = "response_loss none";
pub const UNKNOWN: &str = "response_loss sequence [unknown]";
pub const NONDET: &str = "response_loss nondet retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";
pub const PROB: &str = "response_loss probability 0.1 basis \"test assumption: independent receipt loss conditional on each normal finish\" retry max_attempts 2 when receipt unknown backoff fixed 0min exhausted end failed";

pub const RUNTIME: &[&str] = &[
    "g-resend",
    "g-unknown",
    "g-first-delivered",
    "modeled-resend",
    "modeled-first-delivered",
    "single-unknown",
    "single-delivered",
    "correlated",
    "correlated-tight",
    "backoff",
    "independent",
    "g-nondet",
    "g-probability",
    "g-probability-zero",
    "g-probability-one",
    "g-default",
    "g-clock",
    "g-arrivals",
    "g-label",
    "max-one",
    "max-thirty-two",
    "k8-g-resend",
    "k8-g-unknown",
    "k8-g-clock",
    "known-fail",
    "known-pass",
    "requires-refusal",
    "requires-allowed",
];

pub const STATIC: &[(&str, &str, &str)] = &[
    (
        "r-resend",
        "U-SEM-0",
        "response_loss_resend_mechanism_unmodeled",
    ),
    (
        "r-without-once",
        "U-SEM-0",
        "response_loss_resend_mechanism_unmodeled",
    ),
    (
        "k8-r-resend",
        "U-SEM-0",
        "response_loss_resend_mechanism_unmodeled",
    ),
    (
        "modeled-missing-scope",
        "U-SEM-0",
        "response_loss_resend_mechanism_unmodeled",
    ),
    (
        "idempotency-without-atomic",
        "U-SEM-0",
        "idempotency_mechanism_unmodeled",
    ),
    (
        "idempotency-wrong-set",
        "U-SEM-0",
        "idempotency_mechanism_unmodeled",
    ),
    ("idempotency-start-write", "U-SEM-0", ""),
    ("max-zero", "E-CON-0", "invalid_retry_contract"),
    ("max-thirty-three", "U-SEM-0", "retry_expansion_limit"),
    ("sequence-short", "E-CON-0", "invalid_retry_contract"),
    ("sequence-extra", "E-CON-0", "invalid_retry_contract"),
    ("missing-exhausted", "E-SYN-0", ""),
    ("missing-backoff", "E-SYN-0", ""),
    ("missing-basis", "E-CON-0", ""),
    ("empty-basis", "E-CON-0", ""),
    ("invalid-probability", "E-PROB-0", ""),
    ("fork-retry", "U-SEM-0", "retry_feature_combination"),
    ("version-retry", "U-SEM-0", "retry_feature_combination"),
];

pub fn replace(s: &str, old: &str, new: &str) -> String {
    assert_ne!(old, new);
    assert_eq!(s.matches(old).count(), 1, "exactly one edit: {old}");
    s.replacen(old, new, 1)
}

pub fn append(s: &str, items: &str) -> String {
    let i = s.rfind('}').unwrap();
    format!("{}\n{items}\n{}", &s[..i], &s[i..])
}

pub fn service(kind: &str, suffix: &str) -> String {
    let mechanism = match kind {
        "g" => {
            r#"idempotency key key
            atomic { if key in keys { result rejected_duplicate }
              else { keys += key; registered = true; count += 1; result committed } }"#
        }
        "modeled" => {
            "effect_scope modeled; result nondet { committed }; finish { registered = true; count += 1 }"
        }
        "r" => "result nondet { committed }; finish { registered = true; count += 1 }",
        _ => panic!("unknown mechanism {kind}"),
    };
    let keys = if kind == "g" {
        "state keys: set<CaseId>"
    } else {
        ""
    };
    let writes = if kind == "g" {
        "[keys, registered, count]"
    } else {
        "[registered, count]"
    };
    format!(
        r#"model Response version "0.2" {{
      type Ack = enum {{ committed, rejected_duplicate }}
      observed case {{ registered: bool = false; count: 0..2 = 0 }}
      actor Ledger {{ capacity 1; {keys}
        capability post(key: CaseId) -> Ack {{
          commits; writes {writes}; duration fixed 1min
          {mechanism}
          cancel forbidden_after_start
        }}
      }}
      process Flow {{ ack = Ledger.post(case.id) {suffix};
        if ack == rejected_duplicate {{ end duplicate }} else {{ end posted }}
      }}
      cut C {{ include [Ledger] }}
      bounds {{ cases 1; live_tasks 2; clock 40min }}
      contract C {{ at_most_once Ledger.post.commit per case.id }}
      verify safety no_resource_overcommit
      verify probability reached registered == true >= 1
      verify probability reached count == 2 <= 0
    }}"#
    )
}

pub fn correlated(backoff: u64) -> String {
    format!(
        r#"model Correlated version "0.2" {{
      type V = enum {{ pass, fail }}
      latent case {{ bad: bool ~ {{ true:0.1, false:0.9 }} }}
      kernel K given [bad] {{ true -> {{ 1.0: fail }}; false -> {{ 1.0: pass }} }}
      actor Check {{ capacity 1; capability run() -> V {{ duration fixed 1min;
        outcomes kernel K; kernel_reads latent [bad]; cancel forbidden_after_start }} }}
      process Flow {{ v = Check.run() response_loss none
        retry max_attempts 2 when result [fail] backoff fixed {backoff}min exhausted end failed;
        if v == pass {{ end ok }} else {{ end failed }} }}
      cut C {{ include [Check] }}
      bounds {{ cases 1; live_tasks 2; clock 40min }}
      verify safety no_resource_overcommit
      verify probability reached end.failed <= 0.1
    }}"#
    )
}

pub fn k8(g: bool, suffix: &str) -> String {
    let s = if g {
        include_str!("../../../../fixtures/k8-cut-g/model.relca")
    } else {
        include_str!("../../../../fixtures/k8-cut-r/model.relca")
    };
    let cap = if g { "Ledger" } else { "Poster" };
    replace(
        s,
        &format!("p = {cap}.post(case.id)"),
        &format!("p = {cap}.post(case.id) {suffix}"),
    )
}

pub fn source(name: &str) -> String {
    match name {
        "g-resend" => service("g", UD),
        "g-unknown" => replace(
            &source("g-resend"),
            "[unknown, delivered]",
            "[unknown, unknown]",
        ),
        "g-first-delivered" => replace(
            &source("g-resend"),
            "[unknown, delivered]",
            "[delivered, delivered]",
        ),
        "modeled-resend" => service("modeled", UD),
        "modeled-first-delivered" => replace(
            &source("modeled-resend"),
            "[unknown, delivered]",
            "[delivered, delivered]",
        ),
        "modeled-missing-scope" => replace(&source("modeled-resend"), "effect_scope modeled;", ""),
        "single-unknown" => service("g", UNKNOWN),
        "single-delivered" => replace(&source("single-unknown"), "[unknown]", "[delivered]"),
        "correlated" => correlated(0),
        "correlated-tight" => replace(&correlated(0), "<= 0.1", "<= 0.01"),
        "backoff" => replace(&correlated(0), "backoff fixed 0min", "backoff fixed 3min"),
        "independent" => replace(
            &correlated(0),
            "kernel K given [bad] { true -> { 1.0: fail }; false -> { 1.0: pass } }",
            "kernel K { * -> { 0.1: fail; 0.9: pass } }",
        ),
        "g-nondet" => service("g", NONDET),
        "g-default" => replace(&source("g-nondet"), "response_loss nondet ", ""),
        "g-probability" => service("g", PROB),
        "g-probability-zero" => replace(
            &source("g-probability"),
            "probability 0.1 basis",
            "probability 0.0 basis",
        ),
        "g-probability-one" => replace(
            &source("g-probability"),
            "probability 0.1 basis",
            "probability 1.0 basis",
        ),
        "g-clock" => replace(&source("g-resend"), "clock 40min", "clock 1min"),
        "g-arrivals" => append(
            &replace(&source("g-resend"), "cases 1;", "cases 2;"),
            "arrivals fixed [0min, 4min]\npolicy fifo tie_by case_id",
        ),
        "g-label" => source("g-resend")
            .replace("Ledger", "Book")
            .replace("actor Book {", "actor Book { grain document;"),
        "max-one" => replace(
            &source("g-unknown"),
            "[unknown, unknown] retry max_attempts 2",
            "[unknown] retry max_attempts 1",
        ),
        "max-thirty-two" => replace(&correlated(0), "max_attempts 2", "max_attempts 32"),
        "max-thirty-three" => replace(
            &source("max-thirty-two"),
            "max_attempts 32",
            "max_attempts 33",
        ),
        "max-zero" => replace(&correlated(0), "max_attempts 2", "max_attempts 0"),
        "r-resend" => service("r", UD),
        "r-without-once" => replace(
            &source("r-resend"),
            "contract C { at_most_once Ledger.post.commit per case.id }",
            "",
        ),
        "k8-r-resend" => k8(false, UD),
        "k8-g-resend" => k8(true, UD),
        "k8-g-unknown" => replace(
            &source("k8-g-resend"),
            "[unknown, delivered]",
            "[unknown, unknown]",
        ),
        "k8-g-clock" => replace(&source("k8-g-resend"), "clock 20min", "clock 2min"),
        "idempotency-without-atomic" => replace(
            &source("g-resend"),
            "atomic { if key in keys { result rejected_duplicate }\n              else { keys += key; registered = true; count += 1; result committed } }",
            "result nondet { committed }; finish { keys += key; registered = true; count += 1 }",
        ),
        "idempotency-wrong-set" => {
            let s = replace(
                &source("g-resend"),
                "state keys: set<CaseId>",
                "state keys: set<CaseId>; state other: set<CaseId>",
            );
            replace(&s, "if key in keys", "if key in other")
        }
        "idempotency-start-write" => replace(
            &source("g-resend"),
            "duration fixed 1min",
            "duration fixed 1min; start { registered = true }",
        ),
        "sequence-short" => replace(&source("g-resend"), "[unknown, delivered]", "[unknown]"),
        "sequence-extra" => replace(
            &source("g-resend"),
            "[unknown, delivered]",
            "[unknown, delivered, delivered]",
        ),
        "missing-exhausted" => replace(&source("g-resend"), " exhausted end failed", ""),
        "missing-backoff" => replace(&source("g-resend"), " backoff fixed 0min", ""),
        "missing-basis" => replace(
            &source("g-probability"),
            " basis \"test assumption: independent receipt loss conditional on each normal finish\"",
            "",
        ),
        "empty-basis" => replace(
            &source("g-probability"),
            "test assumption: independent receipt loss conditional on each normal finish",
            "",
        ),
        "invalid-probability" => replace(
            &source("g-probability"),
            "probability 0.1 basis",
            "probability 1.1 basis",
        ),
        "fork-retry" => replace(
            &source("g-resend"),
            &format!("ack = Ledger.post(case.id) {UD};"),
            &format!("fork tasks {{ ack = Ledger.post(case.id) {UD} }} join all tasks;"),
        ),
        "version-retry" => replace(
            include_str!("../../../../fixtures/cf-version/model.relca"),
            "r = Ledger.claim()",
            "r = Ledger.claim() response_loss none retry max_attempts 2 when result [conflict] backoff fixed 0min exhausted end retry_needed",
        ),
        "known-fail" => replace(&correlated(0), "true:0.1, false:0.9", "true:1.0, false:0.0"),
        "known-pass" => replace(&correlated(0), "true:0.1, false:0.9", "true:0.0, false:1.0"),
        "requires-refusal" => replace(
            &service("g", UNKNOWN),
            "commits; writes",
            "requires registered == true else rejected_duplicate; commits; writes",
        ),
        "requires-allowed" => replace(
            &source("requires-refusal"),
            "registered == true else",
            "registered == false else",
        ),
        _ => panic!("unlisted input {name}"),
    }
}

/// Same old-language substrate, NOT an oracle for new semantics.
pub fn substrate(name: &str) -> String {
    let s = source(name);
    match name {
        "g-resend" | "modeled-resend" | "r-resend" => s
            .replace(UD, "")
            .replace("idempotency key key", "")
            .replace("effect_scope modeled;", ""),
        "correlated" | "independent" => replace(
            &s,
            "response_loss none\n        retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed",
            "",
        ),
        _ => panic!("no substrate {name}"),
    }
}

pub fn mutation_source(kind: &str) -> String {
    service(kind, NONE)
}

pub fn skipped_source() -> String {
    let s = mutation_source("g");
    let body = format!(
        "a = Ledger.post(case.id);\n\
        b = Ledger.post(case.id) response_loss sequence [delivered];\n\
        c = Ledger.post(case.id) response_loss none retry max_attempts 1 when receipt unknown backoff fixed 0min exhausted end failed;\n\
        fork f {{ d = Ledger.post(case.id) response_loss none }} join all f;\n\
        ack = Ledger.post(case.id) {NONE};"
    );
    replace(&s, &format!("ack = Ledger.post(case.id) {NONE};"), &body)
}

pub fn simulation_source(name: &str) -> String {
    append(
        &source(name),
        "simulation replications 200 seed 7 confidence 0.95",
    )
}

pub fn comparison_source(name: &str) -> String {
    let s = replace(&source(name), "process Flow {", "process AFlow {");
    append(
        &s,
        "process BFlow { end other }\nconfiguration A { cut C; process AFlow }\nconfiguration B { cut C; process BFlow }",
    )
}

pub fn no_binding_leak_source(unknown: bool) -> String {
    let suffix = if unknown {
        UNKNOWN
    } else {
        "response_loss sequence [delivered]"
    };
    let s = service("g", suffix);
    let s = replace(&s, "include [Ledger]", "include [Ledger, After]");
    let s = append(
        &s,
        "actor After { capacity 1; capability run() -> Ack { duration fixed 7min; result nondet { committed }; cancel forbidden_after_start } }",
    );
    replace(
        &s,
        "if ack == rejected_duplicate",
        "later = After.run(); if ack == rejected_duplicate",
    )
}

pub fn indicators(unknown: bool) -> String {
    append(
        &source(if unknown {
            "single-unknown"
        } else {
            "single-delivered"
        }),
        r#"
      property case.Registered = reached(registered == true)
      property case.Done = reached(end.posted || end.duplicate)
      metric cohort.RegisteredPerArrival = expected(count(case.Registered)) / arrivals
      metrics { window 40min; currency "JPY"; rate Ledger per_minute 1; rate Ledger.post per_start 2 }
    "#,
    )
}

pub const PAIRS: &[(&str, &str, &str, &str)] = &[
    (
        "g-resend",
        "g-unknown",
        "[unknown, delivered]",
        "[unknown, unknown]",
    ),
    (
        "g-resend",
        "g-first-delivered",
        "[unknown, delivered]",
        "[delivered, delivered]",
    ),
    (
        "modeled-resend",
        "modeled-first-delivered",
        "[unknown, delivered]",
        "[delivered, delivered]",
    ),
    (
        "modeled-resend",
        "modeled-missing-scope",
        "effect_scope modeled;",
        "",
    ),
    (
        "single-unknown",
        "single-delivered",
        "[unknown]",
        "[delivered]",
    ),
    ("correlated", "correlated-tight", "<= 0.1", "<= 0.01"),
    (
        "correlated",
        "backoff",
        "backoff fixed 0min",
        "backoff fixed 3min",
    ),
    (
        "g-probability",
        "g-probability-zero",
        "probability 0.1 basis",
        "probability 0.0 basis",
    ),
    (
        "g-probability",
        "g-probability-one",
        "probability 0.1 basis",
        "probability 1.0 basis",
    ),
    (
        "g-probability",
        "missing-basis",
        " basis \"test assumption: independent receipt loss conditional on each normal finish\"",
        "",
    ),
    (
        "g-probability",
        "empty-basis",
        "test assumption: independent receipt loss conditional on each normal finish",
        "",
    ),
    (
        "g-probability",
        "invalid-probability",
        "probability 0.1 basis",
        "probability 1.1 basis",
    ),
    ("g-resend", "g-clock", "clock 40min", "clock 1min"),
    (
        "max-thirty-two",
        "max-thirty-three",
        "max_attempts 32",
        "max_attempts 33",
    ),
    ("correlated", "max-zero", "max_attempts 2", "max_attempts 0"),
    (
        "g-resend",
        "sequence-short",
        "[unknown, delivered]",
        "[unknown]",
    ),
    (
        "g-resend",
        "sequence-extra",
        "[unknown, delivered]",
        "[unknown, delivered, delivered]",
    ),
    ("g-resend", "missing-exhausted", " exhausted end failed", ""),
    ("g-resend", "missing-backoff", " backoff fixed 0min", ""),
    (
        "k8-g-resend",
        "k8-g-unknown",
        "[unknown, delivered]",
        "[unknown, unknown]",
    ),
    ("k8-g-resend", "k8-g-clock", "clock 20min", "clock 2min"),
    (
        "requires-refusal",
        "requires-allowed",
        "registered == true else",
        "registered == false else",
    ),
];
