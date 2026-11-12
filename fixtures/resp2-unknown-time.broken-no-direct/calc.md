# F14 故障 — 空経路を0分の成功にしない

Judge の直接 reads 一句だけ削除。祖先 Reader が読むため required/indirect=[Record]、direct=[]。
O missing、U present のまま。必要な O の証拠経路が無いため時間 unchecked/reason=observation_missing。
within=5、required_duration=null、paths=[]。既知 U の5分だけで成功にしない。
W-RESP-1 は一件。root safety satisfied、returned3/done6分は変わらない。
