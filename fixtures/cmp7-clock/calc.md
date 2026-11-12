# cmp7-clock — ADR 0047 T03C

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T03C: arrivals0,6,12; successful completion5,11,17 (return4,10,16). All finish by30. E[Wrong]=3/50; divided by3 gives1/50.

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
      "constraint": "cohort.WrongPerArrival <= 3/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "3/10",
      "values": [
        {
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/50",
            "upper": "1/50"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/50",
            "upper": "1/50"
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
