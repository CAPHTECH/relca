# F01 aobs-counter.broken-overflow — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

0 at t0; finish t2:0+1=1; finish t4:1+1=2. Third finish t6 attempts 2+1=3>2; stop without end. No clamp, wrap or computational truncation.

Single input transformation: 'b = Desk.take(); end' → 'b = Desk.take(); c = Desk.take(); end'.

Expected verdict: violated; command: check.
RED acceptance is not implementation verification.
