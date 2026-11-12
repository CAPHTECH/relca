# resource-contention: 容量ゼロ

## 単一故障

正解版 resource-contention の capacity 1 を capacity 0 に変えただけ。
共有の語彙で指定された故障で、容量未宣言とは違い E-CAP-0 / invalid にはしない。

## 手計算

単位は分。ADR 0001 の資源規則により 1 スロットを原子的に獲得する必要があるが、容量は 0。
時刻 0 に a と b を生成しても、どちらも開始できない。結果の非決定選択に達する経路は無い。
唯一の停止状態は a 待機・b 待機・process は join all 待ち。作業量は 0 分で、終端時刻は存在しない。
これは計算上の clock 切断ではなく、終端に達しない停止状態なので W-DEADLOCK-0 を期待する。
未完了の状態を経過 0 分の done として計上せず、terminals は空。

占有数は常に 0 で 0 <= capacity 0 だから safety no_resource_overcommit は satisfied。
done に達する経路が 1 本も無く、切断も無いので reachable end.done は violated。
ADR 0002 の verdict 集約により全体も violated。
Cut は undeclared、W-CUT-0 も期待する。
未決: なし。手計算のみで、実装との照合はしていない。
