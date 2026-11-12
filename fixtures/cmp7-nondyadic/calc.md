# cmp7-nondyadic — ADR 0047 F14-public

Hand oracle, authored before implementation. Stage C RED is not verified semantics.

F14 public aggregate. R4,seed2,fair initial correct distribution,no later writes. ADR0016 SHA256 init.case.correct keys yield first64-bit integers 13870960371742070810,16087836130209217323,926088661298817727,4275795123798739053. Exactly two are below2^63 and two above,so Wrong hits2,unknown0,trials4 in BOTH plans;point1/2. m2,confidence3/4 =>alpha1/16. G(2;p)=6p^2-8p^3+3p^4;at D=2^20 the root lies between k=115452 and115453. Outward CP[28863/262144,233281/262144]. Upper<9/10,both hard S. The independent BigInt polynomial test certifies the lattice without calling production interval().

Expected root: satisfied.
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
      "constraint": "cohort.WrongPerArrival <= 9/10",
      "cohort": "WrongPerArrival",
      "op": "<=",
      "bound": "9/10",
      "values": [
        {
          "configuration": "Good",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "28863/262144",
            "upper": "233281/262144"
          },
          "reason": null,
          "source_reason": null,
          "point": "1/2"
        },
        {
          "configuration": "Bad",
          "verdict": "satisfied",
          "basis": "statistical",
          "interval": {
            "lower": "28863/262144",
            "upper": "233281/262144"
          },
          "reason": null,
          "source_reason": null,
          "point": "1/2"
        }
      ]
    }
  ],
  "plans": [
    {
      "configuration": "Good",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    },
    {
      "configuration": "Bad",
      "verify_verdict": "satisfied",
      "hard_verdict": "satisfied",
      "status": "admissible",
      "reasons": []
    }
  ],
  "objectives": [],
  "pareto": {
    "status": "not_computed",
    "reason": "simulation_estimates",
    "among": [
      "Good",
      "Bad"
    ],
    "non_dominated": [],
    "dominated": [],
    "undetermined_pairs": [],
    "not_evaluated": []
  },
  "statistics": {
    "interval_method": "clopper-pearson-per-case-sum",
    "multiplicity": {
      "method": "bonferroni",
      "m": 2,
      "alpha": "1/16"
    },
    "confidence": "3/4"
  }
}
```
