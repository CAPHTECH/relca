# 反復変異の手値

`M3.relca` は ADR 0080 の M3。分岐を残す向きは1箇所につき2件であり、3回分の候補は作らない。

| 形 | 捨てる枝と後続 | 書き下し側ですべての写しに適用する編集と判定 |
|---|---|---|
| M3 | `again` を捨てる方向と `leave` を捨てる方向。前者にも後続の `Ledger.post` がある。 | 全回で即時終了なら、最初の fail も post に届くので Guard は violated。全回で差し戻すなら、3回で escalated し Guard は vacuous。 |
| a | 捨てる枝が空の `keep_then` は作らない。`keep_else` には後続の修正と post がある。 | 変異前は、入力 1 分と審査・修正 3 分×回数に、合格時だけ post 1 分を加える。`done` の経過・作業は 5–11 分、`escalated` は 10 分。post は合格時だけなので Guard は satisfied。早い差し戻しを取り除くと、初回の fail でも修正から post に進む（5 分）。修正は reviewed を書かないので Guard は violated になり、変異は detected。 |
| b | `leave` を捨てる `keep_else` だけ。`again` が if の後ろにあることは、捨てた `leave` を見落とす理由にならない。 | 3回とも review・fix を実行して escalated する。post は実行されず Guard は vacuous。 |
| c | 外側の `keep_else` は、捨てる repeat の `on_limit` に End があり、後続に post がある。内部の guard の2方向と合わせて、計3件。内側の `again`・`leave` は外側の候補の根拠にしない。 | repeat 全体を飛ばすので、直前の review が fail でも post に届き Guard は violated。 |

`leave` を残す方向が選ばれ、その後の同じブロックに fix・`again` がある場合、残した `leave` でその回を抜けるので後続は実行しない。書き下しでは最初の review の後すぐ post に進み、fail でも Guard が発火して `violated`。`end rejected` を残す方向は同じ案件を即時に終えるので post が発火せず `vacuous`。End は `again`／`leave` のための静的な末尾制約とは別で、End の後ろの文は書き下し側と同じ扱いにする。

`scope.py` の L2-pair は初期 bad が確率 1/10。`keep_then` は残す枝が `leave` だけで、繰り返しの後にも呼出しがないため候補にならない。候補は修正を残す `keep_else` の1件。書き下し版のすべての if で修正を残すと、3回で escalated、確率は 1。`hold_delayed` に割り当てられるのは invariant・requires で、L2-pair にある確率の問いは割当て外。この変異は未検出なので、選択した変異の集約は `violated` / 1 になる。

`same_version/cli.py` の反復版は、`at_most 1` の本体で `d = Writer.draft()` の後に必ず `leave` する。展開を手で消すと元の逐次 `d = Writer.draft()` になる。summary_reused の書き下し対照では直後に `d_dup = Writer.draft()` を置き、Checker は `d`、Approver は `d_dup` を読む。内容が同じでも、生成した呼出しが違うので同じ版ではない。正は `satisfied`、変異体の契約は `violated`、変異判定は `detected`。反復側の生成元は `body/0/repeat/0` の1箇所。

- **外側の拒否経路。** `if q == fail { end rejected }` を外すと fail でも post に達し、書き下しでは拒否経路の弱化を検出する。手で展開した `wait 1min` と、wait の後で `leave` する1回の repeat は業務上同じだが、反復版は構成全体の拒否経路を導出しない。他の割当て済み検査はないので `unsupported / loop_control`。
- **別の repeat を捨てる外側の分岐。** `on_limit { leave }` なら修正を2回行って後続へ進むだけなので、書き下しの `f1; f2` と同じく候補なし。`on_limit { end escalated }` なら、書き下しの `f1; f2; end escalated` と同じく、後続の post に到達させる `keep_else` だけ1件。元モデルの外側 else から fail がすでに post に届くため、この変異で Guard は悪化せず `undetected`。
- **応答の混在。** 外側で明示した `response_loss none` と、内側・外側の未宣言の呼出しは、元の処理では各1回の commit を行う。ただし外側の明示 site だけでも応答変異を作ると、変異体は反復と応答喪失・再送を併用する。今回開かない範囲なので、反復内に commit がある場合もない場合も、family 3・6 は `unsupported / loop_feature_combination`、候補数は未計算で変異体0件。
- **family 4 の物理名。** `a` の Doc が `c` の判定対象、Rules が独立源。書き下しで `a_dup` を挿入して `c` の入力だけ付け替えると、元の独立源を保ったまま系譜が変わるため `detected`。反復版では `c` の行がなく `@loop:wave:1:c` だけがあり、現在の比較は回ごとの行で代用しない。割当て済みの別の検査で検出していなければ `unsupported / loop_control`。
- **on_limit の `leave`。** 最初の修正後、review の pass なら `leave` して post、fail なら修正して rejected。書き下しでは pass 枝を post と End に置き換える。`keep_then` だけなら fail も post に届いて `detected`、`keep_else` だけなら post は起きず Guard は vacuous、`undetected`。両版とも2件。
