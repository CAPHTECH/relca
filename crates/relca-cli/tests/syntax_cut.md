# 構文解析と Cut の受入（2026-11-14）

`syntax_cut.rs` の期待値は利用者の決定から手で定める。
実行後の snapshot から意味を逆算しない。

| 入力 | parse | Cut | W-CUT-0 | verdict / exit |
|---|---|---|---|---|
| Cut を書き、end の識別子を `}` に置換 | 失敗 | unresolved | 無し | invalid / 3 |
| Cut を書かず、同じ構文故障 | 失敗 | unresolved | 無し | invalid / 3 |
| end を未対応の while に置換 | 失敗 | unresolved | 無し | unsupported / 4 |
| 末尾に不正な字句 @ を追加 | 失敗 | unresolved | 無し | invalid / 3 |
| Cut 無し、呼出し元 Actor を Missing に置換 | 成功、lower が拒否 | undeclared（conform の lower 拒否は既存の unresolved） | 1件 | invalid / 3 |
| Cut を宣言、呼出し元 Actor を Missing に置換 | 成功、lower が拒否 | unresolved | 無し | invalid / 3 |

全入力で stdout の各 report 内の Cut と診断件数、stderr の警告有無、exit、
hash の存在を検査する。parse の成否と lower 拒否も直接確認する。
選択子を持つ入口も別に実行し、compare / diff は両案、sensitivity は各点を見る。

## 入口の grep と呼出し表

`W_CUT_0`、`syntax::parse`、`empty_report`、`report::check` と `report::simulate`、
`sensitivity_preflight_failure` を `crates/relca-core/src` と `crates/relca-cli/src` から検索した。
生成箇所は `lower.rs` の未宣言判定と `report.rs::finish` の一度だけの補完。
`diag.rs` は語彙定義であり入口ではない。

| CLI 入口 | 解析・集約経路 | parse 失敗時の Cut の出所 |
|---|---|---|
| check（選択有無） | report::check_with_selection → check_with | empty_report |
| explain（選択有無） | explain_with_selection → check_selection_using → check_with | empty_report |
| simulate（選択有無） | simulate_with_selection → simulate_pipeline_raw | empty_report |
| compare exhaustive | compare_with_scenario → 各案の check_selected_scenario_using | empty_report |
| compare simulation | compare_with_scenario → 各案の simulate_selected_metadata / simulate_selected_scenario | simulate_pipeline_raw → empty_report |
| diff | diff_with_scenario → 両側の check_selected_scenario_using | empty_report |
| mutate（選択有無） | mutate_with_options → original の check_with_selection（work_profiles::refusal は parse 成功時のみ） | check_with → empty_report |
| sensitivity exhaustive / simulation | run::run → parse/lower の全点 gate → refuse_model → sensitivity_preflight_failure | empty_report、再parse成功時だけ Cut::parsed |
| conform event-contracts / approval-burden | conform / conform_approval → conform_raw | ConformanceReport::default（元から unresolved、警告無し） |
| verify | CLI の未接続 backend 拒否 | Cut の欄・警告を出す解析入口ではない。変更無し |

Cut::parsed は成功した AST のみから、宣言有りなら unresolved、無しなら undeclared を返す。
診断 code の文字列やソース中の cut という単語では分岐しない。
既存の `check_ast_*`（変異体）は check_with の parse seam に AST を渡すため、
同じ Cut::parsed を通る。引数・I/O 拒否の既存の unresolved は変更しない。

## 校正

empty_report の既定を undeclared に戻す故障で、構文失敗側3試験が落ちる。
Cut::parsed の未宣言側を unresolved に変える故障で、lower 拒否側が落ちる。
Cut::parsed の宣言有り側を undeclared に変える故障では、宣言済み Cut の lower 拒否側が落ちる。
いずれも元へ戻した後に全5試験を再実行する。
旧期待値の追従は Cut と警告だけを変更し、診断 code・message・span・exit の照合を残す。

## 確認用ページの読取り（round 1）

review_page.py を `parse_err|U-SYN|E-SYN|W-CUT|undeclared|unresolved|parse に失敗` と
`relca_diags|severity|_diagnostic` で検索し、次を確認した。SYN code は lower も使う。

| 箇所（修正前 531c046a の行） | 読み方・問題 | 現行の扱い |
|---|---|---|
| scan 1238–1247 | SYN code の接頭から parse 失敗と警告無しを断定 | cut、model.name の明示的 null、入力hashから解析未完了を読む。code/messageは詳細表示だけ |
| scan 1248–1250 | 上のelifにより lower のSYN診断で未宣言alertが消える。cutだけからW-CUT-0を表示 | cutがundeclaredなら先に未宣言を表示。警告の有無はdiagnosticsのW-CUT-0から別に表示 |
| scan 1251–1252 | unresolvedを汎用表示 | model.nameが非nullならparse失敗と呼ばない。unresolvedとW-CUT-0の併存もそのまま表示 |
| _scan_run 1199–1213 | check/explain/simulate、compare各案、mutate originalの配送 | 共通scanを通す。ここにはcodeからの推測無し |
| scan 1310–1311、_diagnostics_html 2190以降 | 診断を収集しcode/severity/messageを表示 | 推測無し、保持 |
| _cut_html 1660以降、section_model 1514以降 | reportのcut objectやquestionを表示 | 推測無し、保持 |
| alerts_html 2658–2664、render 2692–2693 | severityで全診断の警告を一覧化 | parseやCutを推測しない、保持 |
| _reason_of 1352以降、fold_report 890付近 | errorの詳細表示とverdictの畳込み | parseの成否・Cut・W-CUT-0の存在判定には使わない |
| flow_html 1447–1457 | loop診断のmessageを図の拒否注記に用いる | Cutやparse成否の判定ではない、変更無し |

test_review_page.pyは実際のlower拒否5例（rec-s01、s02、s08、s10、s12）で、
未宣言CutとW-CUT-0のalertを検査する。parse失敗はCut宣言有無とinvalid/unsupportedの
4組を実行する。同じSYN codeでもmodel.nameが非nullならparse失敗としないこと、
unresolvedにもW-CUT-0があれば警告すること、警告が無ければ作らないことを
report欄を変えた対照でも検査する。警告の存在を常にFalseにする故障でこの試験が落ちる。
