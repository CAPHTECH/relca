# F06 aobs-start-visible.broken-no-start — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Start case1 at0 publishes busy=true. Case2 arrives2, chooses Alt2..5: completions5,5 elapsed5,3 work5,3. No start effect: case2 waits Desk, runs5..10; completions5,10 elapsed5,8 work5,5. Final busy=false in both. Broken W-UNREACH-0 Alt.other and W-UNREACH-1 then.

Single input transformation: 'start { busy = true };' → ''.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
