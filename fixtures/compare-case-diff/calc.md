# F7: probability-only new-route difference

Cases arrive at 0,3 with capacity1, fixed1 tasks. Complete at 1,4, elapsed/work1 each,
result_at null; no wait/rejection/truncation. Result writes hit directly (no finish/with collision).
P independent result pairs pp,pf,fp,ff have masses 1/16,3/16,3/16,9/16.
Per case yes/pass hit=1/16+3/16=1/4, no=3/4. Q masses 9/16,3/16,3/16,1/16,
so yes/hit=9/16+3/16=3/4, no=1/4. No nondeterministic effect: min=max.
Query >=0 is satisfied for BOTH cases in BOTH plans, hence diff root satisfied/exit0.
Both raw top-level terminals are [], aggregate verdicts and time values equal; case_outcomes
and property.cases must nevertheless preserve 1/4→3/4. Static relation warnings do not alter verdict.
Terminal union sorted no,yes in each case, both sides present. No subtraction or improvement verdict.
