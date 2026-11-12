# sov-one-site.broken-two-sites

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P08: Single edit 'if approval == fail { end returned } else { }; ack = Ledger.post(); end posted' → 'if approval == fail { x = Ledger.post(); end returned } else { ack = Ledger.post(); end posted }'. The exclusive arms contain two lexical sites, even though no run commits twice. Reject before exploration.

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
