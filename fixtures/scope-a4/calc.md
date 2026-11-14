# 適用外のセルを残して比較する

Old は pay だけで1分、work1分。New は approve + pay = 1+1=2分、work2分。
両方 done に到達する。New の boss は Mgr の grant と一致し、Controls は satisfied。
Old の Controls は未検査なので null / not_applicable / configuration_scope。satisfied にはしない。
性質表は reachable と Controls の2行、各行に Old,New の2セルを残す。
適用が非対称なので W-CMP-1 を1件と checked contract_scope、前提の説明が必要。全体 exit0。
壊した対は適用指定を外す。すると Old の Controls は vacuous になり、全体exit5に戻る。
