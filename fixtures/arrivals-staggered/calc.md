# F2 共有容量1・ずれた到着

ADR 0015の独立手計算。確率未入力なのでJSONに確率を足さない。
唯一の時間経路は [0,3), [3,6), [6,9)。待ちは0,2,4分。
completed_at=(3,6,9)、elapsed=終了−到着=(3,5,7)、work=(3,3,3)。
各範囲の最小=最大、result_at null。全案件doneに到達するのでreachable各satisfied。
共有占有≤1でsafety satisfied、拒否0、clock20内でtruncated false。
