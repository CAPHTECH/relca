#![allow(dead_code)]
use serde_json::{Value, json};

pub fn fixture(name: &str) -> String {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(root.join("../../fixtures").join(name).join("model.relca")).unwrap()
}
pub fn append(s: &str, item: &str) -> String {
    let k = s.rfind('}').unwrap();
    format!("{}\n{item}\n{}", &s[..k], &s[k..])
}
pub fn edit(s: &str, old: &str, new: &str) -> String {
    assert_eq!(s.matches(old).count(), 1, "single effective edit {old}");
    assert_ne!(old, new);
    s.replacen(old, new, 1)
}
pub fn scenario(s: &str, h: Option<u64>) -> String {
    let (mut body, mut scope) = (String::new(), String::new());
    for line in s.lines() {
        if ["arrivals fixed ", "bounds {", "simulation replications "]
            .iter()
            .any(|p| line.trim_start().starts_with(p))
        {
            scope.push_str(line);
            scope.push('\n');
        } else {
            body.push_str(line);
            body.push('\n');
        }
    }
    if !scope.contains("arrivals fixed") {
        scope.push_str("arrivals fixed [0min]\n");
    }
    append(
        &body,
        &format!(
            "scenario Day {{ {scope} {} }}",
            h.map_or_else(String::new, |h| format!("horizon {h}min"))
        ),
    )
}
pub fn horizon(h: u64, bound: &str, objective: bool) -> String {
    let s = scenario(&fixture("cmp7-clock"), Some(h));
    let s = edit(
        &s,
        "compare [Check, Fast] cut C hard",
        "compare [Check, Fast] under [Day] cut C hard",
    );
    let s = if bound == "0.3" {
        s
    } else {
        edit(&s, "<= 0.3", &format!("<= {bound}"))
    };
    if objective {
        edit(
            &s,
            "<= 0.3 }",
            "<= 0.3 } objectives { minimize cohort.WrongPerArrival }",
        )
    } else {
        s
    }
}
pub fn simulation(h: u64, denominator: bool) -> String {
    let s = edit(
        &fixture("eval-sim"),
        "kernel bad { * -> { 1: pass with correct = false } }",
        "kernel bad { * -> { 1: pass with correct = true } }",
    );
    let s = edit(
        &s,
        "capability quick() -> V { duration fixed 1min",
        "capability quick() -> V { duration fixed 3min",
    );
    let s = scenario(&s, Some(h));
    let s = edit(
        &s,
        "compare [Good, Bad] cut C hard",
        "compare [Good, Bad] under [Day] cut C hard",
    );
    if denominator {
        edit(
            &s,
            "hard { cohort.WrongPerArrival <= 0.6 }",
            "hard { cohort.WrongPerRegistered <= 0.6 }",
        )
    } else {
        s
    }
}
pub fn with_plans(s: &str, objectives: bool) -> String {
    append(
        s,
        &format!(
            "configuration Left {{ cut C; process P }}\nconfiguration Right {{ cut C; process P }}\ncompare [Left,Right] under [Day] hard {{ cohort.Rate <= 1 }} {}",
            if objectives {
                "objectives { minimize cohort.Rate }"
            } else {
                ""
            }
        ),
    )
}
pub fn row<'a>(v: &'a Value, section: &str, name: &str) -> &'a Value {
    v[section]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["name"] == name)
        .unwrap()
}
pub fn interval(l: &str, u: &str) -> Value {
    json!({"lower":l,"upper":u})
}
pub fn exact(n: &str) -> Value {
    json!({"lower":n,"upper":n,"basis":"exact","reason":null})
}
pub fn without_compare(s: &str) -> String {
    let lines: Vec<_> = s
        .lines()
        .filter(|l| !l.trim_start().starts_with("compare ["))
        .collect();
    assert_eq!(lines.len() + 1, s.lines().count());
    lines.join("\n")
}
