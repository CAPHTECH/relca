# F9 — arrivals are not deadline origins

Business_ended_open min=max=0 after all complete leaves close.

Arrival=10. Judge runs 10..14, work 4. End returned=14 (lead time 4).
Declined closes at 14. Filed starts at 14 and finishes at 16; deadline
14+2=16, so runtime/root satisfied. No obligations; arrivals mode keeps
top-level terminals empty and reports business end in case_outcomes.
Absolute contest times are 14/16, not arrival-relative 4/6.
