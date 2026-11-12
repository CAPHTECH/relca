# eval-agency.broken-swapped — ADR 0047 T10

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Order-control operation reverses declared and requested plans together;from/to must reverse,not sort.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "ToBeP",
      "AsIs"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [],
  "plans": [
    {
      "configuration": "ToBeP",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "AsIs",
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
      "ToBeP",
      "AsIs"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
