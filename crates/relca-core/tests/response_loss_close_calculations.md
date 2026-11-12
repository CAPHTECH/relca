# Q1 閉じの手計算（ADR 0056、2026-11-02）

## 混在した到達性

response_loss_close_inputs::MIXED は1案件・1Task・所要1分、結果は既知の fail のみ。
受領チャネルは非決定であり確率は付けない。

- delivered の経路: fail を読んで rejected に時刻1で到達。elapsed=work=1。
- unknown の経路: サーバの実Taskは時刻1で終わるが、bindingは公開せず ReceiptUnknown。
  end/completed_at/elapsed/work は null、observed_until/observed_elapsed/observed_work は1。
- done への既知の到達witnessは0、しかし unknown 経路の未完了を既知不達へ変換しない。
  reachable end.done と receipt_resolution は案件・全体とも inconclusive、root inconclusive、exit2。
  通常のterminalは rejected のみ、案件別terminalは rejected と receipt_unknown の2種。
  clock5には到達しないので truncated=false。coreの両探索器と実CLIのcheck/explainで照合する。

## alias除外後の空欄

SINGLE_COMMON は潜在 Invoice.difficulty を読む実Taskが1つだけ。
旧グラフでは実試行と採用binding v の2行が候補となるが、vは追加の実行でない。
v除外後は対象1件のため共通原因行0件、元導出と同じ common_causes=None（wireでは欄なし）。
通常callの対も共通原因なし。これは共通原因が不在であることの一般的証明ではない。

model_uncertainties は旧lin-basis入力から導いた bV alias の抑止行1件だけを境界へ渡す。
alias除去後の判定行0件を None に正規化する。合成境界であり、この行集合だけを持つ
自然なDSL入力の存在は主張しない。実試行の抑止行がある旧受入はそのまま残す。

## 未確認だった境界

- report.agency=None を明示的に与え、旧導出の非空via経路を持つ境界を検査する。
  Agencyの有無にかかわらず via_model_paths=None と not_derived(response_control) の印が必要。
- W-LIN 系は現在 W-LIN-2 だけ。通常のno-basis対照で bV を含む警告が実際に生じ、
  応答制御では当該警告を抑止しコード別の未検査印を残す。文字列から名前を削るだけではない。

試験は旧期待を緩めず追加する。snapshotは別の回帰であり、この手値の代わりにしない。

## snapshotへ加える既存正例3種

入力は既存 response_loss_inputs の g-unknown / g-probability / max-thirty-two を
空白以外同一で保存し、CLIの response_snapshot_inputs が対応を検査する。
元の手計算・独立oracleは response_loss_retry_calculations.md と同名受入に残す。

| 正例 | 手値と分類 |
|---|---|
| g-unknown | unknownを2回。実試行2・commit1・時刻2・work2、正常End0。ReceiptUnknown、inconclusive / 2 |
| g-probability | p=1/10の条件付き独立喪失。posted=9/10、duplicate=9/100、ReceiptUnknown=1/100。commitは全経路1回、未解決質量を成功にしないのでinconclusive / 2 |
| max-thirty-two | bad=1/10の同じ初期潜在を全試行で共有。正常9/10は1分でok、bad1/10は32分・32試行でfailed。failed確率1/10は上限1/10を満たしsatisfied / 0。時計40で切断なし |
