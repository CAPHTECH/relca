# B6: 到達不能 call だけを削除

P4 の else にある `y = Extra.run();` だけを削除。intent の 2 主張は残す。
AsIs: Gate.pick 0→1、done=1、work=1。ToBe: Gate.pick 0→1、then done=1、work=1。両側 elapsed=1、result_at=null。
else の仮想経路も今は done=1 だが実行不能。静的集合は両側 actors={Gate}、relations={Gate.pick} で差分無し。
Extra と Extra.run の余剰主張を E-DIFF-0/error で検出。個数は契約にせず包含で照合。
両側 report は satisfied、truncated=false、親 invalid / exit 3。比較可能なので時刻・性質の比較は残す。
