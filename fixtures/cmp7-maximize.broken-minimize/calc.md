# cmp7-maximize.broken-minimize — ADR 0047 T20

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only direction changes;now1/4<3/4 gives Good.

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
      "objective": "minimize cohort.WrongPerArrival",
      "direction": "minimize",
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
      "Good"
    ],
    "dominated": [
      {
        "configuration": "Bad",
        "dominated_by": [
          "Good"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
