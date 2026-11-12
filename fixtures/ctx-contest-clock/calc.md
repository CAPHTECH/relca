# F8 — clock equality

All leaves close; business_ended_open min=max=0.

Judge end=1, work=1. Declined closes at 1. Filed starts at 1 and finishes
at 3, exactly both clock and deadline. Finish is processed, not cut off.
Runtime/root satisfied, truncated=false. End remains 1/1; obligations=[].
Same-time finish/clock handling must not change equality to a violation.
