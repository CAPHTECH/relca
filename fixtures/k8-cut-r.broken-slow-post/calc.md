# V02-14前半: Cut R の単一故障

設計 v0.2 §5.5・§22.1。正例 k8-cut-r/calc.md から登録所要だけ1分→2分。
Extractは0..1分、Posterは1..3分。elapsed=work=3分、end.done確率1、切断なし。
受付1件当たり誤登録は依然(1/50)(1)=1/50。#1、資源安全性、C#1はsatisfied。
2分以内登録は0なので probability reached #2 が violated、全体 violated/exit1。
mechanism_unmodeled注記は残る。確率的違反専用の診断コードは無い。
再送・通信障害を追加した故障ではない。
