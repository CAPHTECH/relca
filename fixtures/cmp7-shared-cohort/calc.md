# cmp7-shared-cohort — ADR 0047 U10-cohort

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Two ratios share numerator property Wrong,not just all-arrival count. Check1/50<1/10 and1/46<1/10,so surely dominates. compare.pareto must name both cohort refs as shared observation.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Check",
      "Fast"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "Check",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Fast",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [
    {
      "objective": "minimize cohort.WrongPerArrival",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Check",
          "lower": "1/50",
          "upper": "1/50",
          "basis": "exact",
          "reason": null
        },
        {
          "configuration": "Fast",
          "lower": "1/10",
          "upper": "1/10",
          "basis": "exact",
          "reason": null
        }
      ]
    },
    {
      "objective": "minimize cohort.WrongPerRegistered",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Check",
          "lower": "1/46",
          "upper": "1/46",
          "basis": "exact",
          "reason": null
        },
        {
          "configuration": "Fast",
          "lower": "1/10",
          "upper": "1/10",
          "basis": "exact",
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "computed",
    "reason": null,
    "among": [
      "Check",
      "Fast"
    ],
    "non_dominated": [
      "Check"
    ],
    "dominated": [
      {
        "configuration": "Fast",
        "dominated_by": [
          "Check"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
