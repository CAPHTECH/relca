# cmp7-overall — ADR 0047 U9

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

U9:work8,named_people0(no named executors),peak1 serial/2 parallel,cost805,perarrival805. All are path_range,not expected values. Only peak improves for AsIs,so surely dominates.

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
  "objectives": [
    {
      "objective": "minimize metrics.overall.work",
      "direction": "minimize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "8",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
          "lower": "8",
          "upper": "8",
          "basis": "path_range",
          "reason": null
        }
      ]
    },
    {
      "objective": "maximize metrics.overall.named_people_work",
      "direction": "maximize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "0",
          "upper": "0",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
          "lower": "0",
          "upper": "0",
          "basis": "path_range",
          "reason": null
        }
      ]
    },
    {
      "objective": "minimize metrics.overall.peak_tasks",
      "direction": "minimize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "1",
          "upper": "1",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
          "lower": "2",
          "upper": "2",
          "basis": "path_range",
          "reason": null
        }
      ]
    },
    {
      "objective": "minimize metrics.overall.observed_cost_total",
      "direction": "minimize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "805",
          "upper": "805",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
          "lower": "805",
          "upper": "805",
          "basis": "path_range",
          "reason": null
        }
      ]
    },
    {
      "objective": "minimize metrics.overall.observed_cost_per_arrival",
      "direction": "minimize",
      "values": [
        {
          "configuration": "AsIs",
          "lower": "805",
          "upper": "805",
          "basis": "path_range",
          "reason": null
        },
        {
          "configuration": "ToBeP",
          "lower": "805",
          "upper": "805",
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
      "AsIs",
      "ToBeP"
    ],
    "non_dominated": [
      "AsIs"
    ],
    "dominated": [
      {
        "configuration": "ToBeP",
        "dominated_by": [
          "AsIs"
        ]
      }
    ],
    "undetermined_pairs": [],
    "not_evaluated": []
  }
}
```
