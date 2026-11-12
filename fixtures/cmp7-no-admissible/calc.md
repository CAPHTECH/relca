# cmp7-no-admissible — ADR 0047 T21

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T21:Both verify S;Wrong1/10 and1/5 both exceed1/20. Both excluded;among empty;no_admissible_plans.

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
      "constraint": "cohort.WrongPerArrival <= 1/20",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Good",
          "verdict": "violated",
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
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 1/20",
          "verdict": "violated",
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
          "constraint": "cohort.WrongPerArrival <= 1/20",
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
    "status": "not_computed",
    "reason": "no_admissible_plans",
    "among": [],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Good",
        "status": "excluded"
      },
      {
        "configuration": "Bad",
        "status": "excluded"
      }
    ]
  }
}
```
