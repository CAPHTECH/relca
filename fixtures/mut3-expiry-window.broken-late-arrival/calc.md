# 遅い到着で、失効を検出できない対

故障は到着を0分から10分へずらすことだけ。approveを10分に開始し、
11分に終了する。経過1分・作業量1分で、時間は確かに進む。
元の期限5分でも11 > 5なので authority_basis は既に violated。
G の期限を0分に変える変異でも11 > 0で violated のまま。
割当ての検査は失敗の向きに変化せず、undetected / no_worsening となる。
元の模型が正しいと認定された意味でも、実務品質の検出率でもない。

正例と同じ1 Authority・非0の期限なので候補1。他に reads、guard、
共有結果の消費者、commit、cohort は無く、対応済み他種は0、未対応種はnull。
candidates_total は1、run は complete。未対応種のため最上位は exit4。

期待は変異体の result=undetected を直接指定して照合する。
比較失敗だけを一覧化するメタデータ undetected:true で許容するのではなく、
手値との不一致は失敗させる。0分の not_applicable の対とは別の役割である。
