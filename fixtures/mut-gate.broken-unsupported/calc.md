# M-B: 未対応文法のgate

mut-gateからdurationだけfixed 1min→unknown。ADR 0020の元gate。
U-SYN-0、元unsupported/4、bounds null、四種blocked、候補生成0。
他の名前・契約・Cutは変えない。元がvacuousの場合と同じ生成禁止だが理由は異なる。

元のcheckは既存規則のW-CUT-0も出す。U-SYN-0で解析が止まりCutを解決しないためで、期待には主診断とこの警告の両方を残す。判定値の意味は変えない。
