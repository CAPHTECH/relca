# F2 故障 — 展開の false を true にする

P=r||ok。SetR は r=true にするため後状態の P は常に真。真→偽の証人はない。
従って O present/Record は正例と同じ、U は effects=[]、not_derived/no_reachable_effect となる。
F1 との O/U の等値試験がこの変更を識別する。実行値 returned3/done6分、safety satisfied は不変。
