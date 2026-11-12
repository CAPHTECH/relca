# F13 原子的な複数代入 — 手導出

P=!r||ok、finish は (r,ok)=(true,true) へ一度に書く。
どの P=true 前状態からも後11の P=true なので破壊候補はゼロ。途中の仮想的な10を効果と数えない。
support={r,ok}、SetBoth は Record を読む。p に Actor がなく O missing/Record、W-RESP-1。
U not_derived/no_reachable_effect、時間句なし unchecked。done3分、safety satisfied。
故障対は finish の ok=true だけ false へ変える。
