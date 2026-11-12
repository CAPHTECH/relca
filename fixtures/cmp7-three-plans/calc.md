# cmp7-three-plans — ADR 0047 T10-all-pairs

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Three plans in deliberately non-sorted order. Fast Wrong1/10 excluded;Check and Middle Wrong1/50 admissible. Both admissible work[6,8] overlap,so neither surely dominates and one unknown pair remains. Agency must include all three i<j pairs,including excluded Fast.

Expected root: violated.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Fast",
      "Check",
      "Middle"
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
          "configuration": "Fast",
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
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/50",
            "upper": "1/50"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Middle",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/50",
            "upper": "1/50"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Fast",
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
      "configuration": "Check",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Middle",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [
    {
      "objective": "minimize metrics.overall.work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Fast",
          "lower": "4",
          "upper": "4",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Check",
          "lower": "6",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Middle",
          "lower": "6",
          "upper": "8",
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
      "Check",
      "Middle"
    ],
    "non_dominated": [
      "Check",
      "Middle"
    ],
    "dominated": [],
    "undetermined_pairs": [
      {
        "to": "Middle",
        "reason": "overlapping_intervals",
        "from": "Check"
      }
    ],
    "not_evaluated": [
      {
        "configuration": "Fast",
        "status": "excluded"
      }
    ]
  }
}
```
