# F7 single fault — duration 2 → 3 minutes

Late response still drains: business_ended_open min=max=0 at complete leaves.

Judge ends at 1, work 1, unchanged. Declined closes at 1. Filed starts at
1 and finishes at 4, after deadline 1+2=3. Runtime and root are violated;
static structure stays satisfied. Keep response_at=4 and result pass:
the late task drains rather than disappearing at its deadline. Closed_at=4.
The terminal snapshot remains elapsed/work=1/1. No obligation instances.
