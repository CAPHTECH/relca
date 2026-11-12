//! Closing acceptance: expectations are hand-derived in response_loss_close_calculations.md.
#![allow(dead_code)]

pub const MIXED: &str = r#"model Mixed version "0.2" {
  type V = enum { pass, fail }
  actor Check { capacity 1; capability run() -> V {
    duration fixed 1min; result nondet { fail }
  } }
  process Flow { v = Check.run() response_loss nondet;
    if v == pass { end done } else { end rejected }
  }
  cut C { include [Check] }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
  verify reachable end.done
}"#;

pub const SINGLE_COMMON: &str = r#"model SingleCommon version "0.2" {
  type V = enum { pass }
  actor Invoice { source document; latent difficulty: bool = false }
  kernel K given [Invoice.difficulty] {
    false -> { 1: pass }; true -> { 1: pass }
  }
  actor Check { capacity 1; capability run() -> V {
    duration fixed 1min; outcomes kernel K; kernel_reads latent [Invoice.difficulty]
  } }
  process Flow { v = Check.run() response_loss none; end done }
  cut C { include [Invoice,Check] }
  bounds { cases 1; live_tasks 1; clock 5min }
  verify safety no_resource_overcommit
}"#;
