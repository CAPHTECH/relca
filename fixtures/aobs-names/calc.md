# F21 aobs-names — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Separate namespaces are accepted only when case names do not collide with observed local names. Collision must be detected independent of declaration order; same short names in distinct Actors remain legal.

Single input transformation: 'case_flag' → 'flag'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
