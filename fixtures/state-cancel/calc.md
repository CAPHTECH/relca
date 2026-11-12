# F4: cancellation never writes

Initial changed=false, receipt0. Both Fast and Slow start0; either start order
is possible with independent capacity1. Neither start writes. Fast.finish(pass)1
is adopted at1, result_at=1, then cancellation is requested of Slow at1.
Slow ack2 precedes its normal due5. Request and ack both preserve changed=false;
ack is not finish and publishes no result write. Drain completes2. Probe starts2,
finishes(ok)3; C1 compares false==false, true. done3, rejected is never reached.
Elapsed=3; work=Fast1+Slow(2-0)+Probe1=4; C1/safety/root satisfied/exit0.
The pair changes only Slow cancellation to forbidden_after_start. Additional
tie and initial-true tests distinguish ack, request, normal finish and erasure.
