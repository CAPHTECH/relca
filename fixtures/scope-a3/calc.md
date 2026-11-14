# 到達しない枝も適用範囲の中

Desk.pay が返すのは ok のみ。p == bad は偽なので承認は開始されない。
pay は0〜1分、done は1分、work1分。reachable は satisfied。
Controls は New に適用されるが正常終了の対象が一度も無く、vacuous / exit5。
承認の字句上の呼出しはあるので W-UNREACH-0 の never_started。W-CONF-1 は出ない。
壊した対では適用先だけを Old に替える。この明示的な検査除外は W-CONF-2 と scope 行で露出する。
自動で非適用にして vacuous を隠す実装は、正の模型の exit5 を満たせない。
