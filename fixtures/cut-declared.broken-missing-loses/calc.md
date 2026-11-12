# cut-declared.broken-missing-loses

正解版との差分は 1 つ: `exclude Payment` の `loses "支払誤りの検出"` を消した。除外で失われる能力が書かれていない(設計 v0.2 §6.2)。

期待する検出: 警告 `W-CUT-1`。警告なので結果は変えず、時間と作業量は正解版と同じ(経過 8、作業量 8、satisfied)。cut の Payment の `loses` は null。
