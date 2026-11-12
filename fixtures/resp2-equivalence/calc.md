# F2 同値の展開 — 手導出

対象は (r == false || ok == true)。真理値 00/01/10/11 は 1/1/0/1 で F1 の含意と等しい。
SetR の前00→後10 が破壊。support={r,ok}、required_sources={Record}、p の Guard は直接 reads。
O present、U present、SetR.put/out、Guard.check/j all_paths、bypasses0。O/U/warnings の全構造を F1 と照合する。
これは旧構文なので rules は responsibility-p1-1、時間は unchecked の一鍵だけ。
実行の returned3/done6分は責務を除いても同じ。故障対は比較の false 一語だけを true へ変える。
