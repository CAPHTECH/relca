# nondet-race

結果の非決定(2 値)と取消応答の時刻(1〜2 分)を、binary の出力まで通して確かめる。どちらかを代表 1 経路で済ませる engine では、終端の集合か範囲が合わない。

## 手計算

単位は分。capacity 2 と work-conserving により a(3 分)と b(5 分)は時刻 0 に開始する。a は取消不可なので必ず時刻 3 に終わる。占有は時刻 3 まで 2 枠、その後は b の 1 枠。

| a の結果(時刻 3) | 採用 | b の扱い | 終了時刻 | 作業量 | result_at | 終端 |
|---|---|---|---:|---:|---:|---|
| pass | a、時刻 3 | 取消要求。ack 4 | 4 | 3 + 4 = 7 | 3 | fast |
| pass | a、時刻 3 | 取消要求。ack 5 と通常終了 5 が同時(両方展開) | 5 | 3 + 5 = 8 | 3 | fast |
| fail | なし | 継続、時刻 5 に pass | 5 | 3 + 5 = 8 | 5 | slow_ok |
| fail | なし | 継続、時刻 5 に fail | 5 | 3 + 5 = 8 | — | rejected |

- ack は要求(時刻 3)から 1〜2 分後(ADR 0001)なので 4 か 5。5 は b の通常終了と同時で、ADR 0001 により両方を展開する。どちらでも終了 5・作業量 8。
- a が pass のとき、採用後の `if a == pass` は真なので fast。a が fail で b が pass なら b を時刻 5 に採用し、`if a == pass` は偽なので slow_ok。どちらも pass でなければ `if none` で rejected。
- `if a == void { end rejected }` は到達しない。`void` は型にあるがどの Capability の結果集合にも無い。`if none -> rejected` の飛び先を `end` 文で定義するために置いた(ADR 0001)。lower はこの分岐を静的に偽と判定するので、rejected は「採用なし」の経路だけを持つ(採用あり・なしの混在は未決 6 により unsupported)。
- fast: 経過 4〜5、作業量 7〜8、result_at 3。slow_ok: 5 / 8 / 5。rejected: 5 / 8 / null(採用なし)。
- 3 終端とも到達できるので reachable は 3 件とも satisfied。同時に占有するのは最大 2 枠で、容量 2 を超えないので safety も satisfied。全体 satisfied。
- 最遅の終了は 5 分で clock 6 分以内、生存 Task は最大 2。切断なし。Cut は undeclared、W-CUT-0 を期待する(ADR 0002)。

検出したい故障: 結果を先頭の 1 値だけにする engine は slow_ok と rejected を落とす。ack を最短の 1 時刻だけにする engine は fast の経過と作業量を 4 / 7 の点にする。

未決: なし。tie nondet(同時刻の複数採用候補)はこの fixture では起きない。engine の単体テストだけが覆う。
