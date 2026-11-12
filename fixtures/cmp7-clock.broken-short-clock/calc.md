# cmp7-clock.broken-short-clock — ADR 0047 T03C

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

First two expected Wrong=1/25. Last unknown[0,1]. [(1/25)/3,(1/25+1)/3]=[1/75,26/75] crosses3/10. Computational reason truncated.

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
      "constraint": "cohort.WrongPerArrival <= 3/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "3/10",
      "values": [
        {
          "configuration": "Check",
          "verdict": "inconclusive",
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
          "verdict": "inconclusive",
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
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        },
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 3/10",
          "verdict": "inconclusive",
          "reason": "truncated"
        }
      ]
    },
    {
      "configuration": "Fast",
      "verify_verdict": "inconclusive",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "inconclusive",
          "reason": null
        },
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 3/10",
          "verdict": "inconclusive",
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
