# V02-14前半: Cut G の単一故障

設計 v0.2 §5.5・§22.1。正例 k8-cut-g/calc.md から登録時間だけ1→2分。
Extractは0..1分、Ledgerは1..3分。elapsed=work=3分、end.done確率1、切断なし。
受付1件当たり誤登録は依然(1/50)(1)=1/50、#1とC#1と資源安全性はsatisfied。
2分以内登録は0なので probability reached #2 が violated、全体violated/exit1。
check-and-insertの機構はそのまま、mechanism_unmodeled注記なし。
確率的違反専用の診断コードは無い。再送や応答喪失を追加した故障ではない。
