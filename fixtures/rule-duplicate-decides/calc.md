# rule-duplicate-decides — S25D (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

S25D: one edit; U-SEM-0: duplicate state variable. No execution, no time/work or success conversion.

## Pair witness

Good: rule-duplicate-decides; control: rule-duplicate-decides.broken-s25d.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects satisfied. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
