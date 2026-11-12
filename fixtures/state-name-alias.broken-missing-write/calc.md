# F5 fault: alias without result writer

Single edit: remove writes approval from A.approve. The selected process binds
approval, but its call no longer writes the same observed variable from Result.
ADR 0012 addendum requires U-SEM-0/error, unsupported/exit4. Do not choose binding
precedence to accept it. No paths/times/clause verdicts are reported: properties
and terminals empty, agency null, all four bounds null. The declared optional
initial none does not make the alias legal. W-CUT-0 is retained as a warning.
