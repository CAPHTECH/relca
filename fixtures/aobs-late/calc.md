# F09 aobs-late — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Shared initial0 occurs once before t0. Case1 posts at1/work1. Case2 arrives3, reads current1, refused at3/work0. Reset-on-admission mutation instead posts at4; final1 alone cannot detect it. Input counterpart removes requires and attempts2 at4.

Single input transformation: '      requires posted == 0 else rejected\n' → ''.

Expected verdict: satisfied; command: check.
RED acceptance is not implementation verification.
