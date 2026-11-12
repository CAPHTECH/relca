# F4 単一故障: 最後の到着2 → 3分

case2は2に開始済みなので時刻3にW=0、case3を受理。占有[0,2),[2,4),[4,6)。
質量1、completed(2,4,6)、elapsed(2,3,3)、各work2、result_at null、各End確率1。
hit各1で>=0、safetyもsatisfied。拒否0、切断無し。時刻2の空き枠先取り故障との差を検出する。
