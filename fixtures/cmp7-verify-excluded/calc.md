# cmp7-verify-excluded — ADR 0047 U3-verify

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

U3 structural verify constraint:post commits with registered=true,so invariant registered==false is violated in both plans. Check's hard1/50<=1/20 still S,but verify V excludes it. Fast excluded too. No admissible plans,not a work-based rescue. Verify reason precedes hard reason.

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
      "constraint": "cohort.WrongPerArrival <= 1/20",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "1/20",
      "values": [
        {
          "configuration": "Check",
          "verdict": "satisfied",
          "basis": "exact",
          "interval": {
            "lower": "1/50",
            "upper": "1/50"
          },
          "reason": null,
          "source_reason": null
        },
        {
          "configuration": "Fast",
          "verdict": "violated",
          "basis": "exact",
          "interval": {
            "lower": "1/10",
            "upper": "1/10"
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
      "verify_verdict": "violated",
      "hard_verdict": "satisfied",
      "status": "excluded",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "violated",
          "reason": null
        }
      ]
    },
    {
      "configuration": "Fast",
      "verify_verdict": "violated",
      "hard_verdict": "violated",
      "status": "excluded",
      "reasons": [
        {
          "kind": "verify",
          "constraint": null,
          "verdict": "violated",
          "reason": null
        },
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerArrival <= 1/20",
          "verdict": "violated",
          "reason": null
        }
      ]
    }
  ],
  "objectives": [
    {
      "objective": "minimize metrics.overall.work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "Check",
          "lower": "6",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "Fast",
          "lower": "4",
          "upper": "4",
          "basis": "path_range",
          "reason": null
        }
      ]
    }
  ],
  "pareto": {
    "status": "not_computed",
    "reason": "no_admissible_plans",
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
