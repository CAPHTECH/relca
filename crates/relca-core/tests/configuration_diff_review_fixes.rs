//! Configuration-diff boundary cases and mutation controls.
//! Every expected value is derived by hand from the model text and the rule
//! stated next to it, never copied from production output.
use relca_core::diff;
use serde_json::{Value, json};
#[path = "support/configuration_diff_inputs.rs"]
#[allow(dead_code)]
mod inputs;
use inputs::source;

fn compare(src: &str) -> Value {
    serde_json::to_value(diff::diff(src, "From", "To")).unwrap()
}
fn compare_between(src: &str, from: &str, to: &str) -> Value {
    serde_json::to_value(diff::diff(src, from, to)).unwrap()
}
fn count(v: &Value, code: &str) -> usize {
    v["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|d| d["code"] == code)
        .count()
}
fn keys(v: &Value) -> Vec<(String, String, String)> {
    v["differences"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["kind"].as_str().unwrap().to_owned(),
                r["target"].as_str().unwrap().to_owned(),
                r["change"].as_str().unwrap().to_owned(),
            )
        })
        .collect()
}
fn k(kind: &str, target: &str, change: &str) -> (String, String, String) {
    (kind.into(), target.into(), change.into())
}
fn row<'a>(v: &'a Value, kind: &str, target: &str) -> &'a Value {
    let found: Vec<_> = v["differences"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == kind && r["target"] == target)
        .collect();
    assert_eq!(found.len(), 1, "{kind}/{target}: {v}");
    found[0]
}
fn override_of<'a>(v: &'a Value, kind: &str, target: &str) -> &'a Value {
    row(v, kind, target)["annotations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["kind"] == "override")
        .unwrap_or_else(|| panic!("override on {kind}/{target}: {v}"))
}
fn unknown(effect: &str, call: &str) -> Value {
    json!({"effect":effect,"call":call,"judgements":[],"stops":"unknown","bypasses":null})
}
fn authority_value(permission: &[&str]) -> Value {
    json!({"permission":permission,"grants":[],"authority_basis":[],"responsibilities":[]})
}

// [major] design v0.2 §7.2: authority / responsibility = Permission, Authority
// and duties. The Permission of an active capability is its permit union, or
// the Actor's executors when it has no permit (ir.rs), or null for anonymous
// executors without permit. A null Permission with three empty arrays is null.
#[test]
fn permission_is_part_of_the_authority_value_of_every_active_capability() {
    // diff2-resource: A executors [q,p], no permit, no grant/basis/duty.
    // Before activates nothing; After activates A.a. Rows: actor A, relation
    // A.a, authority A.a null -> {permission [p,q]}, resource A = 4; W1=W3=4.
    let v = compare(&source("diff2-resource"));
    assert_eq!(
        keys(&v),
        [
            k("actor", "A", "added"),
            k("relation", "A.a", "added"),
            k("authority", "A.a", "added"),
            k("resource", "A", "added"),
        ]
    );
    let a = row(&v, "authority", "A.a");
    assert_eq!(a["from"], Value::Null);
    assert_eq!(a["to"], authority_value(&["p", "q"]));
    assert_eq!((count(&v, "W-DIFF-1"), count(&v, "W-DIFF-3")), (4, 4));

    // diff2-authority: A executors [p]; A.a active on both sides with the same
    // Permission [p] -> no row; A.b added with Permission [p] and grant G.
    let v = compare(&source("diff2-authority"));
    assert_eq!(
        keys(&v),
        [
            k("relation", "A.b", "added"),
            k("authority", "A.b", "added")
        ]
    );
    assert_eq!(
        row(&v, "authority", "A.b")["to"]["permission"],
        json!(["p"])
    );

    // An explicit permit replaces the executors fallback: permission [q].
    let permit = source("diff2-authority")
        .replace("people [p]", "people [p,q]")
        .replace("executors [p];", "executors [p,q];")
        .replace("  authority G", "  permit q to A.b\n  authority G");
    assert_ne!(permit, source("diff2-authority"));
    let v = compare(&permit);
    assert_eq!(
        row(&v, "authority", "A.b")["to"]["permission"],
        json!(["q"])
    );

    // Anonymous executors and no permit: Permission null, all arrays empty ->
    // value null on both sides, no authority row (diff2-visibility: A capacity 1).
    let v = compare(&source("diff2-visibility"));
    assert!(
        !keys(&v).iter().any(|(kind, ..)| kind == "authority"),
        "{v}"
    );
}

