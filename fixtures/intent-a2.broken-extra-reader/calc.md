# A2の故障対 / A3の追加読み手

単一故障は Audit.log の reads [Doc] 追加。時間・作業量は4/4分、行数は12のまま。
Doc の読み手は元の{Clerk}から{Audit,Bot}へ変わるが、差し替えだけでは{Bot}にしかならない。
よって visibility Doc も覆わず、合計7行を覆う（direct2/derived5）。Audit4行とDoc1行が残る。
W-DIFF-1/2/3=5/0/5、satisfied / exit 0。原本の追加読み手を黙って承認する実装を捕まえる。
