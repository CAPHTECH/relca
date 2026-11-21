# 結果だけを非決定にする

正の outcomes kernel K を result nondet { pass, fail } に置換した1故障。
常にpassなら(2+4)/2=3分、常にfailなら(5+7)/2=6分。
Lead は [3,6] per_case_relaxation。一様化した9/2分のexactではない。
Done/Reworked の分母下界は0、上界1なので denominator_may_be_zero、下界無し。
Done 全流入は全到達可能な方策もあるため beyond_reached_mass と最小3分。
verify metric は今回の範囲外。reachable done の判定は satisfied/exit 0 のまま。
