# F24 aobs-simulation — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Staggered deterministic starts finish at1 and3; one replication, no Actor state random draw. No witness safety remains inconclusive. Simultaneous two eligible starts are nondeterministic: unsupported with zero completed replications, never randomize scheduler.

Single input transformation: '[0min,2min]' → '[0min,0min]'.

Expected verdict: inconclusive; command: simulate.
RED acceptance is not implementation verification.
