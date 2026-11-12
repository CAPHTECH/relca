# F4 single fault: G loses only q's grant

Officer executors remain p,q; both Permission entries remain. G grants only p.
Static all-executor containment fails: E-CON-0/error, invalid/exit3.
No exploration, properties/terminals empty, all four bounds null, obligations
absent (not []). The execution skeleton without the new contract is legal.
Undeclared Cut still emits W-CUT-0 on static failure, as on the old binary.
