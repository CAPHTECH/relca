# 文書の案内

- [DSL](guide/dsl.md): 宣言と実行手順の書き方。
- [結果](guide/results.md): exit と JSON の読み方。
- [保証](guide/guarantees.md): 計算の前提と限界。
- [受入対応表](guide/acceptance.md): 39 項目の ID と、fixture・試験名の対応。
- [請求書例](../examples/invoice/README.md): 計算過程と実行例。

`design-sections.txt` は版と節番号だけの索引です。
ソースや手計算にある `v0.1 §18`、`v0.2 §22.1` などの来歴番号を検査します。
索引に本文は含まず、番号が存在することは機能の実装や意味の正しさを保証しません。
受入対応表も名前・順序・根拠の存在を検査するもので、意味の検証そのものではありません。

手計算や試験の T、F、P などの番号は、そのファイル内の入力や検査点を区別するための ID です。
開発時の符号と commit 識別子については [fixture 索引の注記](../fixtures/README.md#開発時の記録について) を参照してください。
機能の対応範囲はガイドと各結果の unchecked を参照してください。
