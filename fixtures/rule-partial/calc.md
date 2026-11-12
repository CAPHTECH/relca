# rule-partial — T12 (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Only approval_required amended, mode remains declared open (not first member strict). Open: posted3/work3, K V. Strict: requires false, only held2/work2, zero commits -> K vacuous. All bounds have one declaration.

## Pair witness

Good: rule-partial; control: rule-partial.broken-t12.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects violated. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "partial": "open"
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
