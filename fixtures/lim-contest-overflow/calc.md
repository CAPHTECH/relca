# lim-contest-overflow

ADR 0039; design v0.2 §22.3. Hand-derived before execution.

Party has Standing on done and a positive-time route with authorized reversible executor. Target overflowed is refused as unsupported before running a route.

P2 delay, P9 label and K mutations are not counted as single-input defect pairs.

## overflow の検算

最初の入力は done へ無条件に届き、done を左右する呼出しが無いので、旧い contest の規則(contest.rs の derive、終端を決める呼出しの executor・permission・authority)では p の decision が 0 件になり、reversal_authority が no_decision で violated だった。overflow を reject に替えただけの入力を変更前の binary に掛けて再現した。

直した入力は r == ok で分岐し、どちらの腕も done に終わる。A.run が done を左右する呼出しになり、p は executor かつ G の grant を持つので reversal_authority は満たされる(contest は satisfied)。else の腕は取られない(W-UNREACH-1 は警告)。

案件: 到着は 3 件とも 0。overflow を reject に替えた入力では、変更前の binary で案件 1 が done(2 分)、案件 2 と 3 が受け付けられなかった。ADR 0039 は溢れたときの行き先だけを変えるので、案件 2 と 3 は overflowed で終わり、done に届かない [R](溢れる案件が reject の入力と同じであることに依る)。

contest_response: 案件 1 は done の 2 分で起票でき、Route.reply 1 分で期限 1 分の内に応答する(satisfied)。案件 2 と 3 は done に届かないので vacuous。旧い集約(reject の入力で変更前の binary が satisfied・vacuous・vacuous を vacuous にまとめた)により property は vacuous、report の verdict も vacuous(exit 5)。
