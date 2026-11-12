# A2 — finish snapshot, hand oracle
enabled is false initially, true immediately after A.finish at 2 min,
then false after Reset.finish at 3 min. Only the event snapshot is relevant.
Authority is satisfied; done elapsed/work=2+1=3 min. Start/final reads are wrong.
An auxiliary test adds expires 2min; Reset must not retroactively expire A.
