# cmp7-case-multiplicity.broken-one-case — ADR 0047 T15

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Single case-count transformation updates arrivals+bound together. m2 alpha1/8,upper7/8<=9/10.

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
      "constraint": "cohort.WrongPerArrival <= 9/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "9/10",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "7/8"
          },
          "reason": null,
          "source_reason": null,
          "point": "0"
        },
        {
          "configuration": "Bad",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "7/8"
          },
          "reason": null,
          "source_reason": null,
          "point": "0"
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
    "reason": "simulation_estimates",
    "among": [
      "Good",
      "Bad"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  },
  "statistics": {
    "interval_method": "clopper-pearson-per-case-sum",
    "multiplicity": {
      "method": "bonferroni",
      "m": 2,
      "alpha": "1/8"
    },
    "confidence": "1/2"
  }
}
```
