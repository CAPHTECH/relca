# cmp7-overlap-max.broken-narrow — ADR 0047 T19-max

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only duration lower support narrows to9;9>8 sure max domination.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "One",
      "Two"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "One",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Two",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [
    {
      "objective": "maximize metrics.overall.work",
      "direction": "maximize",
      "values": [
        {
          "configuration": "One",
          "lower": "9",
          "upper": "9",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Two",
          "lower": "8",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "computed",
    "reason": null,
    "among": [
      "One",
      "Two"
    ],
    "non_dominated": [
      "One"
    ],
    "dominated": [
      {
        "configuration": "Two",
        "dominated_by": [
          "One"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
