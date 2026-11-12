# rule-label.broken-t08 — T08 (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Renaming ApprovalRule to Charter and grain document does not change any event/guard/value. Compare semantic rows under explicit name mapping; hash, spans and embedded names legitimately differ. This is equivalence, not an additional fault pair.

## Pair witness

Good: rule-label; control: rule-label.broken-t08.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects satisfied. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "profile": "A",
  "rule_actor": "Charter"
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
