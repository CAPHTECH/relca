# eval-agency.broken-implicit-cut — ADR 0047 T10

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

Only optional declared cut is omitted. Existing selected plan Cut C remains compatible;declared.cut is null,not silently filled.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "AsIs",
      "ToBeP"
    ],
    "under": null,
    "scenario": null,
    "cut": null
  },
  "hard": [],
  "plans": [
    {
      "configuration": "AsIs",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "ToBeP",
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
      "AsIs",
      "ToBeP"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
