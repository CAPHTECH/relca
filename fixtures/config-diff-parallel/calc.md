# P1: 逐次から並行へ (ADR 0008、計画第6・12節)

実装出力を使わず計算。時間・作業量の単位は分。結果は pass のみで確率は置かない。

| 構成・経路 | 開始→終了 | elapsed | work | result_at |
|---|---|---:|---:|---|
| AsIs / Serial | short 0→3、long 3→8、done 8 | 8 | 3+5=8 | null |
| ToBeP / Parallel | short 0→3、long 0→5、join all 5、done 5 | max(3,5)=5 | 3+5=8 | null |

capacity=2 なので並行枝の開始順はどちらでも同じ時刻。全経路で上記値、min=max。
両方の静的 actors={Office}、relations={Office.short, Office.long}。差分は空。
reachable done と資源安全性は両側 satisfied。clock=10、live_tasks=2 に達して切断されず truncated=false。
同じ Cut C、intent 不要。時間だけの変更を W-DIFF-1 にしない。親 satisfied / exit 0。
root の E-DIFF-0/E-CMP-0/W-DIFF-1/W-DIFF-2、各側の W-CUT-0/W-DEADLOCK-0/W-CONF-0 は不在。
