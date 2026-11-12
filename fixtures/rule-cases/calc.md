# rule-cases — T13 (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Four approval combinations. a1 serializes cases: c1 approve0..2, c2 approve2..4. FIFO c1 posted3/held2, c2 posted5/held4; work posted3/held2 for each, waiting2 is not work. Without FIFO either case can be first: posted elapsed3..5, held2..4 for both. No probability invented.

## Pair witness

Good: rule-cases; control: rule-cases.broken-t13.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects satisfied. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "cases": "fifo"
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
