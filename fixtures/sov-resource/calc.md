# sov-resource

Hand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.

P26: Different people c1/n1 isolate resource R capacity1. With both uses: route first End6/response4 or notify first End5/response6. Route use removed: both start3, End5,response4. R occupied3 units-min vs2 on posted filed branches (whole range0..3/0..2).

Commit finish → same-tick filing/queue management → ordinary process/End → dispatch. Filing is nondeterministic, not a probability. Route work is excluded from the business terminal and included in occupation/overall work. A business End is not execution close. Existing responsibility/completion warnings are not errors. RED is not semantic verification.
