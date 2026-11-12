# cmp7-sim-zero.broken-registered — ADR 0047 T18

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only finish assignment changes;N is0/1,D1,intervals[0,15] and[1/16,16],not null.

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
      "constraint": "cohort.WrongPerRegistered <= 1",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1",
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
          "verdict": "inconclusive",
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
          "constraint": "cohort.WrongPerRegistered <= 1",
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
          "constraint": "cohort.WrongPerRegistered <= 1",
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
      "m": 4,
      "alpha": "1/16"
    },
    "confidence": "1/2"
  }
}
```
