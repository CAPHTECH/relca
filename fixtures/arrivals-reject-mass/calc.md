# F3 拒否の質量を残す

ADR 0015・0014による独立手計算。D1=3 (質量1/2)なら時刻2にcase2未開始でW=1、case3拒否。
この枝のD2=1/3は各1/2×1/2=1/4、case2終了4/6。拒否ではdraw/Received/Task無し。
D1=1ならcase2は1に開始、case3は2に受理。D2,D3の4組各1/2³=1/8。
case3終了は3,5,5,7、elapsedは1,3,3,5、workは1,3,1,3。
End質量=4×1/8=1/2、Reject=1/2、和1。within3の期限5までの質量=3×1/8=3/8。
case1 eventual/within3=1、case2 eventual1、within3=1/4+1/4+1/4=3/4。
各min=max。全案件 >=1/2 と <=1 を満たす。safety satisfied。
時間範囲は正質量End経路のみ: completed1..3,2..6,3..7、elapsed1..3,1..5,1..5、各work1..3。
result_at null、Rejectの時間4欄null。拒否件数は最小0/最大1、平均ではない。切断無し。
