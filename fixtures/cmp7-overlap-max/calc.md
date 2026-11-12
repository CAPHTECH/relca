# cmp7-overlap-max — ADR 0047 T19-max

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T19 max control:One[6,9],Two[8,8]. Maximize requires lower>=other upper,not comparing upper endpoints. Both non-dominated,one overlapping_intervals pair.

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
          "lower": "6",
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
      "One",
      "Two"
    ],
    "dominated": [],
    "undetermined_pairs": [
      {
        "to": "Two",
        "reason": "overlapping_intervals",
        "from": "One"
      }
    ],
    "not_evaluated": []
  }
}
```
