# pol-resource

ADR 0036; design v0.2 §22.3. Hand oracle, before implementation.

P05: R intervals x1[0,2),x2[2,4),y1[4,7),y2[7,10). At2 key(0,2) precedes(2,1), so completion7,10 and work5 each. Clock9 cuts case2 without an invented terminal. Resource release at endpoints is half-open.

The pair changes only 'clock 10min' to 'clock 9min'.
This is the positive side. Expected verdict: satisfied; exit 0.
Not an implementation observation. New-policy parsing is expected RED at Stage C.
