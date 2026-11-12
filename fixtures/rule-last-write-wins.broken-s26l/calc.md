# rule-last-write-wins.broken-s26l — S26L (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

S26L: one edit; U-SEM-0: last_write_wins outside shared Actor state is unsupported.
Integration correction: ADR0041 is now present. Its existing var_decl grammar
accepts last_write_wins, but a fixed Rule is not shared mutable ActorObserved
state. Refuse in lower with a source span, not a new parser restriction. No
execution, no time/work or success conversion. The expected code/message is
derived from that existing boundary, not permission learned from new output.

## Pair witness

Good: rule-last-write-wins; control: rule-last-write-wins.broken-s26l.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects unsupported. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "error_code": "U-SEM-0",
  "error_message": "last_write_wins outside shared Actor state is unsupported",
  "error_count": 1,
  "parser": false
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
