# B2: configuration 選択無し

model は P1 と byte 単位で同一。唯一の故障は invocation.json が無いこと(default check)。
潜在的な Serial 経路は short 0→3、long 3→8、elapsed/work=8/8。
Parallel は short 0→3、long 0→5、elapsed/work=5/8。どちらも result_at=null。
しかし selector 未指定で選べず E-CONF-0/error、invalid / exit 3。
engine は実行しないので時刻の実測出力なし、truncated=null、terminals/properties=[]。
宣言 Cut があるため W-CUT-0 は不在。diff でなく旧 Report wire を照合する。
