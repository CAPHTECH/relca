# 既存 DSL の独立した経路模型

新構文の模型とは別に、審査の P / FP / FFP / FFF と再確認の各経路を
既存の call / if / wait / end で記述した入力。新構文を読む adapter ではない。
分数・時刻・回数の根拠は一つ上の `calculations.md`。

L4 と P1 の回数は有限の観測 enum とし、確認の kernel が次の値を書き込む。
その値を次の確認の開始時 snapshot が読む。合格後の enum は外から読まない。
新構文の公開 `NAME.round` の型検査を代用するものではない。
後段の call は成功する各枝の中にあるため、静的な Task 行の数まで同じとは
主張しない。手値に必要な実行された Task 数・時刻・work・待ちを照合する。

32 回版も具体的な既存 DSL で保存し、既定の reference limits のまま検査する。
これらの入力は現在の parser/lower を通す足場であり、生の repeat が通らない
ことを隠すために製品 CLI へ挟む変換ではない。
