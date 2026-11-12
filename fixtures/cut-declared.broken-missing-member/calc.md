# cut-declared.broken-missing-member

正解版との差分は 1 つ: `exclude Archive …` の行を消した。Archive は宣言済みの Actor なのに、include にも Actor の exclude にも無い(Cut 漏れ、設計 v0.2 §6.2)。

期待する検出: `E-CUT-1`、全体 invalid。静的エラーなので性質を評価せず、終端も出さない。`truncated` は null。cut は書かれているが解決できなかったので `"unresolved"`、`W-CUT-0` は出さない(ADR 0007)。
