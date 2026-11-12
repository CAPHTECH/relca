# ADR 0030 F11 正例

DSLは合法な別case変数scratchへの書込み。公開IR試験でkernel_writesと全枝overridesをInvoice.difficultyへ付け替える。routine=true相当、difficult=false相当とする。正例は全枝difficult、故障は全枝routine。各初回ok=1、正例中間(D,R)、最終(D,D)、各reached D=1。故障は常に(R,R)、reached D=0。2回目givenは正例bad、故障ok。fixture CLIの数値はscratch、IRの数値は専用試験で別に照合する。修飾DSLの受理を主張しない。

単一変更: scratch = false → scratch = true。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
