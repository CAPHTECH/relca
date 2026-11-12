# eval-zero.broken-all-fail — ADR 0047 T04

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

One kernel distribution changes. Both verify returned S;N=D=0. 0/0 is null, not0 or S.

Expected root: inconclusive.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Check",
      "Fast"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.WrongPerRegistered <= 1/20",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Check",
          "verdict": "inconclusive",
          "basis": null,
          "interval": null,
          "reason": "zero_denominator",
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "inconclusive",
          "basis": null,
          "interval": null,
          "reason": "zero_denominator",
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Check",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1/20",
          "verdict": "inconclusive",
          "reason": "zero_denominator"
        }
      ]
    },
    {
      "configuration": "Fast",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1/20",
          "verdict": "inconclusive",
          "reason": "zero_denominator"
        }
      ]
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "no_objectives",
    "among": [],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Check",
        "status": "undetermined"
      },
      {
        "configuration": "Fast",
        "status": "undetermined"
      }
    ]
  }
}
```
