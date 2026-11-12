# F1: grain は記録だけ (ADR 0023)

A の 1 分の後に B の 1 分。終了 1+1=2 分、作業量も 2 分。
どちらも process から呼ぶので既定 control は policy。
person と ledger はともに 6 byte。交換しても SHA と Actor 表 grain 以外の
全出力・状態数・生成数は一致しなければならない。潜在的意味を名前へ足さない。
W-CUT-0 のみ。手計算を先に固定し、新実装の出力から採取していない。
