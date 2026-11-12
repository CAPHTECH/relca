# F3: specified Authority and capability

Hand oracle from ADR 0011; units min. Runner=p or q (two assignments).
Receipt0 → start(person)0 → [normal finish(person,ok),commit(person)] bundle1 → done1.
Permission={p,q}. Grant's run grants union={p,q}; Controls #1 activates at
normal finish1 and satisfies for both people. Safety/root satisfied, exit0;
e1/w1/result_at null, no truncation, W-CUT-0. issuer and excluded are distinct
from all grantees, so there is no not_by contradiction. Explicit permit suppresses
Runner.permission default, named pool suppresses anonymous executor assumption.
