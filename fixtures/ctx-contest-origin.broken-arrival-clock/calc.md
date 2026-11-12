# F9 single fault — respond_by 2 → 1 minute

The late route drains; business_ended_open min=max=0.

Arrival=10, judge 10..14, route 14..16 as before. Deadline=14+1=15.
Keep late response/result at 16 and execution close 16. Runtime/root
violated, static structure satisfied. End snapshot and lead/work=4/4
are unchanged. An arrival-based deadline would wrongly fail the positive pair.
