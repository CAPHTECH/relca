//! Responsibility boundary tests. Expected
//! values: hand derivation (time, U) and the pre-ADR-0028 binary's output for
//! old-syntax targets (compatibility: the ADR allows only E-SYN-0 -> U-SYN-0).
use relca_core::report;
use serde_json::{Value, json};

fn check(src: &str) -> Value {
    serde_json::to_value(report::check(src)).unwrap()
}

fn errors(v: &Value) -> Value {
    json!(
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["severity"] == "error")
            .cloned()
            .collect::<Vec<_>>()
    )
}

const T1: &str = "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Other { source document }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  actor Slow { capacity 1; capability check(v:Verdict) -> Verdict { duration fixed 10min; result nondet { pass,fail } } }\n  responsible p for Judge.check within 5min\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; k = Slow.check(j); if k == pass {} else {}; out = Effect.put(j); end done }\n  cut Scope { include [Record,Other,Reader,Judge,Effect,Slow] }\n  bounds { cases 1; live_tasks 4; clock 300min }\n  verify safety no_resource_overcommit\n}\n";

// major 1 (裁定 U3): a judgement that does not stop the effect (Agency's own
// W-AGY-0) is not evidence on the judgement -> effect DAG. Hand: O = Judge 2;
// U = j -> out = 2 + 3 = 5 <= within 5 -> satisfied; Slow (10) is not on it.
#[test]
fn a_judgement_that_does_not_stop_the_effect_does_not_lengthen_the_time_frame() {
    for src in [T1.to_owned(), T1.replace("if k == pass {} else {}; ", "")] {
        let a = check(&src);
        let tf = &a["responsibilities"][0]["time_frame"];
        assert_eq!(tf["status"], "satisfied", "{tf}");
        assert_eq!(tf["required_duration"], 5, "{tf}");
    }
}

// major 2 and minors 1-2: old-syntax targets keep the pre-ADR-0028 code,
// message and span (end / executor. / multi-level path stay U-SYN-0; mixed
// && and || without parentheses stays E-SYN-0 as in the shared grammar).
#[test]
fn old_syntax_responsibility_targets_keep_their_previous_diagnostics() {
    for (src, expected) in [
        (
            "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for (end == x)\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
            "[{\"code\": \"U-SYN-0\", \"severity\": \"error\", \"message\": \"unsupported syntax: end is not a state expression\", \"span\": {\"start\": 666, \"end\": 669}}]",
        ),
        (
            "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for (end)\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
            "[{\"code\": \"U-SYN-0\", \"severity\": \"error\", \"message\": \"unsupported syntax: end is not a state expression\", \"span\": {\"start\": 666, \"end\": 669}}]",
        ),
        (
            "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for (executor.id == p)\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
            "[{\"code\": \"U-SYN-0\", \"severity\": \"error\", \"message\": \"unsupported syntax: executor state expression\", \"span\": {\"start\": 674, \"end\": 675}}]",
        ),
        (
            "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for (Judge.a.b == c)\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
            "[{\"code\": \"U-SYN-0\", \"severity\": \"error\", \"message\": \"unsupported syntax: multi-level state path\", \"span\": {\"start\": 673, \"end\": 674}}]",
        ),
        (
            "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for (r == true && ok == true || r == false)\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
            "[{\"code\": \"E-SYN-0\", \"severity\": \"error\", \"message\": \"mixed && and || require parentheses\", \"span\": {\"start\": 690, \"end\": 692}}]",
        ),
    ] {
        let expected: Value = serde_json::from_str(expected).unwrap();
        assert_eq!(errors(&check(src)), expected, "{src}");
    }
}

const M2: &str = "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Other { source document }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } }; capability peek() -> Entry { duration fixed 1min; result nondet { entry } } }\n  \n  responsible p for Effect.maintenance\n  process Flow { e = Reader.read(); j = Judge.check(e); x = Effect.peek(); out = Effect.put(j); end done }\n  cut Scope { include [Record,Other,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 4; clock 300min }\n  verify safety no_resource_overcommit\n}\n";
const M2B: &str = "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Other { source document }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  \n  responsible p for Effect.maintenance\n  process Flow { e = Reader.read(); j = Judge.check(e); out = Effect.put(j); end done }\n  cut Scope { include [Record,Other,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 4; clock 300min }\n  verify safety no_resource_overcommit\n}\n";

// major 3 (裁定 U2): a non-commits Capability of the maintained Actor that
// writes nothing has no effect, so it neither makes U not_derived nor hides
// the known missing U (W-RESP-2) of the ungated Effect.put.
#[test]
fn a_maintained_actor_capability_without_writes_does_not_hide_missing_refusal() {
    let base = check(M2B);
    let with_peek = check(M2);
    assert_eq!(
        base["responsibilities"][0]["refusal"]["status"], "missing",
        "{base}"
    );
    assert_eq!(
        with_peek["responsibilities"][0]["refusal"]["status"],
        base["responsibilities"][0]["refusal"]["status"],
        "{with_peek}"
    );
    let codes = |v: &Value| {
        v["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"] == "W-RESP-2")
            .count()
    };
    assert_eq!(codes(&with_peek), codes(&base));
    assert!(codes(&base) > 0);
}

// minor 3: the displayed implication target is itself an accepted target.
#[test]
fn displayed_implication_target_round_trips_as_a_responsibility_target() {
    let src = M2B.replace(
        "responsible p for Effect.maintenance",
        "responsible p for (r -> ok == true)",
    );
    let a = check(&src);
    let shown = a["responsibilities"][0]["target"]["value"]
        .as_str()
        .unwrap()
        .to_owned();
    let again = check(&M2B.replace(
        "responsible p for Effect.maintenance",
        &format!("responsible p for {shown}"),
    ));
    assert_eq!(errors(&again), json!([]), "{shown}");
    assert_eq!(
        again["responsibilities"][0]["refusal"],
        a["responsibilities"][0]["refusal"]
    );
}

