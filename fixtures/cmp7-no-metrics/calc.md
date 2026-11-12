# cmp7-no-metrics — ADR 0047 U9-not-configured

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

U9:the target has a closed type but metrics were not declared. Values null/not_configured,not static error or0. Missing-value pair retained;both admissible and non-dominated;root S.

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
          "lower": null,
          "upper": null,
          "basis": null,
          "reason": "not_configured"
        },
        {
          "configuration": "ToBeP",
          "lower": null,
          "upper": null,
          "basis": null,
          "reason": "not_configured"
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
    "undetermined_pairs": [
      {
        "to": "ToBeP",
        "reason": "missing_value",
        "from": "AsIs"
      }
    ],
    "not_evaluated": []
  }
}
```
