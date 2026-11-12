# Q9 merge 前レビュー受入の手計算

production 基点 cd5a7074。ADR 0063 の 3.2 節・3.3 節・4.2 節と
Q9 review 裁定 MJ1・m1〜m4・U1・U2 を対象とする。U3 は変更しない。
新しい入力・期待のみを追加し、旧固定ファイルから実出力を採取しない。

## MJ1: 範囲停止

fixed の Judge は 1 分、対象 returned は時刻 1、起票 p=1、route は 2 分、
期限は時刻 4。各々 16 反復。整数 n は 0..1、初期値 1、finish の加算は
1+1=2>1。この試みは書込みも通常終了も成立させず、計算 clock と別に停止する。

| 入力 | 停止時刻 | target_observed / not_reached / unknown | filed / unresolved / failed | 率 | 時間3欄 |
|---|---:|---|---|---|---|
| range-before-target: Judge.finish で加算 | 1 | 0 / 0 / 16 | 0 / 0 / 0 | null、numerator=0、denominator=0、unfinished | 全数値null、samples=0、unfinished |
| range-in-route: Reviewer.finish で加算 | 3 | 16 / 0 / 0 | 16 / 16 / 0 | 1、numerator=16、denominator=16、reason=null | 全数値null、samples=0、unfinished |

その他の構造件数は 0。target 前の停止は不達の証明でない。route 停止は
失敗応答でも通常応答でもない。既知の対象/起票率は ADR の A7 と同じく保つ。
range_stops は16、truncated_replications は0、state_in_range witness により
root violated/exit1。各反復の target_known/failed/closed_at と、公開集約を別に照合する。
reference は公開の反復観測を手値に照合する。集約器は公開 API でなく、
reference 集約を再実装して production 集約の検査を済ませたふりはしない。

## m1: Permission の違反と未完了

Reviewer の唯一の実行者 r に対し permit j のみ。p=1、対象/開始は時刻1、
正常終了予定3、clock2。開始できる実行者と Permission の検査は別なので、
起票16・開始16・応答0・unresolved16・route_failed0 を期待する。
この入力が実行前に拒否されるなら探索記録だけとし、通らない runtime を要求しない。

## m2: decoder の意味対

手組みの正対は trials16。known は対象16・起票16・応答16、
unresolved は対象16・起票16・応答15・未解決1、target_unknown は対象15・
対象不明1・起票15・応答15。その他0。各分割の和を保つ。
通常応答の service=2、queue=0、elapsed=2。未知のある正対の時間は unfinished/null。
負対は時間3欄だけを通常応答15本の数値へ置き換える。件数恒等式・平均加法は
正しいままなので、別の形のエラーで落ちたことにしない。
逆向きは未知0の known の時間だけを truncated/null または unfinished/null にする。
別対では時間を変えず、率の値だけを null と truncated/unfinished に変える。
未知0なのに率まで撤回する組も拒否する(:271 の境界)。
正常の数値時間と、未知があるときの理由付き null の正対を併置する。

## m3: check の開示

orch 追加裁定: rates 付き check に field=metrics.contests、value=not_provided が
ちょうど1行、旧 rates 無し check に0行。affects の文言と simulate の有無は固定しない。
両 check に標本表を生成しない。意味検索や実装からの語の採取はしない。

## m4: 旧遭遇と期限加算

旧 rates 無しで target_at=1、respond_by=18446744073709551615 (u64 MAX)。
加算は桁あふれするが、標本で先に遭遇する未解決選択によって全体を拒否する。
unsupported/exit4、completed0、truncated=null、U-SEM-0 の非決定遭遇を期待。
桁あふれだけによる同じ exit4 では合格しない。

## U1: End 時の未開始 Task

一案件、時刻0で fork の a が requires を満たさず拒否、値 denied が勝者になる。
容量0の別 Actor B の b は未開始。cancel immediate の b は取消され、
drain 後の End done は時刻0。b.starts=0、cancelled_before_start=1、work=0、
clock 切断0、deadlock0。16反復とも同じ。存在的 reachable End の witness があり
root satisfied/exit0。cancelled な未開始 Task は履歴に残る。
非terminal の Queued を残す End に到達しないことには、構文上の根拠も必要である。

## U2: 他案件の live_tasks

到着0/1、1反復。Judge の一値結果を K={fail:1/2,pass:1/2} にする。独立 SHA256
計算で seed5/rep1/kernel.Flow.d/n1 は case1 が前半、case2 が後半に入る
(K の枝の宣言順 fail,pass の CDF。lower.rs:3445-3452 は宣言順を保持)。
case1 は Judge[0,1)→fail→wait2→時刻3にreturned。case2 は Judge[1,2)→
pass→wait1→時刻3に二 Task を fork する。live_tasks1 では2>1、control は上限4。
case1 の End に遭遇した時点で、次の案件の管理より先に旧異議の選択を拒否する。
両入力で全標本 unsupported/exit4、completed0、truncated=null。
reference が case2 の fork を先に処理して inconclusive にしないことを守る。
率付きの表は旧模型へ追加しない。
