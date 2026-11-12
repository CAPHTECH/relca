# M-L: ちょうど128

ADR 0020追記。S00..S63の64source、未呼出しProbe.readに各一reads。
一種あたり64、Evidence→原本の順で計128試行。G0。
全て通常callなし、割当検査の変化なし、undetected/no_assigned_check。
128候補ちょうどは切断でない。limit_reached=false、run complete。
元はend doneだけでsatisfied、六種unsupportedで全体4。
手書き各体はreadsリストの対象source一要素だけ削除し、他63要素を保持する。
