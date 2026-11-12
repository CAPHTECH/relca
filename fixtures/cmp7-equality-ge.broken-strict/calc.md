# cmp7-equality-ge.broken-strict — ADR 0047 T14

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Change only comparison token. Equality violates strict < or >.

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
      "constraint": "cohort.WrongPerArrival > 1/2",
      "cohort": "WrongPerArrival",
      "op": ">",
      "bound": "1/2",
      "values": [
        {
          "configuration": "Good",
          "verdict": "violated",
          "basis": "exact",
          "interval": {
            "lower": "1/2",
            "upper": "1/2"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Bad",
          "verdict": "violated",
          "basis": "exact",
          "interval": {
            "lower": "1/2",
            "upper": "1/2"
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
          "constraint": "cohort.WrongPerArrival > 1/2",
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
          "constraint": "cohort.WrongPerArrival > 1/2",
          "verdict": "violated",
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
