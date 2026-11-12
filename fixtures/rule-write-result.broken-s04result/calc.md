# rule-write-result.broken-s04result — S04RESULT (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Uninvoked one-minute self-owned Flag probe; Rule bit and observed scratch have the same enum type. Only the write target changes (matching writes list follows assignment). Reject in lower before engine guard; no runtime value or time is produced for the bad input.

## Pair witness

Good: rule-write-result; control: rule-write-result.broken-s04result.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects unsupported. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "error_code": "U-SEM-0",
  "error_message": "rule values cannot be written during a run",
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
