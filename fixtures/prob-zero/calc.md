# F7: 0枝を展開しない

行和1+0=1。pass/false の確率1だけを展開、fail/true の0枝は安全性・契約・時間・Roomへ寄与しない。
sample 0→1 分、slow100分は呼ばれない。done確率1、elapsed=work=1、result_at=null。
danger到達0×1=0、min=max0。<=0 と finish後 danger=false invariant は satisfied。
容量1なので safety satisfied、root satisfied/exit0、truncated=false。W-CUT-0 のみ。
0枝の型はなお静的検査する。期待は手計算で実装出力未使用。
