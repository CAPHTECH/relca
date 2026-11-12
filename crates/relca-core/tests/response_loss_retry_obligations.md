# ADR0056 実装側に残す非公開検査

受入の公開targetはresponse_loss_retry。以下を新受入の存在だけで実施済みにしない。
今回の担当はcrates/*/srcを編集しないため、非公開型にtest moduleを接続していない。

| ID | 実装側の追加検査 | 公開受入が直接観測する隣接契約 |
|---|---|---|
| P01 | 不正IRの試行数0/33、sequence長、他案件/別site/別keyのcommand混同を両validatorで拒否 | max/refusal、arrivals、identityの結果 |
| P02 | Hash/Eqに未公開実結果、Receipt、command/attempt、保存入力、未解決印が含まれ、異なる後続を併合しない | g-nondetの全経路と最終unknown |
| P03 | 完了遷移の実効果とunknown選択の間に他Taskの開始/観測が割り込めない | registered/count性質、未知bindingからの後続禁止 |
| P04 | retry後も固定入力と初期潜在を保持し、環境のmutable stateは次Taskの開始時snapshotで読む | correlated、backoff、原子的G |
| P05 | public Outcomeが未SerializeのReceipt内部履歴についてもreferenceと別実装の観測を照合 | public ReportのReceipt表、raw Outcomeの終端/契約/確率 |
| P06 | replayのattempt番号、Receipt値、実効果、command元siteを一つずつ改竄して参照再生が拒否する | explainのverifiedとsource location |
| P07 | 任意の確率pの新乱数鍵が元call/attempt/チャネルを識別し、案件初期潜在を再抽選しない | p=0/1の決定的標本、non-det拒否、網羅p=1/10 |
| P08 | Receipt held専用判定へtruncated/SemanticAbort/不足wireを渡してinconclusiveにする | 再送clock切断、公開held証拠、R invalid |
| P09 | 新種が既存予算順/128上限の前に割り込まない、kind7との共通上限 | 既存reads2→新再送126→残りnot_runの公開対 |
| P10 | 原本・人・資源・Agencyにattemptを新しい独立根拠として足さない | 名称不変・work・capability回数。Agency全項目の独立oracleは未追加 |

実装後は故障を一つずつ入れ、どの公開受入/非公開検査が落ちたかを記録し、元へ戻して再検証する。
要求を満たすためにignoredや一律unsupportedへ移したり、unknownの比較対象を落としたりしない。
compare/diff等の明示的な未対応交差はADR0056の境界であり、check/explain/arrivals/通常品質/指標の必須範囲を拒否へ縮める根拠ではない。
