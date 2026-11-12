#[path = "support/read_write_review.rs"]
mod review;
use serde_json::json;

#[test]
fn outer_candidate_disappearance_survives_other_changes() {
    let (_, src, _) = review::h::cases()
        .into_iter()
        .find(|(n, _, _)| *n == "two-if-sites")
        .unwrap();
    review::assert_outer_removed(&json!(relca_core::mutation::mutate(&src)));
}
#[test]
fn excluded_substrate_and_independent_counts_match_hands() {
    for (name, src, counts, n) in review::cases() {
        let m = review::h::model(&src);
        assert_eq!(review::excluded_counts(&m), counts, "{name}");
        assert_eq!(review::h::rows(&m).len(), n, "{name}");
        let v = json!(relca_core::report::check(&src));
        assert_ne!(v["verdict"], "invalid", "{name}: {v}");
        assert_ne!(v["verdict"], "unsupported", "{name}: {v}");
    }
}
macro_rules! excluded { ($($test:ident => $name:literal),+) => { $(
    #[test] fn $test() {
        let (_,src,counts,n)=review::cases().into_iter().find(|(name,_,_,_)|*name==$name).unwrap();
        review::assert_disclosure(&json!(relca_core::report::check(&src)),counts,n);
    }
)+ }; }
excluded! {single_fork=>"single-fork",two_fork=>"two-fork",case_local=>"case-local",per_case=>"per-case",mixed=>"mixed",empty=>"empty",case_local_repeated=>"case-local-repeated",case_local_two=>"case-local-two",case_local_three=>"case-local-three",single_two=>"single-two",mixed_single_five=>"mixed-single-five"}
#[test]
fn per_case_writer_forms_remain_rejected() {
    for (i, src) in review::per_case_refusals().into_iter().enumerate() {
        let v = json!(relca_core::report::check(&src));
        assert_eq!(v["verdict"], "unsupported", "{v}");
        assert!(
            v["diagnostics"]
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == if i == 0 { "U-SEM-0" } else { "U-SYN-0" })
        );
        review::assert_disclosure(&v, [0, 0], 0);
    }
}
#[test]
fn contains_or_and_atomic_writes_match_hand_pairs() {
    let src = review::set_source();
    let m = review::h::model(&src);
    let writer = m.capabilities.iter().find(|c| c.name == "w").unwrap();
    assert!(writer.start_writes.is_empty());
    assert!(
        writer.writes.is_empty(),
        "this witness must require atomic.writes, not ordinary finish writes"
    );
    assert_eq!(writer.atomic.as_ref().unwrap().writes.len(), 1);
    let reader = m.capabilities.iter().find(|c| c.name == "r").unwrap();
    let relca_core::ir::Cond::Or(arms) = &reader.requires.as_ref().unwrap().condition else {
        panic!("hand reader must lower to Or");
    };
    assert_eq!(arms.len(), 2);
    assert!(
        arms.iter()
            .all(|c| matches!(c, relca_core::ir::Cond::Contains { .. }))
    );
    let wanted = ["A.keys: A.r -> A.w", "A.keys: A.w -> A.w"];
    review::h::assert_rows(&src, &json!(relca_core::report::check(&src)), &wanted);
    let broken = src.replace(
        "case.id in keys || case.id in keys",
        "case.id in other || case.id in other",
    );
    review::h::assert_rows(
        &broken,
        &json!(relca_core::report::check(&broken)),
        &wanted[1..],
    );
}
#[test]
fn excluded_row_stays_before_quality_metrics_and_simulation_tails() {
    let s = review::surface_source();
    let _ = review::h::model(&s);
    for v in [
        json!(relca_core::report::check(&s)),
        json!(relca_core::report::simulate(&s)),
        json!(relca_core::explain::explain(&s)),
    ] {
        review::assert_disclosure(&v, [0, 1], 1);
    }
}
