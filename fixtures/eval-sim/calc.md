# eval-sim — ADR 0047 T05

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T05: R3,confidence1/2,m=2*1*1*1=2,alpha1/8. x0 upper1-(1/8)^(1/3)=1/2; x3 lower(1/8)^(1/3)=1/2. Good[0,1/2] S at3/5;Bad[1/2,1] I. Samples work4 and2, three observations. No simulation Pareto.

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
      "constraint": "cohort.WrongPerArrival <= 3/5",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "3/5",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "1/2"
          },
          "reason": null,
          "source_reason": null,
          "point": "0"
        },
        {
          "configuration": "Bad",
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "1/2",
            "upper": "1"
          },
          "reason": null,
          "source_reason": null,
          "point": "1"
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
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 3/5",
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    }
  ],
  "objectives": [
    {
      "objective": "minimize metrics.overall.work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Good",
          "sample": {
            "mean": "4",
            "min": "4",
            "max": "4",
            "samples": 3,
            "reason": null
          },
          "reason": null
        },
        {
          "configuration": "Bad",
          "sample": {
            "mean": "2",
            "min": "2",
            "max": "2",
            "samples": 3,
            "reason": null
          },
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "not_computed",
    "reason": "simulation_estimates",
    "among": [
      "Good"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
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
      "alpha": "1/8"
    },
    "confidence": "1/2"
  }
}
```
