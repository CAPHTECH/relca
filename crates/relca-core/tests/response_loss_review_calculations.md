# Q1 独立レビュー追従の受入（ADR 0056）

手値は設計 v0.2 §5.6・§9.1、設計 v0.1 §5.4、ADR 0056 の
Command/Task と未解決の区別、取り込み前レビュー裁定から導く。
以下は実行前に固定した期待であり、snapshot や production の採取値ではない。
新しい fixture ディレクトリを増やさず、既存の小模型を明示の一編集で拡張する。
時刻・elapsed・work の単位は分。

## M1: alias を判定・対象にしない

- common: Actor Invoice の潜在 difficulty を二つの check が given に読む。
  c1 に response_loss none を付ける。実試行は1個と c2 の1個、
  仕事は2、done は時刻2。共通原因の対象は実試行と c2 であり、c1 という
  非実行 alias は追加の対象ではない。通常 call 対照では c1/c2 は実行対象。
- judgement: lin-basis に aV/bV をガードとする Effect.post を足した基盤を作る。
  bV に response_loss none、総試行2、fail 時だけ retry、backoff0 を付ける。
  kernel は常に pass なので e/aV/B/Effect は各1回、done は4、work4。
  bV は alias なので lineage/common_causes/model_uncertainties の判定ではない。
  字句上の B の実試行2サイトは残す。basis は独立性の正しさの認定ではない。
- target: 同じ基盤の e に response_loss none を付ける。P/A/B/Effect は各1回で4分。
  bV の不確実性の対象に非実行 e を残さない。普通の call 対照では e は実対象。
- lineage の判定数は字句上の実試行数であって、到達回数や独立源数ではない。
  alias を含む各行を残す選択なら、その行自体に not_derived と
  response_control を伴わせる。Report 全体の注意書きだけでは代用しない。
  alias を除くだけで全行を空にする修正も受け入れない。
- 壊した出力対: 手組みの正しい三表へ alias を判定または対象として一つだけ挿入し、
  検査が拒否することを確認する。行ごとの理由付き not_derived は受理する。

## m1: 消した警告を未検査として開示

W-AGY-0（止めない判定）、W-AGY-2（集中）、W-LIN-2（新しい根拠なしの独立検出仮定）、
W-RESP-2（責務に必要な停止経路なし）は response 制御を解釈しない旧導出から
断定しない。ただし無警告を検査済みと読ませない。各コードと response_control を
同じ未検査記録で結ぶ。既存 assumptions の field/value/affects を使ってもよい。
受入は特定の field 名や日本語の文面を固定せず、同一の小さな記録内に
コードと response_control、unchecked または not_derived の印を要求する。
agency.unchecked の response_control 一語だけでは、どの警告を調べなかったか不明。

通常模型は各警告の実在を先に確かめる対照を持つ。応答模型では4コードの警告を出さず、
理由付き未検査を出す。警告の有無で Core verdict を変更しない。
W-AGY-1 は残った旧構造上の条件付き迂回の注意を隠さないために残す。
W-AGY-3 は保持宣言された fallback の通常経路での未行使の注意であり、
技能や応答下の拒否力を認定しないために残す。両者に正例と一編集の非該当対照を置く。
壊した出力対は、各未検査記録を一つだけ消す、または理由だけを別語に変える。

## m2: 空の via_model_paths と未導出を区別

act-via は Extract(doc@v3) -> Judge -> Ledger の実読取り経路を持つ。
e の call に response_loss none を一つ足すと、実行は引き続き done=5分または
rejected=4分、work も同じ。応答制御のため via_model_paths を出せないなら、
via_model_paths と response_control と not_derived/unchecked を同じ記録に残す。
via model 宣言のない対照でも、response のため未検査にしたなら印を残す。
一方、応答 suffix を外す一編集対では実経路を導き、未導出とは呼ばない。
壊した出力対は未導出記録の削除、理由の削除。通常経路表の空化も拒否する。

## m3: 未解決の受領から未到達の違反を作らない

Check.run の戻り値は pass の一値、所要1分、容量1。
receipt sequence [unknown] の唯一の枝は時刻1に ReceiptUnknown で停止する。
end done は字句上存在するが正常Endは0、completed_at=null、観測work=1、
truncated=false。verify reachable end.done は inconclusive、receipt_resolution も
inconclusive、root inconclusive、CLI exit2。未解決を失敗と決めない。
unknown を delivered にする一編集対は done=1、reachable satisfied、exit0。
同じ模型の戻り値を fail にする別の既知応答対は end rejected=1、
reachable end.done violated、exit1。本当に既知の未到達は引き続き違反。
さらに未知停止に独立の invariant 違反を加える対は root violated/exit1 のまま、
reachable だけ inconclusive。これは他の確定した違反を unknown で消さない対照。
手組みの正しい期待に対し reachable を violated とする単一故障を拒否する。

## 適用範囲

core Report と実CLIの check/explain、engine/reference の reachable をそれぞれ手値へ照合する。
内部の一般的な Agency 全導出、全交差、実サービスの妥当性や全スケジュールの同値証明ではない。
実装の再現結果とRED/GREENは作業記録へ分け、ここには独立に導出した期待を置く。
