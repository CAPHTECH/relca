//! Closed ADR 0027 population; historical numerical denominators never grow.
pub const NAMES: [&str; 40] = [
    "auth-scope",
    "auth-scope.broken-false-branch",
    "auth-scope-post",
    "auth-scope-post.broken-finish-value",
    "auth-scope-latent",
    "auth-scope-latent.broken-latent",
    "auth-expiry",
    "auth-expiry.broken-minute-early",
    "auth-expiry-cut",
    "auth-expiry-cut.broken-short-clock",
    "auth-expiry-absolute",
    "auth-expiry-absolute.broken-earlier-expiry",
    "auth-delegation",
    "auth-delegation.broken-wider-grants",
    "auth-delegation-forward",
    "auth-delegation-forward.broken-cycle",
    "auth-delegation-not-by",
    "auth-delegation-not-by.broken-forbidden-person",
    "auth-delegation-name",
    "auth-delegation-name.broken-missing-parent",
    "auth-delegation-expiry",
    "auth-delegation-expiry.broken-longer-child",
    "auth-delegation-issuer",
    "auth-delegation-issuer.broken-issuer",
    "auth-self-grant",
    "auth-self-grant.broken-self-issued",
    "auth-waive",
    "auth-waive.broken-restricted-authority",
    "auth-contest",
    "auth-contest.broken-restricted-reversal",
    "auth-revocation",
    "auth-revocation.broken-missing-record",
    "auth-inherited-scope",
    "auth-inherited-scope.broken-parent-scope",
    "auth-inherited-expiry",
    "auth-inherited-expiry.broken-parent-earlier",
    "auth-requires-integration",
    "auth-requires-integration.broken-starts",
    "auth-known-violation-cut",
    "auth-known-violation-cut.broken-clock",
];
#[allow(dead_code)]
pub fn fixture(name: &str) -> bool {
    let member = NAMES.contains(&name);
    assert!(
        !name.starts_with("auth-") || member,
        "unclassified ADR0027 fixture {name}"
    );
    member
}
