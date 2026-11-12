# M-fifo, one duration change

Only B fixed5 becomes4. A unchanged: [0,3),[3,6),[6,9).
B ready 3,6,9; runs [3,7),[7,11),[11,15); waits0,1,2.
First/a waits0,2,4; ends7,11,15; leads7,10,13.
A work9, B work12; total21, peak2, starts3 each.
Utilization A9/20, B12/20=3/5. Cost18+36+3+12=69.
Each completed case costs23, all-arrival69/3=23. Three completions; min=max.
