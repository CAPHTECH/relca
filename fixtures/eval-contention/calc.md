# eval-contention — ADR 0047 T02

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T02: FIFO completions One3,6 and Two4,4. Inclusive deadline4 gives (1+0)/2=1/2 and (1+1)/2=1. work6<8. Both hard S;One dominates.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "One",
      "Two"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.OnTimePerArrival >= 1/2",
      "cohort": "OnTimePerArrival",
      "op": ">=",
      "bound": "1/2",
      "values": [
        {
          "configuration": "One",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/2",
            "upper": "1/2"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Two",
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
      "configuration": "One",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Two",
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
          "configuration": "One",
          "lower": "6",
          "upper": "6",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Two",
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
      "One",
      "Two"
    ],
    "non_dominated": [
      "One"
    ],
    "dominated": [
      {
        "configuration": "Two",
        "dominated_by": [
          "One"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
