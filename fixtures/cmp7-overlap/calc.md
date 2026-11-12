# cmp7-overlap — ADR 0047 T19

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T19:minimize[3,6] versus[4,4]. 6<=4 false;4<=3 false. Both non-dominated,overlapping_intervals. Lower3<4 alone cannot establish domination.

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
      "objective": "minimize metrics.overall.work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "One",
          "lower": "3",
          "upper": "6",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Two",
          "lower": "4",
          "upper": "4",
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
