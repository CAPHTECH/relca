# cmp7-named-work — ADR 0047 U9-named

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Two named executors supply Office. short3+long5 gives named_people_work8 in serial and parallel;do not substitute total work by name or always return0. No strict improvement.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "AsIs",
      "ToBeP"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "AsIs",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "ToBeP",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [
    {
      "objective": "minimize metrics.overall.named_people_work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "8",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
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
      "AsIs",
      "ToBeP"
    ],
    "non_dominated": [
      "AsIs",
      "ToBeP"
    ],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
