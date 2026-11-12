# F13 aobs-reserved.broken-set — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

End only t0/work0 in the scalar control. set<PersonId> remains unsupported;
set<CaseId> is accepted by ADR0035 and has moved to the independent aset group.

Single input transformation: 'state mode: Mode = ok' → 'state keys: set<PersonId>'.

Expected verdict: unsupported; command: check.
RED acceptance is not implementation verification.

Closed migration authorized by answer-a3-tests1. PRE-A3 retains U-SYN-0/4.
Original set<CaseId> source bytes/SHA are archived in caseset_compatibility.json.
Answer-a3-tests3 permits only the primary diagnostic's fixed message/span change:
unsupported syntax: set element type is unsupported, spanning the PersonId
element token. This follows the new set parser's type check and the existing
syntax::unsupported message prefix, not any new executable's output.
