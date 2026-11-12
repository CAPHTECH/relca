//! Hand inputs for the pre-merge review rulings; no report-derived expectations.
#![allow(dead_code)]

pub const RETRY: &str = "response_loss none retry max_attempts 2 when result [fail] backoff fixed 0min exhausted end failed";
pub const SUPPRESSED: &[&str] = &["W-AGY-0", "W-AGY-2", "W-LIN-2", "W-RESP-2"];

pub fn edit(s: &str, from: &str, to: &str) -> String {
    assert_ne!(from, to);
    assert_eq!(s.matches(from).count(), 1, "one exact edit: {from}");
    s.replacen(from, to, 1)
}

pub fn common(response: bool) -> String {
    let s = include_str!("../../../../fixtures/lin-case-latent.broken-actor-local/model.relca");
    if response {
        edit(s, "c1 = A.check()", "c1 = A.check() response_loss none")
    } else {
        s.into()
    }
}

pub fn basis(response: Option<&str>) -> String {
    let s = include_str!("../../../../fixtures/lin-basis/model.relca");
    let s = edit(
        s,
        "process Flow {",
        "actor Effect { capacity 1; capability post(v:Verdict) -> Verdict { commits; duration fixed 1min; result nondet { pass } } } process Flow {",
    );
    let s = edit(
        &s,
        "end done",
        "if aV == pass && bV == pass { p = Effect.post(bV); end done } else { end rejected }",
    );
    let s = edit(&s, "include [S,R,P,A,B]", "include [S,R,P,A,B,Effect]");
    match response {
        Some("judgement") => edit(&s, "bV = B.check(e)", &format!("bV = B.check(e) {RETRY}")),
        Some("target") => edit(&s, "e = P.make()", "e = P.make() response_loss none"),
        None => s,
        _ => panic!("unknown basis case"),
    }
}

pub fn warning(code: &str, response: bool) -> String {
    let (s, from) = match code {
        "W-AGY-0" => (
            include_str!("../../../../fixtures/agency-any.broken-single-valued-a/model.relca"),
            "entry = Reader.enter()",
        ),
        "W-AGY-2" => (
            include_str!("../../../../fixtures/agy2-concentration/model.relca"),
            "e = A.enter()",
        ),
        "W-LIN-2" => (
            include_str!("../../../../fixtures/lin-basis.broken-no-basis/model.relca"),
            "bV = B.check(e)",
        ),
        "W-RESP-2" => (
            include_str!("../../../../fixtures/resp-refusal.broken-cut-guard/model.relca"),
            "entry = Reader.enter()",
        ),
        _ => panic!("unknown warning"),
    };
    if response {
        edit(s, from, &format!("{from} response_loss none"))
    } else {
        s.into()
    }
}

pub fn bypass(single: bool) -> String {
    let s = include_str!("../../../../fixtures/agency-any/model.relca");
    let s = edit(
        s,
        "entry = Reader.enter()",
        "entry = Reader.enter() response_loss none",
    );
    if single {
        edit(
            &s,
            "join any f where result == pass tie by [a,b] remaining drain if none -> rejected",
            "join all f",
        )
    } else {
        s
    }
}

pub fn fallback(keep: bool) -> String {
    let s = include_str!("../../../../fixtures/agy2-keep-fallback/model.relca");
    let s = edit(
        s,
        "e = Extractor.extract()",
        "e = Extractor.extract() response_loss none",
    );
    if keep {
        s
    } else {
        edit(
            &s,
            "retire Clerk.enter from normal_path keep fallback",
            "retire Clerk.enter reason \"retired\"",
        )
    }
}

pub fn via(response: bool, model: bool) -> String {
    let s = include_str!("../../../../fixtures/act-via/model.relca");
    let s = if response {
        edit(
            s,
            "e = Extract.make()",
            "e = Extract.make() response_loss none",
        )
    } else {
        s.into()
    };
    if model {
        s
    } else {
        edit(&s, "via model \"doc@v3\";", "")
    }
}

pub fn reachable(receipt: &str, fail: bool, violation: bool) -> String {
    assert!(["unknown", "delivered"].contains(&receipt));
    let result = if fail { "fail" } else { "pass" };
    let contract = if violation {
        "contract C { invariant broken == false }"
    } else {
        ""
    };
    format!(
        r#"model UnknownReach version "0.2" {{
      type V = enum {{ pass, fail }}
      observed case {{ broken: bool = false }}
      actor Check {{ capacity 1; capability run() -> V {{ duration fixed 1min;
        result nondet {{ {result} }}; writes broken; finish {{ broken = true }} }} }}
      process Flow {{ v = Check.run() response_loss sequence [{receipt}];
        if v == pass {{ end done }} else {{ end rejected }} }}
      cut C {{ include [Check] }}
      bounds {{ cases 1; live_tasks 1; clock 5min }}
      {contract}
      verify safety no_resource_overcommit
      verify reachable end.done
    }}"#
    )
}
