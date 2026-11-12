# cmp7-abort.broken-deterministic — ADR 0047 T22

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Single outcome eliminates runtime nondeterminism. correct stays initialfalse,so both Wrong1;both CP[1/2,1].

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
          "verdict": "inconclusive",
          "basis": "statistical",
          "interval": {
            "lower": "1/2",
            "upper": "1"
          },
          "reason": null,
          "source_reason": null,
          "point": "1"
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
      "alpha": "1/8"
    },
    "confidence": "1/2"
  }
}
```
