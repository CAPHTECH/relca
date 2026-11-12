# cmp7-responsibility — ADR 0047 T10-F32

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T10/F32:responsible a1 for Approver.approve. Both consume entry indirectly from Doc;From never directly inspects Doc,so observation missing,required[Doc],direct[],indirect[Doc],missing[Doc]. To calls Approver.inspect reading Doc,so observation present,direct[Doc],missing[]. No within:time_frame remains unchecked/not_comparable on both. These responsibility values must reach C5,not None.

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
