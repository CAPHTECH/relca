# cmp7-clock-strict — ADR 0047 T03L/S-clock

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T03L/S computational counterpart: 26/75<=1/2 implies hard S; 1/75>1/100 implies hard V, even with unknown. Verify I is never admissible.

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
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.WrongPerArrival <= 1/100",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/100",
      "values": [
        {
          "configuration": "Check",
          "verdict": "violated",
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
          "verdict": "violated",
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
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        },
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 1/100",
          "verdict": "violated",
          "reason": "truncated"
        }
      ]
    },
    {
      "configuration": "Fast",
      "verify_verdict": "inconclusive",
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        },
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 1/100",
          "verdict": "violated",
          "reason": "truncated"
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
        "status": "excluded"
      },
      {
        "configuration": "Fast",
        "status": "excluded"
      }
    ]
  }
}
```
