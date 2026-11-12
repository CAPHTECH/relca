# F4: 正確な確率和

唯一の行は 1/4+3/4=1。sample 0→1 分で hit=true/bad は1/4、false/good は3/4。
非決定による値の差なし、各 min=max。両終端 elapsed=work=1、result_at=null。
容量1、safety satisfied。最大1/4 <= 閾値1/4、確率性質/root satisfied/exit0。
truncated=false、W-CUT-0 のみ。これは受付1件当たりの値。期待は ADR 0013 と算術から固定。
