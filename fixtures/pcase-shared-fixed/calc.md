# ADR 0030 F10 正例

Shared.fixed=trueは未印共有bank、Invoice.difficulty=routineは各local。故障はSharedも各local。確率はどちらも1。common_causesは各潜在1行、targetsはa,bの2つ、case数で倍増しない。Sharedのinstance欄は故障だけper_case。source/grain置換は所有を変えない。

単一変更: actor Shared { → actor Shared { instance per_case;。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
