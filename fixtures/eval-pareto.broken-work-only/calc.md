# eval-pareto.broken-work-only — ADR 0047 T07

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Remove only the quality objective:Fast4<Check lower6, hence sure domination.

Expected root: satisfied.
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
  "hard": [],
  "plans": [
    {
      "configuration": "Check",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Fast",
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
          "configuration": "Check",
          "lower": "6",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Fast",
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
      "Check",
      "Fast"
    ],
    "non_dominated": [
      "Fast"
    ],
    "dominated": [
      {
        "configuration": "Check",
        "dominated_by": [
          "Fast"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
