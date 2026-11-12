# cmp7-sim-zero — ADR 0047 T18

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

T18:R1,Registered hits0 =>lower0,not proof p=0. Interval null/denominator_may_be_zero. Wrong hits0 too,point0/0=null. Source zero_denominator is preserved in source_reason;the CP evaluation reason is denominator_may_be_zero.

Expected root: inconclusive.
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
      "constraint": "cohort.WrongPerRegistered <= 1",
      "cohort": "WrongPerRegistered",
      "op": "<=",
      "bound": "1",
      "values": [
        {
          "configuration": "Good",
          "verdict": "inconclusive",
          "basis": null,
          "interval": null,
          "reason": "denominator_may_be_zero",
          "source_reason": "zero_denominator",
          "point": null
        },
        {
          "configuration": "Bad",
          "verdict": "inconclusive",
          "basis": null,
          "interval": null,
          "reason": "denominator_may_be_zero",
          "source_reason": "zero_denominator",
          "point": null
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Good",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1",
          "verdict": "inconclusive",
          "reason": "denominator_may_be_zero"
        }
      ]
    },
    {
      "configuration": "Bad",
      "verify_verdict": "satisfied",
      "hard_verdict": "inconclusive",
      "status": "undetermined",
      "reasons": [
        {
          "kind": "hard",
          "constraint": "cohort.WrongPerRegistered <= 1",
          "verdict": "inconclusive",
          "reason": "denominator_may_be_zero"
        }
      ]
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "simulation_estimates",
    "among": [],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": [
      {
        "configuration": "Good",
        "status": "undetermined"
      },
      {
        "configuration": "Bad",
        "status": "undetermined"
      }
    ]
  },
  "statistics": {
    "interval_method": "clopper-pearson-per-case-sum",
    "multiplicity": {
      "method": "bonferroni",
      "m": 4,
      "alpha": "1/16"
    },
    "confidence": "1/2"
  }
}
```
