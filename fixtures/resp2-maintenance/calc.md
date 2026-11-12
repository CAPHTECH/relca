# F4 保守の O — 手導出

対象は Effect 自身。put の引数祖先 j→e の reads は Record。宣言Capabilityの reads と祖先原本の和={Record}。
p の Judge は直接 Record を読むので O present、direct=indirect=[Record]。
Effect 自身の commits は put/out 一つ。j fail なら out に進まず U present/all_paths、bypasses0。
句なし時間は unchecked。returned=1+2=3、done=1+2+3=6分。根 safety は satisfied。
故障対は Judge の reads [Record] 一句だけを外す。
