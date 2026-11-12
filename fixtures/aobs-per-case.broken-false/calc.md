# F20 aobs-per-case.broken-false — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Each admission gets local flag=true/answer=none. Probe finishes1,4; elapsed/work1 each. False flag violates both invariants at1,4; no shared(default) assumption for Doc. Explicit per_case only, not grain.

Single input transformation: 'flag: bool = true' → 'flag: bool = false'.

Expected verdict: violated; command: check.
RED acceptance is not implementation verification.
