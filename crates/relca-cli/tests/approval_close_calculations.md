# Q8 閉じの手値と負対照（ADR 0061）

- 入力上限4件: spec bytes超過、spec episode超過、log event超過、log line超過。
  いずれも候補集合の全数を観測していないのでN/D、対象外、未知、承認未観測案件、
  時間の既知/未知件数はnull。案件行もnullで、観測済み0件の空配列と区別する。
  rate/timeのreasonはtruncated、exit2。spec.truncatedは最初の2件だけtrue。
  手組みwireの対では各件数だけ0に、casesだけ空配列に戻す各故障を拒む。
- defer無しのA01はN2/D3、率2/3、時間7/3。A/A.decide/P/P.proposeの
  各deferは率の全3episodeをunknownとしN0/D0、unknown3、率null/not_configured、
  時間もnull/not_configured、exit2。分母0は保留をzero_denominatorへ置換する理由ではない。
- schema/scopeは旧/旧と新/新のみ受理。旧/新と新/旧の各単一置換を拒む。
- 承認scopeの引数でもlog欠落、model不正、log重複、未知flagはargument/invalid/3。
  sidecar欠落、不正path、重複だけの拒否は従来どおりapproval_spec_argument。

上記は合成観測と手組みwireだけであり、実ログの完全性・真正性は検証しない。
