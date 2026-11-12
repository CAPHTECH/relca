# F10 — two cases, all same-time orders

Each of the 00/01/10/11 complete joint branches closes both cases;
business_ended_open min=max=0, computed on joint paths, not marginal rows.

Arrivals [0,0]. Judge has two executors, so both start at 0 and end at 4.
Both executor assignments are retained; the decision grant covers both.
Each business-end lead/work=4/4. Each party independently declines/files:
the joint choices are 00, 01, 10, 11, without probabilities.

With one filing, route p runs 4..6. With two, it runs 4..6 then 6..8.
At t=4 either case may reach end/file/queue first, despite FIFO tie_by
case_id: the task not yet queued cannot win a FIFO comparison. Thus each
case can be first or second; each row contains responses 6 and 8.
Deadline is 4+4=8 for both, including the waiting task. All are on time.
Declined closes at 4. Filed closes at 6 or 8. No work is added to end.
All properties/root satisfied; no obligations, no truncation.
