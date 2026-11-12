# cmp7-responsibility.broken-no-inspect — ADR 0047 T10-F32

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Delete only To's inspection. Changed responsibility observation row disappears;time_frame not_comparable remains.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "From",
      "To"
    ],
    "under": null,
    "scenario": null,
    "cut": "Scope"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "From",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "To",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "no_objectives",
    "among": [
      "From",
      "To"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
