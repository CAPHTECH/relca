# cmp7-under — ADR 0047 T13-under

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T13/U15:under[Day,Spare] is an allowed selector list,not two runs. Both scenarios resolved from original AST. Selected Day keeps T01 numbers. Outside is known but not under.

Expected root: violated.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Check",
      "Fast"
    ],
    "under": [
      "Day",
      "Spare"
    ],
    "scenario": "Day",
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
          "configuration": "Fast",
          "verdict": "violated",
          "basis": "exact",
          "interval": {
            "lower": "1/10",
            "upper": "1/10"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
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
          "configuration": "Check",
          "lower": "6",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Fast",
          "lower": "4",
          "upper": "4",
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
      "Check"
    ],
    "non_dominated": [
      "Check"
    ],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Fast",
        "status": "excluded"
      }
    ]
  }
}
```
