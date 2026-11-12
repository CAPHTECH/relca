# eval-sim-ratio.broken-tight — ADR 0047 T06

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Bad lower1/16>1/20, Good lower0<=1/20 and upper15>1/20.

Expected root: violated.
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
      "constraint": "cohort.WrongPerRegistered <= 1/20",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Good",
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "15"
          },
          "reason": null,
          "source_reason": null,
          "point": "0"
        },
        {
          "configuration": "Bad",
          "verdict": "violated",
          "basis": "statistical",
          "interval": {
            "lower": "1/16",
            "upper": "16"
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
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1/20",
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    },
    {
      "configuration": "Bad",
      "verify_verdict": "satisfied",
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1/20",
          "verdict": "violated",
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
        "status": "excluded"
      }
    ]
  },
  "statistics": {
    "interval_method": "clopper-pearson-per-case-sum",
    "multiplicity": {
      "method": "bonferroni",
      "m": 4,
      "alpha": "1/16"
    },
    "confidence": "1/2"
  }
}
```