// Invoice three plans, recounted (examples/invoice/configurations.relca).
// Clerk executors [c1,c2]; Extractor and Ledger anonymous; Approver [a1].
// AsIs -> ToBeP: Clerk.enter stays active, only two time rows.
// AsIs -> ToBeA: old actor Extractor + relations extract/enter (3), authority
// Clerk.enter {permission [c1,c2]} -> null (1), resource Extractor (1), time
// match_po/match_receipt (2), visibility Invoice (1), entry producer (1) = 9. Extractor.extract
// is anonymous without permit: no authority row. Approver.approve keeps [a1].
// ToBeP -> ToBeA: the same without the two time rows = 7.
// ADR0051: entry is read on both sides, Clerk.enter -> Extractor.extract;
// absent intent/annotations adds one W1 and W3 (reverse swaps the producer pair).
#[test]
fn invoice_three_plans_are_recounted_with_permission() {
    let src = include_str!("../../../examples/invoice/configurations.relca");
    for (a, b, n) in [
        ("AsIs", "ToBeP", 2),
        ("AsIs", "ToBeA", 9),
        ("ToBeP", "ToBeA", 7),
        ("ToBeA", "AsIs", 9),
        ("ToBeA", "ToBeA", 0),
    ] {
        let v = compare_between(src, a, b);
        assert_eq!(v["differences"].as_array().unwrap().len(), n, "{a}->{b}");
        assert_eq!(count(&v, "W-DIFF-1"), n, "{a}->{b}");
        assert_eq!(count(&v, "W-DIFF-3"), n, "{a}->{b}");
    }
    let v = compare_between(src, "AsIs", "ToBeA");
    assert_eq!(
        keys(&v),
        [
            k("actor", "Extractor", "added"),
            k("relation", "Extractor.extract", "added"),
            k("relation", "Clerk.enter", "removed"),
            k("authority", "Clerk.enter", "removed"),
            k("resource", "Extractor", "added"),
            k("time", "Clerk.match_po", "modified"),
            k("time", "Clerk.match_receipt", "modified"),
            k("visibility", "Invoice", "modified"),
            k("relation", "entry", "modified"),
        ]
    );
    assert_eq!(
        row(&v, "relation", "entry")["from"],
        json!({"producer":"Clerk.enter"})
    );
    assert_eq!(
        row(&v, "relation", "entry")["to"],
        json!({"producer":"Extractor.extract"})
    );
    let a = row(&v, "authority", "Clerk.enter");
    assert_eq!(a["from"], authority_value(&["c1", "c2"]));
    assert_eq!(a["to"], Value::Null);
    let v = compare_between(src, "ToBeA", "AsIs");
    assert_eq!(row(&v, "authority", "Clerk.enter")["change"], "added");

    // diff2-invoice: five claims (actor, extract, enter, resource, match_po),
    // global preserve. Authority and entry are unclaimed: W1 = 9 - 5 = 4, W3 0.
    // Its broken pair drops the resource claim: W1 = 5.
    for (name, w1) in [
        ("diff2-invoice", 4),
        ("diff2-invoice.broken-missing-resource-claim", 5),
    ] {
        let v = compare_between(&source(name), "AsIs", "ToBeA");
        assert_eq!(v["differences"].as_array().unwrap().len(), 9, "{name}");
        assert_eq!((count(&v, "W-DIFF-1"), count(&v, "W-DIFF-3")), (w1, 0));
    }
}

// AU1 (survived): every grants clause of one Authority to the capability is
// part of the grant. au1: G grants p to A.b; grants q to A.b -> persons [p,q].
const AU1: &str = "model DiffAuthority version \"0.2\" {\n  type V = enum { pass }\n  people [p,q]\n  actor A { executors [p,q]; capability a() -> V { duration fixed 1min; result nondet { pass } }; capability b() -> V { commits; duration fixed 1min; result nondet { pass } } }\n  authority G { basis \"権限の根拠\"; grants p to A.b; grants q to A.b; issued_by p }\n  process Before { x = A.a(); end done }\n  process After { x = A.a(); y = A.b(); end done }\n  contract Ctl { authority_basis A.b G }\n  cut C { include [A] }\n  configuration From { cut C; process Before }\n  configuration To { cut C; process After }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  verify safety no_resource_overcommit\n}\n";

#[test]
fn au1_all_grants_clauses_of_one_authority_are_recorded() {
    let v = compare(AU1);
    let to = &row(&v, "authority", "A.b")["to"];
    assert_eq!(to["permission"], json!(["p", "q"]));
    assert_eq!(
        to["grants"],
        json!([{"authority":"G","persons":["p","q"],"basis":"権限の根拠","issued_by":"p","not_by":[],
            "scope":null,"expires":null,"effective_expires":null,"revocable_by":null,"delegated_from":null}])
    );
    assert_eq!(
        to["authority_basis"],
        json!([{"id":"contract Ctl #1 authority_basis","authority":"G"}])
    );
}

