//! Closed population separation, not a wildcard exemption for future fixtures.
pub(crate) fn fixture(name: &str) -> bool {
    if !name.starts_with("int-") {
        return false;
    }
    const PAIRS: &[(&str, &str)] = &[
        ("two-add", "assignment"),
        ("upper", "overflow"),
        ("underflow", "underflow"),
        ("default", "default"),
        ("type", "enum"),
        ("latent-policy", "latent"),
        ("lt", "boundary"),
        ("le", "boundary"),
        ("gt", "boundary"),
        ("ge", "boundary"),
        ("ne", "boundary"),
        ("fork", "same-target"),
        ("start-finish", "zero-start"),
        ("set-stop", "outward"),
        ("dist", "threshold"),
        ("nondet", "threshold"),
        ("class-int", "unbounded"),
        ("class-param", "caseid"),
        ("class-caseid", "reference"),
        ("class-reads", "observed"),
        ("class-stateid", "caseid"),
        ("kernel-given", "integer"),
        ("requires", "refused"),
        ("kernel-finish", "overflow"),
        ("atomic", "overflow"),
        ("cancel", "drain"),
        ("range-vs-clock", "clock"),
        ("arrivals", "overflow"),
    ];
    assert!(
        PAIRS
            .iter()
            .any(|(base, fault)| name == format!("int-{base}")
                || name == format!("int-{base}.broken-{fault}")),
        "unclassified integer fixture {name}"
    );
    true
}

pub(crate) fn model(source: &str) -> crate::ir::Model {
    crate::lower::lower(&crate::syntax::parse(source).unwrap()).unwrap()
}

pub(crate) fn latent(max: u64) -> String {
    format!(
        "model M version \"0.2\" {{ type T = enum {{ ok }}; latent case {{ h: 0..{max} }}; actor A {{ capacity 1; capability c() -> T {{ duration fixed 1min; result nondet {{ ok }} }} }}; process P {{ end done }}; bounds {{ cases 1; live_tasks 2; clock 5min }}; verify reachable end.done }}"
    )
}
