# F2 single fault: delete permit q row

Hand oracle, ADR 0011. Units min. Assignments remain p and q: Permission is a
monitor, not a start guard. Receipt0 → start(person)0 → finish(person,ok)1 → done1.
Permission={p}; p path satisfies Controls #1 at0, q path violates it at0.
Both complete e1/w1/result_at null. Aggregate permitted/root violated, exit1;
safety satisfied, not truncated. W-CUT-0, no E-CON-0. Runner.permission default
is still absent because its only capability has an explicit permit.
This catches filtering unauthorized people or using executor set instead of explicit Permission.
