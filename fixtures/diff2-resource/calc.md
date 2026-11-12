# F2 active resources, not declared resources

Before B=A=empty. After B=A={A.a}, owner={A}. Therefore actor A,
relation A.a, authority A.a and resource A are added, exactly 4 rows.
Authority A.a: no permit, so Permission = A's executors {q,p} -> [p,q]; no
grant, basis or duty. Permission is not null, so the value is not null:
from=null, to={permission:[p,q],grants:[],authority_basis:[],responsibilities:[]}
(design v0.2 §7.2 "Permission・Authority・責務"). Resource from=null,
to={capacity:2,executors:[p,q]} (sorted names, declaration order q,p).
No common capability means no time row. Before end at0/work0, After1/1 min.
W1=W3=4. Remove only After's call: all sets equal empty, differences0,
elapsed/work0 on both sides despite the unchanged Actor declaration.
