# ADR 0030 F12 正例

両版ともD=1。印ありの初期鍵はrelca-sim-v1|seed=7|rep=1|case=1|site=init.actor.Invoice.difficulty|n=1。印なしだけcase=0。確率1でもdraw1、Fixedなら0。値だけでは鍵の誤りは検出できない。

単一変更: instance per_case;  → 。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
