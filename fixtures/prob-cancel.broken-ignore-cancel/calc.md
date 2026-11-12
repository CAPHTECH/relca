# F6 単一故障: Slow の取消契約だけ forbidden_after_start

採用はFast.finish1。Slowは要求を無視し正常終了5で1/2ずつ changed=true/falseを書き込む。
到達確率(1)(1/2)=1/2、min=max。done確率1/2+1/2=1。
Probe 5→6、done6、elapsed6、work=Fast1+Slow5+Probe1=7、result_at1 分。
<=0 は最大下界1/2>0なので violated。Probe.finish invariant も true の正確率経路で violated。
safety satisfied、root violated/exit1、切断なし。W-CUT-0 は中立。実装出力未使用。
