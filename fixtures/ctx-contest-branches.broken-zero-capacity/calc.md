# F12 single fault — Route capacity 1 → 0

The declined leaf has 0 business-ended/open cases; the filed deadlock has
1. Summary min=0,max=1,reason=null: this is not a clock/budget truncation.

Judge still ends at 1, returned/work=1. Declined closes at 1. Filed queues
but cannot start and is deadlocked at 1, independently of deadline=3.
Response cannot complete: violated, result/executor/response_at=null;
execution is not successfully closed. W-DEADLOCK-0 folds root to violated.
Static contest structure remains satisfied. No invented late result,
no capacity/executor mismatch (route is anonymous), no clock truncation.
