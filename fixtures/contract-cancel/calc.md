# F9: cancellation is neither finish nor commit

Independent ADR 0010 hand oracle, minutes. received0; Fast/Slow start0 in either order.
Fast.finish(pass)1 -> adopt fast1 -> request cancel Slow1 -> ack2 -> end.done2.
Slow natural finish3 never occurs. ack is not a finish/commit: #1/#2/#3 have no
antecedent anywhere, so all vacuous. own slow result is read only on normal finish,
which is statically legal but dynamically absent here. Safety satisfied, root vacuous/exit5.
elapsed2, work Fast1+Slow2=3, result_at1; no cut, rejected unreachable.
Pair changes only Slow cancellation policy; no exemption or business end invented.
