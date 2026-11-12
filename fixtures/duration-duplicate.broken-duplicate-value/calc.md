# 時間値だけを重複させる

正解版の5minを3minへ単一変更。確率和1/4+3/4=1だが、同じ時間3分が2回ある。
ADR 0014 は重複を E-PROB-0 とする。和が正しくても統合して受けない。
invalid/exit3、error、properties/terminals 空、agency null、bounds4欄null。
実行していないため elapsed/work/result_at や確率の極値はない。
W-BUDGET/W-DEADLOCK/U-SEM/E-CAP は禁止。期待は静的規則から計算。
