# 適用外の承認契約

Old は Desk.pay のみ。0分開始、1分終了し done。経過時間=作業時間=1分。
reachable end.done は satisfied。Controls は New だけの要求であり、Old では性質ではない。
id #1、適用先 New、理由と not_applicable を contract_scope/unchecked に残す。
Old の字句上の呼出しに Screen.approve は無いので masks_call=false、W-CONF-2なし。
要求は reachable の1件なので全体 satisfied、exit0。確率・単価の仮定は置かない。

壊した対は Old に承認操作を1箇所足す。違反を消したことではなく、検査の範囲を狭めたことを W-CONF-2 で明示する。
