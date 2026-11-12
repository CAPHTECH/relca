# rule-version-only-diff — EDGE-VERSION (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Values identical true but versions r1/r2 differ: one rule row. Forward S; reverse requires nonexistent r2->r1 amendment, invalid.

## Pair witness

Good: rule-version-only-diff; control: rule-version-only-diff.broken-edge-version.
Only invocation selection changes; model bytes identical.
This input expects satisfied. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "side_expected": {
    "from": {
      "verdict": "satisfied",
      "bounds": {
        "truncated": false
      },
      "terminals": [
        {
          "end": "posted",
          "elapsed": {
            "min": 3,
            "max": 3
          },
          "work": {
            "min": 3,
            "max": 3
          },
          "result_at": null
        },
        {
          "end": "held",
          "elapsed": {
            "min": 2,
            "max": 2
          },
          "work": {
            "min": 2,
            "max": 2
          },
          "result_at": null
        }
      ]
    },
    "to": {
      "verdict": "satisfied",
      "bounds": {
        "truncated": false
      },
      "terminals": [
        {
          "end": "posted",
          "elapsed": {
            "min": 3,
            "max": 3
          },
          "work": {
            "min": 3,
            "max": 3
          },
          "result_at": null
        },
        {
          "end": "held",
          "elapsed": {
            "min": 2,
            "max": 2
          },
          "work": {
            "min": 2,
            "max": 2
          },
          "result_at": null
        }
      ]
    }
  }
}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
