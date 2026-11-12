# F3 単一故障: clock4 → clock2

short 1/2 は seen=true/done@1、work1。long 1/2 は wait 終了4より先に時計2で切断。
未知質量を0と置くと mL=ML=1/2、1と置くと mU=MU=1。
seen と done の表示は[1/2,1]。1/2 < 3/4 <= 1 より >=3/4 は inconclusive。
safety も探索切断により inconclusive、root inconclusive/exit2、truncated=true。
done の時間は完了短経路だけの elapsed=work=1、result_at=null。未完了者で再正規化しない。
W-CUT-0、時計切断なので W-BUDGET-0 は出さない。期待は手計算。
