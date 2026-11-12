# ADR 0030 F17 単一故障版

同じ案件は一度だけ引いたDを2回読む。正例DD=1/5、不一致0。故障は第2結果だけ独立抽選となりDD=(1/5)^2=1/25、不一致2(1/5)(4/5)=8/25。未観測right=noneを不一致に数えない。案件間のDD=1/25とは別。

単一変更: kernel L given [Invoice.difficulty] { routine -> { 1: routine with right = routine }; difficult -> { 1: difficult with right = difficult } } → kernel L { * -> { 0.8: routine with right = routine; 0.2: difficult with right = difficult } }。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
