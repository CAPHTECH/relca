// ADR 0079: classify why the offered same-instant orders agree. A selected
// process is part of the identity: the serial invoice plans are not witnesses.
pub fn accepted(name: &str, cfg: Option<&str>) -> bool {
    reason(name, cfg).is_some()
}

pub fn reason(name: &str, cfg: Option<&str>) -> Option<&'static str> {
    Some(match (name, cfg) {
        ("arrivals-initial-product" | "arrivals-initial-product.broken-fixed-initial", None) => {
            "case-local routing; FIFO orders starts on each actor, and Chooser and R can progress independently"
        }
        (
            "contract-cancel"
            | "contract-cancel.broken-ignore-cancel"
            | "duration-cancel.broken-ignore-cancel"
            | "exec-cancel"
            | "exec-cancel.broken-ignore-cancel"
            | "prob-cancel"
            | "prob-cancel.broken-ignore-cancel"
            | "state-cancel"
            | "state-cancel.broken-ignore-cancel",
            None,
        ) => {
            "fast and slow start on separate actors; the earlier fast result fixes adoption, cancellation and subsequent drain"
        }
        ("any-drain" | "coop-cancel" | "coop-cancel.broken-short-clock", None) => {
            "both starts fit Office capacity two; the 3min finish fixes adoption before the 5min finish (or cut)"
        }
        (
            "duration-contention.broken-extra-capacity"
            | "duration-parallel"
            | "duration-parallel.broken-tight-upper",
            None,
        ) => {
            "both starts fit capacity two; duration keys and separate finish writes do not depend on order"
        }
        ("exec-shared-person", None) => {
            "the single declared executors differ, so neither start can block the other"
        }
        ("prob-finish-read" | "prob-finish-read.broken-late-writer", None) => {
            "starts use separate actors; the writer and kernel read have different finish times"
        }
        ("parallel-time", None)
        | (
            "config-diff-parallel"
            | "config-diff-parallel.broken-no-configuration"
            | "config-diff-parallel.broken-intent-only",
            Some("ToBeP"),
        ) => "both starts fit capacity two and join all observes both results",
        ("prob-invoice" | "prob-invoice.broken-ai-transfer", Some("ToBeP")) => {
            "both checks fit Clerk capacity two, use distinct finish keys and write distinct case fields before join all"
        }
        _ => return None,
    })
}
