# eval-agency — ADR 0047 T10

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T10:C5 differences pair AsIs->ToBeP. Both have read-only calls a,b with same actors/capabilities, no evidence roots/effects,retention counts1 for each. Differences derived,rows empty. No hard means hard_verdict S;no objectives means no_objectives.

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
    "cut": "C"
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
