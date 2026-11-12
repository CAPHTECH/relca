# ADR 0030 F8 正例

未使用かつ0分のCapabilityも禁止。容量不足や実際の呼出し有無に頼らずU-SEM-0。公開IR入口でも禁止する。

単一変更: grain document → capability unused() -> T { duration fixed 0min; result nondet { ok } }; grain document。

期待値は仕様から先に固定した手計算で、実装出力の採取ではない。
時間の単位は分。確率は受付案件当たりであり、拒否を再正規化しない。
RED は未実装の観測であり、意味論の検証完了ではない。
