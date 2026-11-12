# F04 aobs-capacity.broken-two-writers — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Shared writer owner capacity must equal one for cases2, independently of actual scheduling. capacity2 is statically unsupported; no runtime count is invented.

Single input transformation: 'capacity 1' → 'capacity 2'.

Expected verdict: unsupported; command: check.
RED acceptance is not implementation verification.
