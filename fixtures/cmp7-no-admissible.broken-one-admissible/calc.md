# cmp7-no-admissible.broken-one-admissible — ADR 0047 T21

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Threshold1/10 makes Good admissible by equality,Bad excluded. One admissible still computed/non_dominated[Good].

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
      "constraint": "cohort.WrongPerArrival <= 1/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/10",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/10",
            "upper": "1/10"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Bad",
          "verdict": "violated",
          "basis": "exact",
          "interval": {
            "lower": "1/5",
            "upper": "1/5"
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
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 1/10",
          "verdict": "violated",
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
          "lower": "4",
          "upper": "4",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Bad",
          "lower": "2",
          "upper": "2",
          "basis": "path_range",
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "computed",
    "reason": null,
    "among": [
      "Good"
    ],
    "non_dominated": [
      "Good"
    ],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Bad",
        "status": "excluded"
      }
    ]
  }
}
```
