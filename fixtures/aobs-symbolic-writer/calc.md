# F12 aobs-symbolic-writer — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Actor permit remains Unknown even after own prepare result writer. Case storage uses existing unique serial result substitution and has one reachable refusal derivation; Actor has zero. Both runtime finish2/work2.

Single input transformation: 'actor A { capacity 1; state permit: optional<R>;' → 'observed case { permit: optional<R> }; actor A { capacity 1;'.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
