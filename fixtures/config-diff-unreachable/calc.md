# P4: 動的に到達不能でも静的集合に含める

手計算、分。AsIs: Gate.pick 0→1、done=1、elapsed/work=1/1。
ToBe: Gate.pick 0→1、x=pass、then の done=1、elapsed/work=1/1。result_at は両方 null。
else は実行不能。もし else に入れば Extra.run 1→3、work=3 だが pass のみのモデルでは到達経路に数えない。
静的 walker は else も走査するので actors={Gate}→{Gate,Extra}、relations={Gate.pick}→{Gate.pick,Extra.run}。
added actor Extra ("fallback actor") と relation Extra.run ("fallback step") の 2 件、loses=null。
両側 reachable done/safety は satisfied、truncated=false。親 satisfied / exit 0、diff 診断無し。
