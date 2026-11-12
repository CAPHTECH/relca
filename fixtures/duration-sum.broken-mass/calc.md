# 確率和の単一故障

5分の確率だけ1/2から2/5へ。ADR 0014 の検査: 1/2+2/5=9/10≠1。
E-PROB-0/error、invalid/exit3。再正規化して受けてはいけない。
runtime を実行しないので properties/terminals 空、agency null、bounds の4欄は全てnull。
時刻と確率の min/max を計算せず、0時間の仮終端も作らない。
Cut 未宣言の警告有無は部分期待に含めず、W-BUDGET/W-DEADLOCK/U-SEM/E-CAP は禁止。
