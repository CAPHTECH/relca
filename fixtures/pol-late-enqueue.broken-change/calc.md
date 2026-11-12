# pol-late-enqueue.broken-change

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P21: x1,x2[0,3). If x2 finishes first and y2 starts before x1 finishes, case2 ends5 and case1 ends7; converse also allowed. Range5..7 each, work5. Capacity1 gives x1[0,3),x2[3,6),y1[3,5),y2[6,8). Never compare tasks not queued yet.

The pair changes only 'capacity 2' to 'capacity 1'.
This is the single edit side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
