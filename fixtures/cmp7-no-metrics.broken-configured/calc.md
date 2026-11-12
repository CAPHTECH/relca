# cmp7-no-metrics.broken-configured — ADR 0047 U9-not-configured

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Add only the metrics declaration. Both work8;equal points are known non-dominating,not an unknown pair.

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
      "objective": "minimize metrics.overall.work",
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
