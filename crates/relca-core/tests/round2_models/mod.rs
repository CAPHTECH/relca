pub const START: &str = r#"model StartRange version "0.2" {
type T=enum {ok}
people [p]
observed case {n:0..1=1;hit:bool=false}
latent case {coin:bool ~ {true:0.5,false:0.5}}
actor A {capacity 1;executors [p];capability bad()->T {duration fixed 1min;result nondet {ok};start {n+=1}}}
actor B {capacity 1;executors [p];capability good()->T {duration fixed 1min;result nondet {ok};writes hit;finish {hit=true}}}
process P {fork f {a=A.bad();b=B.good()};join all f;end done}
cut C {include [A,B]}
bounds {cases 1;live_tasks 2;clock 5min}
verify probability reached hit==true <= 0.5
}"#;

pub const FINISH: &str = r#"model FinishRange version "0.2" {
type T=enum {ok}
observed case {n:0..2=1;m:0..2=1;hit:bool=false}
actor A {capacity 1;capability first()->T {duration fixed 1min;result nondet {ok};writes [n,hit];finish {n+=1;hit=true}}}
actor B {capacity 1;capability second()->T {duration fixed 1min;result nondet {ok};writes m;finish {m+=2}}}
process P {fork f {a=A.first();b=B.second()};join all f;end done}
cut C {include [A,B]}
bounds {cases 1;live_tasks 2;clock 5min}
verify reachable end.done
}"#;
