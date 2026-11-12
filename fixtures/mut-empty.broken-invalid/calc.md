# M-N: 元が無効

mut-empty から NoSuch.run のcall一つを追加。ADR 0002、ADR 0020追記。
未知Actorで E-NAME-0、元 invalid/3、boundsはnull。
候補生成0、四種blocked/original_not_decidable、六種unsupported。
元のgateを優先し全体もinvalid/3。unsupportedで上書きしない。

元のcheckは既存規則のW-CUT-0も出す。Cutを宣言していないためで、期待には主診断とこの警告の両方を残す。判定値の意味は変えない。
