# F1: exclusion is monitoring, not cancellation

ADR 0022 D1. Minutes; one deterministic result, no probability.
B starts 0 and finishes 1; A starts 1 and finishes 2. A.finish activates
the prohibition only at 2, so no later B.start exists: excludes satisfied.
End done is 2, work is 1 + 1 = 2. No response exists: obligations is [].
Cut is undeclared, so W-CUT-0 is required, not an error.
The broken pair swaps only the two calls. Expectations precede execution.
