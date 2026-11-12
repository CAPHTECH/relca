# rule-basic.broken-t01 — T01 (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

AsIs: pass gives approve[0,2]+post[2,3]=3min elapsed/work; fail refuses before Start at2, held2/work2. K sees a pass commit and is satisfied. ToBeB false makes requires true for both approval values: posted3/work3 only; fail commits and K is violated. Nondeterminism has no 1/2 probability.

## Pair witness

Good: rule-basic; control: rule-basic.broken-t01.
Only invocation selection changes; model bytes identical.
This input expects violated. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "profile": "B"
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
