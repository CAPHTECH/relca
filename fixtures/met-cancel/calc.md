# M-cancel

ADR 0019 occupancy ends at acknowledgement, not cancellation request.
F[0,3) returns pass, S starts0. Adopt at3; cancel S at3; ack4 or5.
End/lead4..5, result_at3. Work3+(4..5)=7..8; peak2, waits0, starts1 each.
F utilization3/10, S2/5..1/2. Occupancy3*2+(4..5)*3=18..21;
fees5+7=12; total30..33. done count1, none_end0/no_terminal.
All positive schedules determine extrema; satisfied, no mean or refund.
The explicit end none_end suffix declares the existing if_none target; it does
not change any reachable path or expected value (the adopted test boundary).
