# F3: 完了までの到達

coin 0→1 分で short/long は各1/2。short は seen=true@1、done@1、work1。
long は wait 1→4 分で seen=true@4、done@4、work4。
到達確率 (1/2)×1+(1/2)×1=1。全順で同値なので min=max=1、done も1。
elapsed/work の極値[1,4]、result_at=null。clock4 は終了時刻4を含む。
safety と >=3/4 は satisfied、root satisfied/exit0、truncated=false。
W-CUT-0 のみ。分母は受付1件、平均時間を報告しない。実装出力未使用。
