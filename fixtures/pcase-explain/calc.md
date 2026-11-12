# ADR 0030 F13 正例

正例routineの4/5枝でinvariant違反、故障はdifficultの1/5枝。どちらもviolated。explainはconfigurationなし、最短反例1件、case付き値とfinish位置と独立再生verifiedを検査。jointの非一様枝でcase1/2を別に再生するのは内部再生の受入義務。

単一変更: obs == difficult → obs == routine。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
