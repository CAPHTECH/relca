# 単独故障と落ちる assertion

採択範囲は ADR 0069 の段階 1。各行は別々に入れて戻す。
ここで挙げる production の合流処理は受入作成時点では存在しない。
以下の校正済み対照は、将来の production 故障の実行結果とは区別する。

| ID | production で入れる故障 | 受入の assertion | 現在の対照 |
|---|---|---|---|
| FI1 | Agency に全 yield 元の疑似 call を登録する | CLI Behavior.test_m3 の a 行不在、lineage(merge_control) と対象名、警告別 unchecked | Calibration.test_fi1_fi2_union_and_intersection_lineage_claims_are_rejected: 正常な段階 1 の観測を組み立て、和集合行を足す／対象の unchecked を消すと失敗 |
| FI2 | 系譜を積集合の一行へ潰す | 段階 1 は a 行の不在だけを要求。段階 2 の経路別二行保証は範囲外 | 上記対照の積集合行も拒否。段階 2 の独立性の保証を校正したとはしない |
| FI3 | engine の copy 元を常に then の元へ固定 | core m1_selected_source_is_zero_time_non_task_copy と CLI Behavior.test_m1 の as_is=3/4、時刻3 | Calibration.test_fi3_fi4_wrong_existing_source_is_rejected: 既存 DSL で固定した誤った値を条件へ渡し、done_as_is=1 を拒否 |
| FI4 | call 完了でだけ合流欄を書き、既存値の枝で複写しない | 同上、explain の merged、Task に合流値を足さないこと | 同上。未定義の合流欄による engine 不変条件違反を現時点で再現したものではない |
| FI5 | copy を1分の Task にする | core M1 の時刻3/5、Task集合 d/r/f/s、CLI 同値 | Calibration.test_fi5_copy_time_is_rejected: 展開模型に待ち1分を足し、4/6を観測して元の手値では失敗 |
| FI6 | arrivals の engine のみ copy を抜く | core m5_each_arrival_gets_its_own_result は engine/reference 各々の2案件の確率・時刻を先に手値照合 | Calibration.test_fi6_second_case_missing_copy_is_rejected: 正常な案件2から as_is の観測を消すと失敗 |
| FI7 | 合流時に枝の状態書込みを捨てる | core m4_branch_writes_survive_the_copy、CLI Behavior.test_m4_quality の flagged=1/20 | Calibration.test_fi7_state_write_loss_is_rejected: 展開模型の fix の bad=false だけを bad=true にし、flagged=1/5 を観測して拒否 |
| FI8 | reference のみ copy 元を逆にする | CLI Connected.test_explain_merged_and_independent_verified_replay の verified と merged、core の各々の手値 | Calibration.test_fi8_unsupported_replay_is_not_a_verified_counterexample: 実際の旧 DSL の verified 反例の status を unsupported に変えると拒否。core calibration_reference_only_state_reset_is_rejected_by_hand_oracle は reference だけを別模型にして1/5を拒否 |
| FI9 | 欠けた yield を暗黙に受理 | Refusals.test_b01_missing_yield の invalid/exit3/E-CON-0/merge_fallthrough | Calibration.test_fi9_missing_reason_is_not_accepted_as_intended_refusal: 既存の U-SEM-0 の理由だけを別の拒否へ変えて診断判定器の感度を検査。B1 の production 受理は未実施 |

## GREEN の入口校正

新機能に依存しない保存試験・展開模型・故障対照は受入作成時点で GREEN になる。
一時的に parser の入口を明示的なエラーへ差し替え、それらが実際の parser を通って失敗することを検査する。
この入口校正は上表の個別の意味論故障と混同しない。特に原因別の網羅率とは呼ばない。
production の変更はその検査の直後に戻し、元に戻した後の限定検査を行う。
