# 反復の Agency と書き下しの照合

`repeat_agency.rs` は下記を手値で検査する。各回の呼出しは明示した対応で置換する。元の呼出し箇所を数える欄だけ、Hand の回別行を元の箇所へ束ね直す。公開 report が `loop_control` で伏せる値は伏せたままを期待する。`unchecked` の `state_guards` と `loop_control`、Loop の規則名 `agency-p2-loop-1` は Hand との既知の差。固定 `gap` を `wait` に展開した Hand の規則は `agency-p1-requires-1`。

## CheckAtEnd（上限2）

- `Reviewer.check` の第1回は最初の `Lead.split`、並行する `Dev.refine_a` / `Dev.refine_b`、`Lead.integrate` の後。第2回は第1回の審査が fail の場合だけ。同じ Reviewer（rev）が審査する。
- 各審査の `p` は `Lead.split` を経た Spec、`x` は Dev の2枝を統合した結果で、両枝とも Spec に由来する。Reviewer が直接読む原本も Spec。したがって両方の審査の subject は Spec、別の独立源は0。Hand の `a`、`b`、`x`、`v` は各回の系譜4行、元の4 site・3 Actor。Loop では反復を通った拒否の解析を行わないため、条件に現れる `v` だけを各回の系譜に残す（元の1 site・1 Actor）。観測の段は各回 `p/a/b/x/v/r` の6行、元 site に束ねると6行。
- `Lead.release` には第1回の pass、または第1回 fail と第2回 pass の2経路がある。各経路には Reviewer の判定があり、`v1` が第1回の release を、`v2` が第2回の release を全経路で止められる。第2回の経路は第1回が fail であることも条件に含む。release の回数は0–1、review は1–2。結果の別名 `r` には Task も拒否・効果の行もない。

## RoundGate（既知の差を固定する対）

初回は `NAME.round == 1` が真で、審査なしに1分待つ。第2回にだけ Rev が `Invoice` 由来の `entry` を審査する。fail が post を全経路で止め、迂回は0。失敗終端は経過3・作業2分、合格終端は経過4・作業3分。実行上、判断は `r2` だけで独立源はない。

ただし Agency は利用者の回数条件を両側に展開する。Loop は実行しない第1回の系譜行を残し、拒否は `state_guards` として未検査。Hand は待ちと第2回だけを明示するため系譜は1行、拒否は `all_paths` / 迂回0と導出する。この差は今回揃えない。

## 経路数と互換性の対

各回に利用者が書いた `if` が k 個あり必ず全 n 回を通り、反復後にも `if` が一つある対では、初期経路と各分割を合わせて `2^(nk+1)`。経路上限4096なので `nk+1 ≤ 12`、n が1・2・3なら k の最大は11・5・3。条件を毎回同じ結果から読んでも、利用者の `if` は簡約しない。

`bounded_repetition/review_compat.py` の G〜G9 は基点との3チャネル互換性を固定する。G3 の書き下しでは各回で審査1分・二次審査1分、差し戻しはさらに2分。最後の二次審査で止まる経路と、post が1分加わる経路がある。G4 は後続の分岐ではなく post の引数に結果を渡す対。F は反復の前の2判断が両方 fail のときだけ commit を止める。片方が pass なら他方は迂回されるので `r0` と `c0` の双方が `conditional`・迂回1。R の反復側では責務の拒否を公開しないので `responsibility_refusal` を残す。

## M3 の変異

- `evidence_missing`: Reader.enter の Invoice の読取りを外しても、割り当てられた判定に悪化は出ない。反復の系譜に複数回の審査を出しても根拠の独立性は増えない。`undetected / no_worsening`。
- `summary_reused`: 変異の比較に使う元の site の系譜がなく、ほかに割り当てられた検査はない。`undetected / no_assigned_check`。
- `refusal_path_cut`: guard 形で `keep_else` を選ぶと、不合格の修正と差し戻しを取り除き、最初の fail が `Ledger.post` に届く。`reviewed == pass` の invariant が破れるため `detected`。通常形の逆向き（全回で差し戻す）は post がなく、この invariant は空虚。公開しない過程の拒否に依存するため `unsupported / loop_control` のまま。
