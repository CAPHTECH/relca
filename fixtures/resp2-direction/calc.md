# F1 含意の向き — 手導出

ADR 0028 と裁定 U1。単位は分。P = !r || ok の真理値は (r,ok)=00,01,10,11 の順に 1,1,0,1。
SetR は r だけを true にする。前00→後10 が真→偽の証人。初期状態や実行の到達性で抽象前状態を削らない。
support は {r,ok}、writer と祖先が読む原本は {Record}。p の Guard は直接 Record を読むので O present。
Guard の fail は out を通さず、j→out は all_paths。U present、effect は SetR.put/out 一件。
時間句なしは {status:unchecked} だけ。実行は returned=1+2=3、done=1+2+3=6、作業も同じ。根の safety は satisfied。
故障対は対象の向き一箇所だけを反転する。実装出力から期待値を採っていない。
