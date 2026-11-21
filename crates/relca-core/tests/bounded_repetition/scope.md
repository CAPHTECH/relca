# 選択構成への反復の限定

`models/L2-pair.relca` は作業入力 `l2pair.relca` の写し。
CLI 試験 `scope.py` では metrics 宣言を Hand2 の構成へ差し替えて
入力 `l2triple.relca` と同じ比較条件も作る。

## 手値と保持する契約

- bad は案件の初めに一度だけ true 1/10、false 9/10。
  review は bad のとき必ず fail、それ以外は必ず pass。
- done は 9/10、review 一回で elapsed = work = 1分。
- escalated は 1/10、review 1分 + fix 2分を三回で
  elapsed = work = 3 × (1 + 2) = 9分。
- escalated の確率 1/10 <= 1/5 なので check は satisfied / exit 0。
- Loop に対する mutate は、残す枝に呼出しがある `keep_else` だけを候補にする。diff は両向きで反復の追加・削除を示す。
- compare は両順で Loop の回数表を報告し、Hand のセルは `not_applicable` とする。
- loop 名の衝突は選択にかかわらず E-NAME-0。

## 観測

core の `repetition_regressions.rs` は両 interpreter の terminal、property、
metrics を照合し、選択した IR の制御変数が Hand では0個、Loop では5個と検査する。

CLI は未選択 process Looped の本体を repeat の無い本体へ置き換えた対照と、
stdout の JSON 全体・stderr 全体・exit を照合する。構成は残す。
終端名の字句順（escalated、done）と各行の長さを保つ。
これによりソース位置も model 全体の終端登録順も比較に残す。
正規化は model の name / sha256 だけ。
sensitivity は同じ入力の hash を point.source_sha256 にも公開するため、
その複製だけも除く。manifest hash、入力表、ledger、位置、並び順は比較に残す。
conform は空ログの拒否も含め、同じ入力に対する全出力を照合する。
ログ対応範囲そのものを拡張する試験ではない。

選んだ process だけが loop を持つ場合の Loop check の全 stdout + stderr + exit
の SHA-256 は修正前の実行から固定した回帰検査であり、手計算の代わりではない。
二つの process が repeat を持つ場合は、未選択の本体だけを置き換えた対照と
選択側の出力全体が一致し、repetition 行に選択した loop だけが残ることを検査する。
metrics の既存の `rework_loss` は loop 名 `rework` と別なので、A1 では
JSON 文字列 `"rework"` と `loop_control` の不在を調べる。

L4 から確率の問いを除き、待機1分で done に到達する U を Hand とする派生も検査する。
global kernel K の given は rework.round のまま残す。U は K を消費しないので
安全性は satisfied / exit 0、elapsed 1分、work 0分。未選択 round の型域は
静的宣言 metadata にだけ保持し、実行stateは空である。U から Rev.review を
呼ぶ故障対は U-SEM-0 / loop_round_out_of_scope、unsupported / exit 4。
この2つの verdict / exit / 診断は、全processで制御変数を宣言する旧動作へ
一時的に戻した校正でも観測した。未消費kernelの宣言は捨てず、報告では
内部state名でなく `rework.round` として公開する。
Loop 側の同じ given は従来の `@loop:rework:round` 表示を維持している。

`models/scope-contest.relca` は loop の無い Hand から contest route を起動する故障対。
route の kernel given / requires / atomic が未選択の round を消費すれば
lower で loop_feature_combination / unsupported / exit 4 にする。
宣言専用roundを読まない対では、既存の reversal_authority の違反（exit 1）を保持する。
Lower を迂回した不正IRの実行も、一案件・到着列・標本推定の両backendで
成功やpanicにせず、kernel rowの欠落に関する診断として拒否する。
実行stateに存在する round の atomic による loop 外の読取りは既存のままにする。

## 壊した対

loop 宣言の選択条件を外す故障は Hand の repetition 表と loop_control を復活させ、
`scope.py Selection.test_check_hand` を失敗させる。
名前重複の対は Unrolled に同じ rework 名の repeat を足す。
別の対は既存の binding r と衝突する loop 名を足す。
