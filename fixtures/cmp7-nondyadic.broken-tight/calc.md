# cmp7-nondyadic.broken-tight — ADR 0047 F14-public

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only bound changes to22/25. Lower<22/25<upper,so both hard I;interval endpoints remain the non-dyadic hand values.

Expected root: inconclusive.
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
      "constraint": "cohort.WrongPerArrival <= 22/25",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "22/25",
      "values": [
        {
          "configuration": "Good",
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "28863/262144",
            "upper": "233281/262144"
          },
          "reason": null,
          "source_reason": null,
          "point": "1/2"
        },
        {
          "configuration": "Bad",
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "28863/262144",
            "upper": "233281/262144"
          },
          "reason": null,
          "source_reason": null,
          "point": "1/2"
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Good",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 22/25",
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    },
    {
      "configuration": "Bad",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 22/25",
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "simulation_estimates",
    "among": [],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Good",
        "status": "undetermined"
      },
      {
        "configuration": "Bad",
        "status": "undetermined"
      }
    ]
  },
  "statistics": {
    "interval_method": "clopper-pearson-per-case-sum",
    "multiplicity": {
      "method": "bonferroni",
      "m": 2,
      "alpha": "1/16"
    },
    "confidence": "3/4"
  }
}
```
