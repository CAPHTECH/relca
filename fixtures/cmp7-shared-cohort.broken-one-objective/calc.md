# cmp7-shared-cohort.broken-one-objective — ADR 0047 U10-cohort

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Remove only Registered ratio objective. Same dominance;the shared-source pair is no longer declared.

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
      "Check"
    ],
    "dominated": [
      {
        "configuration": "Fast",
        "dominated_by": [
          "Check"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