// Mutation controls for previously surviving faults (M6b, M13b, (c)/(c2), and a
// thin (d)). Expected values are hand-derived.
fn refusal(v: &Value) -> (Value, Value, Vec<(String, String)>) {
    let r = &v["responsibilities"][0]["refusal"];
    let effects = r["effects"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e["call"].as_str().unwrap().to_owned(),
                e["status"].as_str().unwrap().to_owned(),
            )
        })
        .collect();
    (r["status"].clone(), r["reason"].clone(), effects)
}
fn unchecked(v: &Value, w: &str) -> bool {
    v["agency"]["unchecked"]
        .as_array()
        .unwrap()
        .contains(&json!(w))
}

// M6b: the process if grammar does not gain "->" (ADR 0028): still E-SYN-0.
#[test]
fn process_if_does_not_accept_implication() {
    let a = check(
        "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  responsible p for Judge.check within 5min\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass -> j == pass {} else { end returned }; out = Effect.put(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
    );
    assert_eq!(a["verdict"], "invalid", "{a}");
    assert_eq!(errors(&a)[0]["code"], "E-SYN-0");
}

// M13b: maintenance U is the maintained Actor's own effects only; a
// downstream commit of another Actor (Archive.store) is not added.
#[test]
fn maintenance_refusal_excludes_another_actors_downstream_effect() {
    let (status, _, effects) = refusal(&check(
        "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } } }\n  actor Archive { capacity 1; capability store(a:Ack) -> Ack { commits; duration fixed 1min; result nondet { committed } } }\n  responsible p for Effect.maintenance\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); z = Archive.store(out); end done }\n  cut Scope { include [Record,Reader,Judge,Effect,Archive] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
    ));
    assert_eq!(status, "present");
    assert_eq!(effects, vec![("out".to_owned(), "all_paths".to_owned())]);
}

// (c)/(c2) with 裁定 U2: a non-commits Capability of the maintained Actor is
// not an effect. Writing nothing it is ignored (U present, effects [out]);
// writing state it makes U not_derived/noncommit_state_write, still not an
// effect row, and both unchecked words appear.
#[test]
fn maintenance_noncommit_capability_is_never_an_effect_row() {
    let silent = check(
        "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } }; capability log(v:Verdict) -> Ack { duration fixed 1min; result nondet { committed } } }\n  responsible p for Effect.maintenance\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); l = Effect.log(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
    );
    let (status, _, effects) = refusal(&silent);
    assert_eq!(status, "present");
    assert_eq!(effects, vec![("out".to_owned(), "all_paths".to_owned())]);
    assert!(!unchecked(&silent, "responsibility_noncommit_writes"));
    let writing = check(
        "model Duty version \"0.2\" {\n  type Entry = enum { entry }; type Verdict = enum { pass, fail }; type Ack = enum { committed }\n  people [p,q]\n  observed case { r: bool = false }\n  actor Record { source record }\n  actor Reader { capacity 1; capability read() -> Entry { reads [Record]; duration fixed 1min; result nondet { entry } } }\n  actor Judge { executors [p]; capability check(e:Entry) -> Verdict { reads [Record]; duration fixed 2min; result nondet { pass,fail } } }\n  actor Effect { capacity 1; capability put(v:Verdict) -> Ack { commits; writes r; finish { r = true }; duration fixed 3min; result nondet { committed } }; capability log(v:Verdict) -> Ack { writes r; finish { r = true }; duration fixed 1min; result nondet { committed } } }\n  responsible p for Effect.maintenance\n  process Flow { e = Reader.read(); j = Judge.check(e); if j == pass {} else { end returned }; out = Effect.put(j); l = Effect.log(j); end done }\n  cut Scope { include [Record,Reader,Judge,Effect] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
    );
    let (status, reason, effects) = refusal(&writing);
    assert_eq!(
        (status, reason),
        (json!("not_derived"), json!("noncommit_state_write"))
    );
    assert_eq!(effects, vec![("out".to_owned(), "all_paths".to_owned())]);
    assert!(unchecked(&writing, "responsibility_refusal"));
    assert!(unchecked(&writing, "responsibility_noncommit_writes"));
}

// (d) 裁定 U1: breaking needs P true before and false after. With
// P = ((r || s) -> ok) and Clear.put writing s = false: if P held, P' =
// (r -> ok) still holds, so no effect breaks P -> effects [], not_derived.
#[test]
fn an_effect_that_cannot_turn_the_implication_from_true_to_false_is_not_a_breaker() {
    let (status, reason, effects) = refusal(&check(
        "model ClearDuty version \"0.2\" {\n  type Ack = enum { committed }\n  people [p]\n  observed case { r: bool = false; s: bool = false; ok: bool = false }\n  actor Record { source record }\n  actor Clear { capacity 1; capability put() -> Ack { commits; reads [Record]; writes s; finish { s = false }; duration fixed 3min; result nondet { committed } } }\n  responsible p for ((r || s) -> ok)\n  process Flow { out = Clear.put(); end done }\n  cut Scope { include [Record,Clear] }\n  bounds { cases 1; live_tasks 2; clock 120min }\n  verify safety no_resource_overcommit\n}\n",
    ));
    assert_eq!(
        (status, reason),
        (json!("not_derived"), json!("no_reachable_effect"))
    );
    assert!(effects.is_empty());
}
