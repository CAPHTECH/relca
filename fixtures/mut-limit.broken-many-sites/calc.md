# M-L: 上限後も行を残す

mut-limitへsource S64を宣言して同じreads/includeへ一つ追加。
ADR 0020追記。R65×2=130候補、Evidence65試行の後、原本S00..S62の63試行。
原本S63/S64の二行はnot_run/mutant_limit、report null、changes空。
試行128で停止するがdescriptor130を保持。limit_reached=true、run inconclusive。
実行済み128体は割当なしで未検出、元satisfied、全体は六unsupportedにより4。
全12dirsの候補は4+4+6+6+6+4+0+0+0+0+128+130=288、試行286。
