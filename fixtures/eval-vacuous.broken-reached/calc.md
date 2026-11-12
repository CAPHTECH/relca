# eval-vacuous.broken-reached — ADR 0047 T09

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only Off/Fast process reference changes. Both now trigger K after registration,verify S and admissible.

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
  "hard": [
    {
      "constraint": "cohort.WrongPerRegistered <= 1/20",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/46",
            "upper": "1/46"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/46",
            "upper": "1/46"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
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
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "no_objectives",
    "among": [
      "Check",
      "Fast"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
