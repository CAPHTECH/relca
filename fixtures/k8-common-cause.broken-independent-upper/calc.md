# V01-11: 独立積を上限にしてしまう単一故障

設計 v0.1 §18、設計 v0.2 §22.1。正例 k8-common-cause/calc.md と同じ二枝。
変更は同時失敗の上限 0.1 → 0.01 の1箇所だけ。周辺確率の検査は変えない。
確率の単位は受付1件当たり。障害(1/10)で二つとも確実にfail、正常(9/10)で二つともpass。
同時失敗1/10 > 1/100なので probability reached #1 のみ violated、全体 violated/exit1。
#2/#3は1/10ちょうどで satisfied。end.doneは確率1、時間・作業量2分、切断なし。
確率的違反専用の診断コードは無い。property id と verdict を検出期待にする。
