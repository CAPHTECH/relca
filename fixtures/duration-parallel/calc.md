# 並列時間と同時刻の独立な積

ADR 0014 本文・追記、ADR 0013 から手計算。分単位。
容量2なので両 Task は時刻0に開始。(3,3),(3,5),(5,3),(5,5) は各1/2×1/2=1/4、総和1。
all の時刻は max(3,3)=3、max(3,5)=5、max(5,3)=5、max(5,5)=5。
両 write が時刻3までに起きるのは最初の経路だけ。各同時刻終了順で P=1/4、min=max1/4。
query <=1/4 satisfied。安全性も satisfied、root satisfied/exit0。
done 確率1、elapsed min3/max5、work は和6,8,8,10からmin6/max10、result_at null。
同時刻の偶然は全 Task の積を非決定より先に決める。4 joint は内部試験で別に検査する。
切断なし、W-CUT-0 のみ。未来の終了時刻は開始時には公開しない。
