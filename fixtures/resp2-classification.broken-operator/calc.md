# F10 故障 — 未対応比較の分類

(r == true) の == だけ > へ変更。構造はあるが責務の > 比較は未対応。
U-SYN-0/unsupported/exit4 とし、位置は > の1byte (source.find(r > true)+2) に固定する。
欠落operand/target/within値を一括 unsupported にする変更は許さない。別行列で E-SYN-0 を維持する。
この resp2 入力は新規であり372旧jobの分類移管集合には含めない。
