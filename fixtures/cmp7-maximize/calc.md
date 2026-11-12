# cmp7-maximize — ADR 0047 T20

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T20:explicit maximize1/4 versus3/4 =>Bad dominates. Do not infer minimization from the name Wrong.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Good",
      "Bad"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "Good",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Bad",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [
    {
      "objective": "maximize cohort.WrongPerArrival",
      "direction": "maximize",
      "values": [
        {
          "configuration": "Good",
          "lower": "1/4",
          "upper": "1/4",
          "basis": "exact",
          "reason": null
        },
        {
          "configuration": "Bad",
          "lower": "3/4",
          "upper": "3/4",
          "basis": "exact",
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "computed",
    "reason": null,
    "among": [
      "Good",
      "Bad"
    ],
    "non_dominated": [
      "Bad"
    ],
    "dominated": [
      {
        "configuration": "Good",
        "dominated_by": [
          "Bad"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
