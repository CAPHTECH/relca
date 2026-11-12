# ADR 0030 F14 単一故障版

正例common_causesはInvoice.difficultyの1行、targets=a,b、instance=per_case。故障は依存1callなので行0。kernelは両難度で同じok、数値は両版不変。Actor instanceと案件間独立の仮定は両版に残る。

単一変更: b = Reader.second() → b = Reader.plain()。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
