# eval-label — ADR 0047 T12

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T12:Relabel One->Zulu,Two->Alpha,OnTime->A0,ratio->Z1,A->Desk/grain team. Physical durations/capacities/rates unchanged, so positional1/2,1 and6,8,non_dominated[Zulu]. Undo the five-name bijection;only old tables.metrics rows may be restored to the frozen pre-rename key order. Assert that this permutation actually occurs. Other arrays,especially declared plans,evaluation,admissibility and Pareto,are never sorted. Only the explicit grain,source hashes and four independently enumerated diagnostic coordinates are projected;all remaining bytes must match.

Expected root: satisfied.
Closed evaluation:

```json
{
  "rules": "compare-evaluation-p1-1",
  "declared": {
    "plans": [
      "Zulu",
      "Alpha"
    ],
    "under": null,
    "scenario": null,
    "cut": "C"
  },
  "hard": [
    {
      "constraint": "cohort.Z1 >= 1/2",
      "cohort": "Z1",
      "op": ">=",
      "bound": "1/2",
      "values": [
        {
          "configuration": "Zulu",
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
          "configuration": "Alpha",
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
      "configuration": "Zulu",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Alpha",
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
          "configuration": "Zulu",
          "lower": "6",
          "upper": "6",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Alpha",
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
      "Zulu",
      "Alpha"
    ],
    "non_dominated": [
      "Zulu"
    ],
    "dominated": [
      {
        "configuration": "Alpha",
        "dominated_by": [
          "Zulu"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