// ME2 (survived): statements after an end are still lexical candidates
// (ADR 0031 U1). me2: After = pick; end done; end rejected -> meaning
// end.rejected false -> true, added. One row, W1 = W3 = 1.
const ME2: &str = "model DiffEndSuffix version \"0.2\" {\n  type V = enum { pass }\n  actor Gate { capacity 1; capability pick() -> V { duration fixed 1min; result nondet { pass } } }\n  process Before { x = Gate.pick(); end done }\n  process After { x = Gate.pick(); end done; end rejected }\n  cut C { include [Gate] }\n  configuration From { cut C; process Before }\n  configuration To { cut C; process After }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  verify safety no_resource_overcommit\n}\n";

#[test]
fn me2_ends_after_an_end_remain_in_the_lexical_meaning_set() {
    let v = compare(ME2);
    assert_eq!(keys(&v), [k("meaning", "end.rejected", "added")]);
    let r = row(&v, "meaning", "end.rejected");
    assert_eq!((&r["from"], &r["to"]), (&json!(false), &json!(true)));
}

// OV3 (survived): a capability without permit is permitted to the Actor's
// executors. ov3: A executors [p], no permit on A.g; override by p. g stops
// P.post on all paths (else ends), so p qualifies through the fallback.
const OV3: &str = "model DiffOverride version \"0.2\" {\n  type V = enum { pass,fail }\n  people [p,q]\n  actor A { executors [p]; capability seed() -> V { duration fixed 1min; result nondet { pass } }; capability g(v:V) -> V { duration fixed 1min; result nondet { pass,fail } } }\n  actor P { capacity 1; capability post(v:V) -> V { commits; duration fixed 1min; result nondet { pass } } }\n  process Before { seed = A.seed(); g = A.g(seed); end done }\n  process After { seed = A.seed(); g = A.g(seed); if g == pass { x = P.post(g) } else { end done }; end done }\n  cut C { include [A,P] }\n  configuration From { cut C; process Before }\n  configuration To { cut C; process After; intent { override relation P.post by p } }\n  bounds { cases 1; live_tasks 2; clock 20min }\n  verify safety no_resource_overcommit\n}\n";

#[test]
fn ov3_person_permission_falls_back_to_the_actor_executors() {
    let v = compare(OV3);
    let o = override_of(&v, "relation", "P.post");
    assert_eq!(o["verdict"], "satisfied");
    assert_eq!(
        o["effects"],
        json!([{"effect":"P.post","call":"x","judgements":["g"],"stops":"all_paths","bypasses":0}])
    );
    assert_eq!(v["verdict"], "satisfied");
}

// [minor] A person cannot be shown to be an executor of an anonymous Actor:
// that is unknown qualification, not a counterexample. diff2-override with
// A anonymous (capacity 1) and no permit (permit needs named executors).
#[test]
fn person_override_through_an_anonymous_judge_is_unsupported_not_violated() {
    let src = source("diff2-override")
        .replace("actor A { executors [p];", "actor A { capacity 1;")
        .replace("  permit p to A.g\n", "");
    assert_ne!(src, source("diff2-override"));
    assert!(!src.contains("permit"));
    let v = compare(&src);
    let o = override_of(&v, "relation", "P.post");
    assert_eq!(o["verdict"], "unsupported");
    assert_eq!(o["effects"], json!([unknown("P.post", "x")]));
    assert_eq!(v["verdict"], "unsupported");
    // Control: the Actor subject is judged by name, not by executors.
    let actor = src.replace("by p }", "by A }");
    assert_ne!(actor, src);
    assert_eq!(
        override_of(&compare(&actor), "relation", "P.post")["verdict"],
        "satisfied"
    );
    // Control: a judge whose named executors exclude p is still violated.
    let other = source("diff2-override").replace("by p }", "by q }");
    assert_ne!(other, source("diff2-override"));
    assert_eq!(
        override_of(&compare(&other), "relation", "P.post")["verdict"],
        "violated"
    );
}

