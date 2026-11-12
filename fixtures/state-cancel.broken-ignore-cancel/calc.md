# F4 fault: cancellation ignored after start

Single edit: Slow.cancel cooperative ack_within1 becomes forbidden_after_start.
Initial changed=false. Receipt0; Fast/Slow start0 in either order. Fast.finish1,
adopt1, cancellation request1 leave changed=false. Slow ignores it and normally
finishes(ok)5, so changed=true before its finish bundle. Probe starts5/finishes6;
C1 reads true==false, false. done6, rejected never reached. elapsed6,
work=1+5+1=7, result_at1. Safety satisfied, C1/root violated, exit1.
A cancellation request alone must not suppress a later normal-finish write.
