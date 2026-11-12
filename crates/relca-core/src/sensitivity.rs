//! ADR 0060 finite-model sensitivity. Front-end units do not execute points;
//! decoding alone never establishes the same-question gate or a robust bound.
pub mod contract;
pub mod input;
pub mod inputs;
pub mod run;
pub mod source;
pub mod statistics;
mod summary;
pub use summary::{PointReference, Summary, SummaryKind, SummaryReason, SummaryStatus};
mod validation;
pub mod wire;
