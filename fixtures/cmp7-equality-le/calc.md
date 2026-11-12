# cmp7-equality-le — ADR 0047 T14

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T14:Each independent distribution has Wrong1/2 exactly. Inclusive comparison at1/2 S;strict comparison V.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Good",
      "Bad"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.WrongPerArrival <= 1/2",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/2",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/2",
            "upper": "1/2"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Bad",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/2",
            "upper": "1/2"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Good",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Bad",
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
      "Good",
      "Bad"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
