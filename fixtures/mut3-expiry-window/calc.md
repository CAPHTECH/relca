# 時間が進む授権失効の正例

到着0分、approveを0分に開始し1分に終了する。作業量も1分。
元の期限5分は終了1分を含むので authority_basis は satisfied。
変異は G の期限だけを0分にする。終了1 > 0 で violated となり、検出される。

authority_basis が名指す G は1つで、期限は非0なので候補は1。
reads、process guard、2消費者のあるproducer、commit、cohort は無い。
他の対応済みの種類は0候補、規則版と未宣言の品質分母は列挙不可(null)。
従って candidates_total は1。全種の実行では未対応種があるため exit4、
run は complete。変異数を品質や網羅率としない。

壊した対では到着だけを10分に変更する。処理は1分のままである。
