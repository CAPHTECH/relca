# cmp7-overlap.broken-narrow — ADR 0047 T19

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Replace only duration upper support:range becomes[3,3],3<4 sure domination.

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
          "upper": "3",
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
