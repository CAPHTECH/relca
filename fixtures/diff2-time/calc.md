# F1: one common capability changes mode

Before B={A.a,A.b}; After B={A.a,A.c}. Intersection={A.a}.
Only A.a changes sequential→fork. Relation rows are c added, b removed,
then time a modified: exactly 3 rows. A remains active, end={done}, no
sources/grants/duties/contest. Before elapsed/work=1+1=2/2 min;
After elapsed=max(1,1)=1, work=2. W1=3, W3=3. Safety satisfied.
Single mutation replaces the fork with sequential calls; time disappears,
two relation rows remain, elapsed/work=2/2. No duration is zero.
