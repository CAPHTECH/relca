# F1 single fault: long takes six minutes

Exactly one replacement: duration fixed 5min becomes duration fixed 6min.
Serial is 0→3→9, elapsed/work 9. Parallel finishes at max(3,6)=6, work 3+6=9.
All positive paths (the sole singleton-result path, with scheduling interleavings) have those extrema.
done reachable and safety remain satisfied in each plan; root satisfied/exit 0.
The defect is visible in the time values, not a new ranking or violation. result_at stays null.
Old-route normalization and requested ToBeP,AsIs order are as in the correct pair.
