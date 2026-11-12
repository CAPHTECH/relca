# F16 structure is not realized concurrency

Both B={A.a,A.b}; mode sequential→fork for each, exactly2 time rows and
no resource row. Capacity2: Before3+5=8 min; After max(3,5)=5, work8.
W1=W3=2, root0. Single capacity2→1 mutation makes both elapsed8/work8,
but the two time rows remain and resource0 (capacity model-wide shared).
