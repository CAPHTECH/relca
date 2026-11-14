# 受信が届いた対

sequenceの1語だけをunknownからdeliveredへ変更。追加のTaskや時間は受信そのものでは増えない。
Judgeは12回固定、Intake1回、Ledger0〜1回。doneは1+12+1=14分、rejectedは1+12=13分。
Fはresponse_controlのまま。Lは切断なしの両探索器のStart範囲が一致したときだけ、この回数を報告する。
安全性は両分岐ともsatisfied。FについてW-AGY-5を出したらJSONとの矛盾を検出する。
