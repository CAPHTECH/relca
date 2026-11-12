# F10 aobs-cancel.broken-finish-after-drain — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Positive-duration Probe1. Both start0, Fast finishes1 and selects result_at1. Immediate Slow ack1: started true persists, finished false; Probe1..2 gives elapsed2/work(1+1+1)=3. Forbidden cancel: Slow finishes5, Probe5..6; elapsed6/work(5+1+1)=7; invariant violated. W-UNREACH-2 absent both.

Single input transformation: 'cancel immediate } }\n  actor Fast' → 'cancel forbidden_after_start } }\n  actor Fast'.

Expected verdict: violated; command: check.
RED acceptance is not implementation verification.
