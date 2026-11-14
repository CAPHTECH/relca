# 合流と根拠源契約の組合せ（Round 2）

`evidence-independent.relca` の `d` は pass / fail の非決定であり、確率は置かない。
どちらの経路でも Reader.enter は時刻1分に終了し、`merged` の複写は同じ時刻で Task を作らない。
Worker.echo は時刻2分、Judge.assess は時刻3分に終了する。
Judge の引数は合流を経由しない d なので、E(t) = E(d) = {Invoice}。
Basis は satisfied、done の経過・work はともに3分、check は exit0。
Reader.enter の reads [Invoice] だけを削る対は E(t) = {} で violated / exit1。
explain の証跡は binding=t、actor=Judge、capability=assess、required=[Invoice]、
sources=[]、missing=[Invoice]、excluded_edges=[]。参照実行器も同じ手値を満たす必要がある。
無関係な other が合流を読むことを、契約全体の拒否や参照器の異常終了の口実にしない。

`evidence-dependent.relca` は t の引数だけを d から other に変える。
merged → other → t と値が推移するため、段階1は U-SEM-0 / merge_control、unsupported / exit4。
両枝が同じ d を返すことを理由に、段階2の系譜を導出済みと扱わない。
追加対照は merged を直接の引数にする形、direct 修飾、観測変数を介する形も同じ拒否。
合流を条件に使うだけで t の入力根拠が d のままなら、制御依存を根拠に数えず通常判定する。
観測変数への合流由来の書込みが必ず上書きされる場合は、その古い値を根拠に数えない。