// [minor] the unknown paths keep a stops=unknown witness.
// (a) control-cell limit: 1002 bindings -> (1002+1+129)*1003 > 1_000_000.
// The override target B.b is not an effect; the only effect-like call
// (commits or state write) is y = P.post(z), so the witness names y, not the
// target's own call z (which is the fallback of a kept difference).
fn many_calls() -> String {
    let calls = (0..1000)
        .map(|i| format!("x{i} = A.a()"))
        .collect::<Vec<_>>()
        .join("; ");
    format!(
        "model Big version \"0.2\" {{\n  type V = enum {{ pass }}\n  people [p]\n  actor A {{ executors [p]; capacity 1; capability a() -> V {{ duration fixed 1min; result nondet {{ pass }} }} }}\n  actor B {{ capacity 1; capability b() -> V {{ duration fixed 1min; result nondet {{ pass }} }} }}\n  actor P {{ capacity 1; capability post(v:V) -> V {{ commits; duration fixed 1min; result nondet {{ pass }} }} }}\n  process Before {{ {calls}; end done }}\n  process After {{ {calls}; z = B.b(); y = P.post(z); end done }}\n  cut C {{ include [A,B,P] }}\n  configuration From {{ cut C; process Before }}\n  configuration To {{ cut C; process After; intent {{ override relation B.b by p }} }}\n  bounds {{ cases 1; live_tasks 2; clock 2000min }}\n  verify safety no_resource_overcommit\n}}\n"
    )
}
#[test]
fn override_control_cell_limit_keeps_an_unknown_witness() {
    let v = compare(&many_calls());
    let o = override_of(&v, "relation", "B.b");
    assert_eq!(o["verdict"], "unsupported");
    assert_eq!(o["effects"], json!([unknown("P.post", "y")]));
    assert_eq!(v["verdict"], "unsupported");
}
// (c) a non-removed difference without any effect: diff2-visibility adds
// relation A.b (no commits, no writes). The target call y is the witness.
#[test]
fn override_without_effects_on_a_kept_difference_keeps_an_unknown_witness() {
    let src = source("diff2-visibility").replace(
        "configuration To { cut C; process After }",
        "configuration To { cut C; process After; intent { override relation A.b by A } }",
    );
    assert_ne!(src, source("diff2-visibility"));
    let v = compare(&src);
    let o = override_of(&v, "relation", "A.b");
    assert_eq!(o["verdict"], "unsupported");
    assert_eq!(o["effects"], json!([unknown("A.b", "y")]));
    assert_eq!(v["verdict"], "unsupported");
}

// E0 (one detection): add matches only added and retire only removed rows.
// diff2-visibility: A.b added, A.a removed. Wrong directions: 2 x E-DIFF-0,
// both rows unclaimed (reason null), root invalid.
#[test]
fn e0_add_and_retire_check_the_direction_of_the_computed_change() {
    let src = source("diff2-visibility").replace(
        "configuration To { cut C; process After }",
        "configuration To { cut C; process After; intent { retire A.b reason \"x\" loses \"y\"; add A.a reason \"z\" } }",
    );
    assert_ne!(src, source("diff2-visibility"));
    let v = compare(&src);
    assert_eq!(count(&v, "E-DIFF-0"), 2, "{v}");
    assert_eq!(v["verdict"], "invalid");
    for target in ["A.a", "A.b"] {
        assert_eq!(row(&v, "relation", target)["reason"], Value::Null);
    }
    // Control: the right directions are accepted.
    let right = src
        .replace(
            "retire A.b reason \"x\" loses \"y\"",
            "add A.b reason \"x\"",
        )
        .replace(
            "add A.a reason \"z\"",
            "retire A.a reason \"z\" loses \"w\"",
        );
    let v = compare(&right);
    assert_eq!(count(&v, "E-DIFF-0"), 0, "{v}");
    assert_eq!(row(&v, "relation", "A.b")["reason"], "x");
}

// OV4 (one detection): an override of a removed difference with no effect is
// vacuous (and folds so), never satisfied.
#[test]
fn ov4_removed_override_without_effects_is_vacuous() {
    let src = inputs::stripped(&source("diff2-override"))
        .replace(
            "configuration From { cut C; process Before }",
            "configuration From { cut C; process After }",
        )
        .replace(
            "configuration To { cut C; process After;",
            "configuration To { cut C; process Before; intent { override relation P.post by p };",
        );
    let v = compare(&src);
    assert_eq!(row(&v, "relation", "P.post")["change"], "removed");
    let o = override_of(&v, "relation", "P.post");
    assert_eq!(o["verdict"], "vacuous");
    assert_eq!(o["effects"], json!([]));
    assert_eq!(v["verdict"], "vacuous");
}

// LK1 (one detection): diff diagnostics belong to the diff root only. The
// side reports never carry W-DIFF-1, W-DIFF-3 or E-DIFF-0.
#[test]
fn lk1_diff_warnings_do_not_leak_into_side_reports() {
    for name in [
        "diff2-resource",
        "diff2-time",
        "diff2-warning-intent.broken-no-change",
    ] {
        let v = compare(&source(name));
        assert!(count(&v, "W-DIFF-3") + count(&v, "W-DIFF-1") > 0, "{name}");
        for side in ["from", "to"] {
            for code in ["W-DIFF-1", "W-DIFF-3", "E-DIFF-0"] {
                assert_eq!(count(&v[side]["report"], code), 0, "{name}/{side}/{code}");
            }
        }
    }
}
