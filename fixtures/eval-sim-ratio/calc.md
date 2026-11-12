# eval-sim-ratio — ADR 0047 T06

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T06: R1,m=2*1*1*2=4,alpha1/16. hits0:[0,15/16];hits1:[1/16,1]. Good N0,D1 =>[0,15];Bad N1,D1=>[1/16,16]. Neither upper is clipped at1.

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
      "constraint": "cohort.WrongPerRegistered <= 1/10",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1/10",
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
          "constraint": "cohort.WrongPerRegistered <= 1/10",
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
          "constraint": "cohort.WrongPerRegistered <= 1/10",
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
