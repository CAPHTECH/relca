# 直列10判定

Intakeを1回、最初のfailまでJudgeを1〜10回呼ぶ。全passだけLedgerを1回呼ぶ。
従ってLはIntake=[1,1]、Judge=[1,10]、Ledger=[0,1]。doneは1+10+1=12分、rejectedは1+m=2〜11分。
doneの全jはall_contexts。rejectedはj10だけが他を全passに固定したとき値で止められ、some_contexts。
先行jは自分か後続のfailでrejectedに届くのでread_only。安全性はすべての直列経路で満たす。
broken対では最後の拒否だけを外す。j10はdoneを選べずread_onlyへ変わる。採取値を期待に写していない。
