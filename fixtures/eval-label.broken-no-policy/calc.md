# eval-label.broken-no-policy — ADR 0047 T12

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only FIFO removed;Zulu interval[0,1]/per_case_relaxation I;Alpha S. Preserve declaration order.

Expected root: inconclusive.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Zulu",
      "Alpha"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.Z1 >= 1/2",
      "cohort": "Z1",
      "op": ">=",
      "bound": "1/2",
      "values": [
        {
          "configuration": "Zulu",
          "verdict": "inconclusive",
          "basis": "per_case_relaxation",
          "interval": {
            "lower": "0",
            "upper": "1"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Alpha",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1",
            "upper": "1"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Zulu",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.Z1 >= 1/2",
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    },
    {
      "configuration": "Alpha",
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
          "configuration": "Zulu",
          "lower": "6",
          "upper": "6",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Alpha",
          "lower": "8",
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
      "Alpha"
    ],
    "non_dominated": [
      "Alpha"
    ],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Zulu",
        "status": "undetermined"
      }
    ]
  }
}
```
