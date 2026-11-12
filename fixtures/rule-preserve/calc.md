# rule-preserve — T03 (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

Version difference remains r1->r2 in both. False allows failing approval commit (K V), true refuses it (K S). Preservation summary V1/Q0/S0 versus V0/Q0/S1; all other bins0, never count V as Q.

## Pair witness

Good: rule-preserve; control: rule-preserve.broken-t03.
Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.
This input expects violated. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{
  "preserve": "violated",
  "w1": 0,
  "w3": 0,
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
      "verdict": "violated",
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
