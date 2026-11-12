# cut-declared.broken-excluded-call

正解版との差分は 1 つ: `b = Office.long()` を `b = Archive.store()` にした。Cut の外に置いた Archive の Capability を process が呼ぶ。除外したものに結論が依存する。

期待する検出: `E-CUT-1`、全体 invalid。性質を評価せず、終端も出さない。`truncated` は null、cut は `"unresolved"`、`W-CUT-0` は出さない(ADR 0007)。
