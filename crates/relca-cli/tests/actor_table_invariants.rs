//! ADR 0023 境界レビューの受入の穴(T1〜T3)。
//! T1: 全 fixture の check の Report で、Actor 表の default の欄と仮定一覧の行が
//!     一対一に対応する(表と仮定は別々に計算される)。
//! T2: grain の入れ替えは check 以外(explain・mutate・simulate)でも出力を変えない。
//! T3: 複合 Actor の拒否は他の lower の警告を落とさず、span は宣言の位置を指す。
use serde_json::Value;
#[path = "support/test_tmp.rs"]
mod test_tmp;
use std::{fs, path::Path, process::Command};

fn root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn run(args: &[&str]) -> (Value, String) {
    let out = Command::new(env!("CARGO_BIN_EXE_relca"))
        .args(args)
        .current_dir(root())
        .output()
        .unwrap();
    (
        serde_json::from_slice(&out.stdout).unwrap(),
        String::from_utf8(out.stderr).unwrap(),
    )
}

fn temp(name: &str, src: &str) -> String {
    let dir = test_tmp::root().join(format!("relca-act-inv-{}-{name}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let p = dir.join("model.relca");
    fs::write(&p, src).unwrap();
    // TMPDIR may be relative to the test process, whereas run() changes the
    // child's cwd to the workspace root. Preserve the file we just wrote.
    p.canonicalize().unwrap().to_str().unwrap().to_owned()
}

#[test]
fn t1_every_check_report_pairs_default_fields_with_assumption_rows() {
    let mut reports = 0;
    for entry in fs::read_dir(root().join("fixtures")).unwrap() {
        let dir = entry.unwrap().path();
        if !dir.join("model.relca").exists() || dir.join("invocation.json").exists() {
            continue;
        }
        let model = dir.join("model.relca");
        let (v, _) = run(&["check", model.to_str().unwrap()]);
        let Some(actors) = v["actors"].as_array() else {
            continue;
        };
        reports += 1;
        let rows: std::collections::BTreeMap<String, Value> = v["assumptions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|a| (a["field"].as_str().unwrap().to_owned(), a["value"].clone()))
            .collect();
        let at = dir.display();
        for a in actors {
            let n = a["name"].as_str().unwrap();
            let default = |f: &str| a[f]["status"] == "default";
            let row = |f: &str| rows.contains_key(&format!("{n}.{f}"));
            assert_eq!(default("boundary"), row("boundary"), "{at} {n} boundary");
            if default("control") {
                assert_eq!(
                    rows.get(&format!("{n}.control")),
                    Some(&a["control"]["value"]),
                    "{at} {n} control: table and assumption computed apart"
                );
            } else {
                assert!(!row("control"), "{at} {n} declared control has a row");
            }
            for f in ["executors", "capacity"] {
                assert_eq!(
                    default(f) && !a[f]["value"].is_null(),
                    row(f),
                    "{at} {n} {f}"
                );
            }
            for f in [
                "grain",
                "contains",
                "overlaps",
                "source",
                "via_model",
                "state",
                "latent",
                "capability",
            ] {
                assert_eq!(default(f), a[f]["value"].is_null(), "{at} {n} {f}");
                assert!(!row(f), "{at} {n} {f} absence is not an assumption");
            }
        }
    }
    // 184 legacy-check fixtures lower successfully at 44ddb61.
    assert!(reports >= 180, "too few reports: {reports}");
}

fn without_grain_and_hash(mut v: Value) -> Value {
    fn walk(v: &mut Value) {
        match v {
            Value::Object(m) => {
                if let Some(Value::Array(actors)) = m.get_mut("actors") {
                    for a in actors {
                        if let Some(o) = a.as_object_mut() {
                            o.remove("grain");
                        }
                    }
                }
                if let Some(Value::Object(model)) = m.get_mut("model") {
                    model.remove("sha256");
                }
                m.values_mut().for_each(walk);
            }
            Value::Array(a) => a.iter_mut().for_each(walk),
            _ => {}
        }
    }
    walk(&mut v);
    v
}

#[test]
fn t2_grain_exchange_leaves_explain_mutate_and_simulate_unchanged() {
    let read =
        |n: &str| fs::read_to_string(root().join("fixtures").join(n).join("model.relca")).unwrap();
    let (a, b) = (read("act-grain"), read("act-grain.broken-swapped"));
    let sim = |s: &str| {
        s.replacen(
            "  verify",
            "  simulation replications 4 seed 0 confidence 0.5\n  verify",
            1,
        )
    };
    for (cmd, sa, sb) in [
        ("explain", a.clone(), b.clone()),
        ("mutate", a.clone(), b.clone()),
        ("simulate", sim(&a), sim(&b)),
    ] {
        let (va, ea) = run(&[cmd, &temp(&format!("{cmd}-a"), &sa)]);
        let (vb, eb) = run(&[cmd, &temp(&format!("{cmd}-b"), &sb)]);
        let table = |v: &Value| {
            if cmd == "mutate" {
                v["original"]["actors"].clone()
            } else {
                v["actors"].clone()
            }
        };
        assert!(table(&va).is_array(), "{cmd}: {va}");
        assert_ne!(
            table(&va),
            table(&vb),
            "{cmd}: the grain itself must differ"
        );
        assert_eq!(
            without_grain_and_hash(va),
            without_grain_and_hash(vb),
            "{cmd}"
        );
        assert_eq!(ea, eb, "{cmd} stderr");
    }
}

#[test]
fn t3_composite_rejection_keeps_lower_warnings_and_points_at_the_declaration() {
    let src = "model T version \"0.2\" {\n  type T = enum { ok }\n  actor A { contains [C]; capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }\n  actor C { capacity 1; capability w() -> T { duration fixed 1min; result nondet { ok } } }\n  actor X { }\n  process Flow { a = A.run(); end done }\n  cut K { include [A, C]; exclude X reason \"r\" }\n  bounds { cases 1; live_tasks 1; clock 3min }\n  verify reachable end.done\n}\n";
    // Hand: A contains C and both have a 1min Capability -> E-CAP-0; the
    // exclude without loses is W-CUT-1, independent of the composite check.
    for (name, model, code, needle) in [
        ("composite", src.to_owned(), "E-CAP-0", "contains [C]"),
        (
            "cycle",
            src.replace(
                "actor C { capacity 1;",
                "actor C { contains [A]; capacity 1;",
            ),
            "E-CON-0",
            "contains [A]",
        ),
    ] {
        let (v, _) = run(&["check", &temp(name, &model)]);
        assert_eq!(v["verdict"], "invalid", "{name}");
        let d = v["diagnostics"].as_array().unwrap();
        assert!(d.iter().any(|d| d["code"] == "W-CUT-1"), "{name}: {v}");
        let e = d.iter().find(|d| d["code"] == code).unwrap();
        let (s, t) = (
            e["span"]["start"].as_u64().unwrap() as usize,
            e["span"]["end"].as_u64().unwrap() as usize,
        );
        assert!(
            t - s < model.len() / 2,
            "{name}: span is the whole model {s}..{t}"
        );
        assert_eq!(
            &model[s..t],
            needle,
            "{name}: the clause, not the declaration"
        );
    }
}
