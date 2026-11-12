//! Independent exact arithmetic certifies the non-dyadic hand interval; this
//! test alone does not claim the private aggregate adapter has been linked.
use num_bigint::BigInt;

#[test]
fn non_dyadic_clopper_pearson_endpoints_are_outward_not_inward() {
    // P(Binomial(4,p)>=2)=6p²-8p³+3p⁴. Lower root has tail1/16.
    // At grid k/2^20 compare 16*(6k²D²-8k³D+3k⁴) to D⁴ using integers.
    let d = BigInt::from(1u64 << 20);
    let target = d.pow(4);
    let polynomial = |k: u64| {
        let k = BigInt::from(k);
        BigInt::from(16)
            * (BigInt::from(6) * k.pow(2) * d.pow(2) - BigInt::from(8) * k.pow(3) * &d
                + BigInt::from(3) * k.pow(4))
    };
    assert!(polynomial(115452) <= target);
    assert!(polynomial(115453) > target);
    assert_eq!(115452 / 4, 28863);
    assert_eq!(((1u64 << 20) - 115452) / 4, 233281);
    let data: serde_json::Value =
        serde_json::from_str(include_str!("support/evaluation_private_oracles.json")).unwrap();
    assert_eq!(
        data["cp_nondyadic"]["interval"],
        serde_json::json!({"lower":"28863/262144","upper":"233281/262144"})
    );
}
