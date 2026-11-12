//! Test-first direct seam: a generic record validator must not hide an absent
//! Rule-specific guard. Canonical record metadata is created by existing lower.
use super::*;
#[test]
fn rule_validator_itself_rejects_record_metadata() {
    let src = include_str!("../../../../fixtures/rule-basic/model.relca");
    let k = src.rfind('}').unwrap();
    let s = format!(
        "{} type R = record {{ flag: bool }} {}",
        &src[..k],
        &src[k..]
    );
    let mut m = crate::lower::lower_configuration(&crate::syntax::parse(&s).unwrap(), "AsIs")
        .unwrap()
        .model;
    assert!(validate_rules(&m).is_ok());
    let id = m
        .actors
        .iter()
        .find_map(|a| a.metadata.rule.as_ref().map(|r| r.values[0].variable))
        .unwrap();
    let ty = m.types.iter().position(|t| t.name == "R").unwrap();
    m.state_variables[id.0].base = ir::TypeId(ty);
    assert!(
        validate_rules(&m).is_err(),
        "Rule boundary must inspect resolved record metadata itself"
    );
}
