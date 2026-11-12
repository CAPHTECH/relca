# cmp7-clock-loose — ADR 0047 T03L/S-clock

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T03L/S computational counterpart: 26/75<=1/2 implies hard S; 1/75>1/100 implies hard V, even with unknown. Verify I is never admissible.

Expected root: inconclusive.
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
  "hard": [
    {
      "constraint": "cohort.WrongPerArrival <= 1/2",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/2",
      "values": [
        {
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "with_undetermined",
          "interval": {
            "lower": "1/75",
            "upper": "26/75"
          },
          "reason": "truncated",
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "satisfied",
          "basis": "with_undetermined",
          "interval": {
            "lower": "1/75",
            "upper": "26/75"
          },
          "reason": "truncated",
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Check",
      "verify_verdict": "inconclusive",
      "hard_verdict": "satisfied",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    },
    {
      "configuration": "Fast",
      "verify_verdict": "inconclusive",
      "hard_verdict": "satisfied",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        }
      ]
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "no_objectives",
    "among": [],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Check",
        "status": "undetermined"
      },
      {
        "configuration": "Fast",
        "status": "undetermined"
      }
    ]
  }
}
```
