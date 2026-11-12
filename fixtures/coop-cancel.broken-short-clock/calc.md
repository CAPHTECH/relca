# coop-cancel: clock 不足

## 単一故障

正解版 coop-cancel の clock 4min を clock 3min に変えただけ。
業務上の期限ではなく計算上の切断を早めた故障で、ADR 0001 の境界規則により inconclusive。
共有の語彙は clock 切断専用の診断コードを定めていないので、エラーコードは創作せず、diagnostics には共通の W-CUT-0 のみを要求する。
故障の検出条件は reachable end.done と全体の verdict が inconclusive になること。

## 手計算

単位は分。開始イベント順 a, b / b, a の両経路とも、a は [0, 3)、b は時刻 0 から資源を占有する。
時刻 3 は clock と同じなのでまだ切断されず、a の pass を採用し b への取消要求を出せる。
この時点の累積作業量は 3 + 3 = 6 分だが b は未完了で、process は drain 待ち。
ack は時刻 4 にしか来ず、4 > clock 3 で全経路が切断される。
切断が無ければ b は [0, 4)、経過 4 分・作業量 7 分となるが、この故障版で終端には到達しない。

ADR 0002 により terminals は切断されなかった経路だけを集計するため空。
採用時刻 3 分だけで done を生成したり、部分作業量 6 分を終端の work に入れたりしない。
到達経路が無く切断経路があるので、共有の reachable 規則により violated ではなく inconclusive。
性質はこれ 1 つなので全体も inconclusive。生存 Task の上限 2 は超えない。
Cut は undeclared。未決: なし。手計算のみで、実装との照合はしていない。
