# wait-combined

ADR 0048; design v0.2 §22.3. Units: minutes.

T05: case1 a0..3,b0..5 End5; case2 a3..6,b5..10 End10. Case2 [0,3) has two different causes => combined3, service[3,10)=7. Task waits a3/b5; actor integrals3,5 /T10 =>3/10,1/2. Control replaces resource serialization with Actor serialization, same intervals, case actor3.

[R] Hand interval arithmetic, not captured collector output. Stage C acceptance stops RED; production fault calibration not performed.
