# eval-vacuous — ADR 0047 T09

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T09:Probe.mark occurs only in Check,after registered=true. K S there;Fast never triggers antecedent, K vacuous. WrongPerRegistered remains1/46 both. Fast is undetermined despite hard S.

Expected root: vacuous.
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
      "constraint": "cohort.WrongPerRegistered <= 1/20",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/46",
            "upper": "1/46"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/46",
            "upper": "1/46"
          },
          "reason": null,
          "source_reason": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Check",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Fast",
      "verify_verdict": "vacuous",
      "hard_verdict": "satisfied",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "vacuous",
          "reason": null
        }
      ]
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "no_objectives",
    "among": [
      "Check"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Fast",
        "status": "undetermined"
      }
    ]
  }
}
```
