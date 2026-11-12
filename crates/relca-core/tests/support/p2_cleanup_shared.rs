//! One lib-test module owns the shared acceptance inputs/oracles.
use crate as core;
#[path = "p2_cleanup_contests.rs"]
pub(crate) mod contests;
#[path = "p2_cleanup_private.rs"]
pub(crate) mod hand;
#[path = "p2_cleanup_inputs.rs"]
pub(crate) mod input;
#[path = "p2_cleanup_q7.rs"]
pub(crate) mod q7;
