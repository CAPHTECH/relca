# F6: 取消は抽選でない

Fast/Slow start0、Fast normal1/pass、採用1、Slow取消要求1。ack_within1 は整数の正の遅延1だけで ack2。
Slow は正常終了しないので kernel の積和は空、changed=true へ達する確率0、doneは1。
Probe 2→3、done3。elapsed3、work=Fast1+Slow2+Probe1=4 分、result_at1。
safety satisfied、<=0 は0<=0で satisfied、Probe.finish 時 changed=false の invariant satisfied。
root satisfied/exit0、truncated=false、W-CUT-0 のみ。初期抽選で with を先に書いてはならない。
期待は ADR 0013 と取消時刻から独立算出。
