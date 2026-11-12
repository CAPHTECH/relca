# F3 fault: failed approval still posts

Single edit: Approver result set {pass} becomes {fail}. Initial tuple none/false.
Receipt0/start0 preserve it. finish(fail)1 writes checked=fail,approval=fail before
C1, which is false. Ledger.start1 does not alter state. finish(committed)+commit2
writes registered=true; C2=(fail==pass and true==true)=false. Failure monitoring
does not block the next call. done2/e2/w2/result_at=null. Safety satisfied,
C1 and C2 violated, root violated/exit1. No static type or contract error occurs.
