#[path = "support/relation_producers.rs"]
mod acceptance;
#[test]
fn join_any_reads_branch_results() {
    acceptance::join_positive(run)
}
#[test]
fn join_any_same_producer_and_join_all_controls() {
    acceptance::join_controls(run)
}
#[test]
fn join_any_directed_intent() {
    acceptance::join_intent(run)
}
#[test]
fn producer_wire_refuses_non_preserve_annotations() {
    acceptance::wire_annotations()
}
fn run(src: &str, a: &str, b: &str) -> serde_json::Value {
    serde_json::to_value(relca_core::diff::diff(src, a, b)).unwrap()
}
#[test]
fn minimal_positive_and_reverse() {
    acceptance::positive(run)
}
#[test]
fn same_producer_and_other_zero_controls() {
    acceptance::controls(run)
}
#[test]
fn lexical_reads_forks_conditions_and_suffix() {
    acceptance::lexical(run)
}
#[test]
fn directed_intent_and_explicit_refusals() {
    acceptance::intent(run)
}
#[test]
fn failed_sides_and_mismatch_keep_relation_unchecked() {
    acceptance::failures(run)
}
#[test]
fn invoice_hand_nucleus() {
    for src in [
        include_str!("../../../fixtures/invoice-phase0-ai/model.relca"),
        include_str!("../../../examples/invoice/phase1.relca"),
    ] {
        acceptance::invoice(run, src)
    }
}
#[test]
fn closed_wire_valid_forms() {
    acceptance::wire_controls()
}
#[test]
fn closed_wire_single_faults() {
    acceptance::wire_faults()
}
#[test]
fn all_lexical_substrates_are_already_static_valid() {
    for src in acceptance::lexical_sources() {
        let ast = relca_core::syntax::parse(&src).unwrap_or_else(|e| panic!("{src}: {e:?}"));
        for name in ["From", "To"] {
            relca_core::lower::lower_configuration(&ast, name)
                .unwrap_or_else(|e| panic!("{src}/{name}: {e:?}"));
        }
    }
}
#[test]
fn all_join_any_substrates_are_static_valid_before_diff() {
    let positive = acceptance::join_source(false, false, "");
    for src in [
        positive.clone(),
        positive.replace("tie by [x, spare]", "tie nondet"),
        acceptance::join_source(true, false, ""),
        acceptance::join_source(false, true, ""),
        acceptance::join_source(
            false,
            false,
            "intent { change relation x reason \"transfer\" }",
        ),
    ] {
        let parsed = relca_core::syntax::parse(&src).unwrap();
        for name in ["From", "To"] {
            relca_core::lower::lower_configuration(&parsed, name)
                .unwrap_or_else(|e| panic!("{src}/{name}: {e:?}"));
        }
    }
}
