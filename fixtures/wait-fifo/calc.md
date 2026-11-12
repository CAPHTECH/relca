# wait-fifo

ADR 0048; design v0.2 §22.3. Units: minutes.

T01: arrivals 0,1,2. A [0,3),[3,6),[6,9). B [3,8),[8,13),[13,18). Lead 8,12,16 = service8 + queue0,4,8. Each Actor integral6; T18 => 1/3. Wide B starts3,6,9 and ends8,11,14: A integral6/T14=3/7, B0. Predecessor b=3,5,7.

[R] Hand interval arithmetic, not captured collector output. Stage C acceptance stops RED; production fault calibration not performed.
