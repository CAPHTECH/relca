# M-parallel, one slot

Only capacity 2 becomes 1. Both tasks ready at 0.
a first: [0,3), b [3,8), waits (0,3).
b first: [0,5), a [5,8), waits (5,0).
Thus a wait min0/max5, b min0/max3; first wait 0; lead/work=8; peak1.
Utilization=8/(1*10)=4/5. Starts 1 each; cost=800+2+3=805.
Do not sum independent wait extrema into a fictitious path. Satisfied.
