# cmp7-row-multiplicity.broken-another-row — ADR 0047 T16

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

2plans*2rows*1case=4 even with shared numerator. upper15/16>9/10 but <=19/20.

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
      "constraint": "cohort.WrongPerArrival <= 9/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "9/10",
      "values": [
        {
          "configuration": "Good",
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "15/16"
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
            "lower": "0",
            "upper": "15/16"
          },
          "reason": null,
          "source_reason": null,
          "point": "0"
        }
      ]
    },
    {
      "constraint": "cohort.WrongPerArrival <= 19/20",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "19/20",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "0",
            "upper": "15/16"
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
            "upper": "15/16"
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
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 9/10",
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
          "constraint": "cohort.WrongPerArrival <= 9/10",
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
