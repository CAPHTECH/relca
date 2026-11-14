# 範囲外でも承認を呼ぶ対

正の模型の Old process に Screen.approve を1箇所追加した。それ以外は同じ。
承認0〜1分、支払1〜2分、経過時間=作業時間=2分。reachable は satisfied。
契約は適用外のままだが、対象が呼ばれるので masks_call=true と W-CONF-2 が必要。
非適用を違反や satisfied の性質にせず、全体は satisfied / exit0 を保つ。
故障を検出する欄は警告と scope 行。時間や exit だけでは警告欠落を検出できない。
