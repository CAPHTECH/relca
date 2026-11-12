# F6: sequential overwrite

Only initial tuple flag=false. Receipt0, A.start0 leave false. A.finish(ok)1
writes true. B.start1 leaves true, B.finish(ok)2 writes false. Probe.start2 and
finish3 preserve false; C1 at finish3 reads false==false, true. done3, elapsed
1+1+1=3, work3, result_at=null. Safety/C1/root satisfied/exit0. Sequential
writers do not overlap and need no lock. Both writes are constants, not ok.
The pair changes just the first two calls into fork siblings plus join all;
drained calls after a join are not siblings of its branches.
