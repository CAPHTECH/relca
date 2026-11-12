# V01-10: 単一故障

これは判定上限の比較の対であり、模型へ入れた故障の対ではない。模型故障はbroken-miss-rateに別置きする。

設計 v0.1 §18、設計 v0.2 §22.1。正例 k8-conditional-error/calc.md の経路を維持する。
変更は確率上限 0.004 → 0.003 の1箇所だけ。
受付1件当たりの逃避率は (1/50)(1/5)=1/250、accepted=123/125、rejected=2/125。
時間・作業量は各1分。1/250 > 3/1000 なので probability reached #1 が violated、全体 violated/exit1。
切断なし、資源安全性 satisfied。確率的違反専用の診断コードは無い。
この property id と violated が検出であり、unsupported/invalid での拒否を代用しない。
