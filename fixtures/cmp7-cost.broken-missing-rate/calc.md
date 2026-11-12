# cmp7-cost.broken-missing-rate — ADR 0047 T07-cost

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Remove only Check's start rate. Check cost is null/not_configured,not0. Both remain non-dominated;one missing_value pair.

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
      "objective": "minimize cohort.WrongPerArrival",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Check",
          "lower": "1/50",
          "upper": "1/50",
          "basis": "exact",
          "reason": null
        },
        {
          "configuration": "Fast",
          "lower": "1/10",
          "upper": "1/10",
          "basis": "exact",
          "reason": null
        }
      ]
    },
    {
      "objective": "minimize metrics.overall.observed_cost_total",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Check",
          "lower": null,
          "upper": null,
          "basis": null,
          "reason": "not_configured"
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
      "Check",
      "Fast"
    ],
    "dominated": [],
    "undetermined_pairs": [
      {
        "to": "Fast",
        "reason": "missing_value",
        "from": "Check"
      }
    ],
    "not_evaluated": []
  }
}
```
