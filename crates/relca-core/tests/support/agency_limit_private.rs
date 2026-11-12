//! Small injected limits distinguish each counter without pinning charge counts.
//! Compiled as a child of agency; no production limits or CLI options are added.
use super::*;
#[path = "agency_limit_inputs.rs"]
mod input;

fn base() -> ir::Model {
    crate::lower::lower(&crate::syntax::parse(&input::gate(2)).unwrap()).unwrap()
}

fn limits(field: &str, value: usize) -> Limits {
    let mut l = DEFAULT;
    match field {
        "depth" => l.depth = value,
        "paths" => l.paths = value,
        "contexts" => l.contexts = value,
        "cells" => l.cells = value,
        "work" => l.work = value,
        _ => unreachable!(),
    }
    l
}

fn counter(field: &str) {
    let model = base();
    let expected = derive(&model).unwrap();
    let mut low = 0;
    let mut high = match field {
        "depth" => DEFAULT.depth,
        "paths" => DEFAULT.paths,
        "contexts" => DEFAULT.contexts,
        "cells" => DEFAULT.cells,
        _ => DEFAULT.work,
    };
    while low < high {
        let middle = low + (high - low) / 2;
        if derive_with_limits(&model, limits(field, middle)).is_ok() {
            high = middle;
        } else {
            low = middle + 1;
        }
    }
    assert!(low > 0, "{field} must be charged");
    assert_eq!(
        derive_with_limits(&model, limits(field, low)).unwrap(),
        expected
    );
    let e = derive_with_limits(&model, limits(field, low - 1)).unwrap_err();
    assert_eq!(e.code, "U-SEM-0");
    assert_eq!(e.severity, diag::Severity::Error);
    assert!(
        e.message.starts_with("Agency derivation limit exceeded:"),
        "{e:?}"
    );
    assert!(
        e.message.contains(&format!("{field} {}", low - 1)),
        "counter and limit: {e:?}"
    );
    let requested = e
        .message
        .split("requested ")
        .nth(1)
        .unwrap()
        .split(|c: char| !c.is_ascii_digit())
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    assert!(requested >= low, "{e:?}");
    assert!(
        e.message.contains(" in ") && e.message.contains("process Flow"),
        "phase and process: {e:?}"
    );
    let span = e.span.expect("every injected limit must identify source");
    assert!(span.start < span.end && span.end <= input::gate(2).len());
    if field == "contexts" {
        assert!(e.message.contains("refusal contexts"), "{e:?}");
        assert!(
            e.message.contains("effect p") && e.message.contains("Ledger.post"),
            "{e:?}"
        );
        assert!(e.message.contains("judgement j"), "{e:?}");
        let ir::Stmt::Call(effect) = &model.process.body[5].kind else {
            panic!()
        };
        assert_eq!(span, effect.span);
    }
}

macro_rules! test_counter {
    ($name:ident, $field:literal) => {
        #[test]
        fn $name() {
            counter($field);
        }
    };
}
test_counter!(depth_counter_is_named_and_located, "depth");
test_counter!(paths_counter_is_named_and_located, "paths");
test_counter!(contexts_counter_is_named_and_located, "contexts");
test_counter!(cells_counter_is_named_and_located, "cells");
test_counter!(work_counter_is_named_and_located, "work");
