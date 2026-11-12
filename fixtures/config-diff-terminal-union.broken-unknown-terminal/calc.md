# B7: verify の未知終端

P5 から verify の `end.new` だけを `end.absent` に変更。process の end new は残す。
潜在的な Old 経路は Office.c 0→3、old=3、work=3。New は Office.c 0→3、new=3、work=3。
しかし inventory={old,new} に absent が無く、両側の lower が E-NAME-0/error で失敗。engine は未実行。
両 report と親は invalid / exit 3、側の truncated=null、terminals/properties=[]。
比較未実施なので root の 3 比較配列も空、mismatch=null、checked=[]、全未実施項目を unchecked に足す。
root に側の E-NAME-0 を複写しない(D5)。各側に宣言 Cut があるため W-CUT-0 は不在。
