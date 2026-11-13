# 上限つき繰り返し・段階 1 の受入

対象は差し戻しと再提出、条件を待つ再確認。明細と再到着は対象外。
`models/` は新構文、`witnesses/` は別に記述した既存 DSL の経路模型。
手値の正本は `calculations.md`。出力を採取して手値にしてはいけない。

## 実行

リポジトリの根で実行する。実装前の新構文は RED であり、ignore や xfail は使わない。

```sh
CARGO_NET_OFFLINE=true TMPDIR=.tmp CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -p relca-core --test bounded_repetition
CARGO_NET_OFFLINE=true TMPDIR=.tmp CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0 cargo test -p relca-cli --test bounded_repetition
```

core は engine と独立 reference を、それぞれ手値に照合する。reference は
既定の limits を明示して使い、相互一致だけを成功としない。各入力に加え、
旧 DSL の全経路模型と、reference だけ 1 回減らす負対照も実行する。
同時到着する 2 案件の L1 では、共有資源の全ての順序について案件別の時刻・work と全案件の starts を照合する。

CLI の Python 試験を個別に回す場合:

```sh
python3 -B crates/relca-core/tests/bounded_repetition/cli.py --binary target/debug/relca
python3 -B crates/relca-core/tests/bounded_repetition/cli.py --binary target/debug/relca --expanded Behavior Simulation Preservation
python3 -B crates/relca-core/tests/bounded_repetition/calibration.py --binary target/debug/relca
```

`--expanded` は Behavior と Simulation の入力を固定した旧 DSL に切り替える。
Grammar と Gates は常に生の新構文を使う。製品の parser が repeat を受理した
ように見せるモードではない。Preservation は既存 golden の stdout/stderr/exit
も完全一致で検査する。既存 golden を更新する機能は本受入にはない。

`calibration.py` は一時的なローカル CLI proxy で時刻、診断位置、出力 byte、
判定を一つずつ壊し、元の保存試験が拒否することを検査する。production source、
固定入力、期待値を編集せず、一時ファイルはソース木の `.tmp` だけに置く。
`Behavior.test_semantic_fault_pairs` は潜在の独立再抽選、修正無効、回を無視した確率、
最後の gap の四つの模型の故障を、元の分数 assertion が拒否することを検査する。

## 持ち出した手値と公開 API の接続

- `acceptance.py`、`backends.py`、`faults.py` の分数・時刻・starts の assertion は保持。
  `cli.py` は同じ手値を使い、CLI 呼出しと失敗の切分けを unittest にまとめる。
- `backends.py` の追加引数は一つの case を選ぶためだけ。core の新しい test-only
  adapter は公開 parser / lower / engine / reference から JSON 観測を作る。
- 元の adapter にあった private visited の観測は製品の公開 API にはない。
  公開の state budget を二分探索して、同じ全 Outcome を保つ容量の証明を作る。
  `visited` adapter 欄は visited+frontier の必要容量（visited の上界）を入れる。
  元の `0 < visited < 1_000_000` はそのまま、より強い上界で検査する。
  実 visited の測定値や速度の主張はしない。
- round を等値/hash から除く最適化は採らないため、その private 操作を再実装しない。
  公開受入は `Grammar.test_round_unread_public_observations`。round を読まない模型と、
  常に真の観測を足して counter を実体化した模型の終端・確率・時刻・指標・判定を照合する。
- 平均は確率と固定経路から分数で計算する。存在しない metrics の期待平均欄を仮定しない。
  simulate の safety は反例がなければ inconclusive、終端は case_outcomes を読む。
  explain の actor と capability は別欄として照合する。
