# 繰り返しのページ受入

`rework.relca` と `poll.relca` は [DSL ガイド](../../../../../docs/guide/dsl.md#繰り返し) の全文例。
試験はこの全文との一致を確かめ、debug または `RELCA_BIN` の実体で check と explain を実行する。
JSON は試験の一時領域に保存し、値を以下の手計算と照合してから表示を検査する。

- Rework: 初回を含め1〜3回。1回で通る確率は1/2、2回で通る確率は1/4。
  3回開始する確率は1/4（3回目で通る1/8と、上限後に進む1/8の和）。
  必ず初回を開始するので0回の確率は0（出力の分布にはこの行も残る）。
  上限後の件数範囲は0〜1、確率は1/8。3回とも不合格なら3×(1+2)+1=10分で期限9分に違反し、exit 1。
  元の呼出し r と c はそれぞれ body の1・2・3回、e は on_limit で round=null。
- Poll: 1回目 pending、2回目 ready。2回を確率1で開始し、1+5+1=7分で終了。
  上限後の件数と確率は0、exit 0。body の s は1・2・3回の site を持つが、3回目の到達を意味しない。
- Poll の clock=6分: 7分の完了前に切断。exit 2、coverage=not_derived、cases=[]。
  未導出を0回・上限到達0件とは読まず、ページと端末の未確認集計に加える。
- Actor の経由モデルは既存の `fixtures/act-via/model.relca` の `Extract` を使う。
  target `Extract.via_model` は `via model "doc@v3"` に対応し、実測・確認済みの申告も人が未確認として表示する。

表示用の不正入力は元の例の一箇所を変更し、Relca の拒否とページの拒否表示を照合する。
入れ子と over、閉じない経路、on_limit 内の again などを正常な流れとして描かない。
