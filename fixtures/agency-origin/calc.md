# F3: 同じ原本への直接到達と中継は両立

ADR 0009・計画の第4節からの独立手計算、時間は分。
S(entry)={Invoice}, S(approval)={Invoice}。approval の対象は {Invoice}、
直接読む Invoice は既に対象なので independent_sources={}。1判定1Actor、独立源0。
Invoice 行は両集合空。Reader.direct={Invoice}, indirect={}。
Reviewer.direct={Invoice}, indirect={Invoice}。交差を除去しない。
Ledger.direct={}, indirect={Invoice}。

Reach(posted)=approval.pass。文脈は空タプル {} の1個。
approval=pass は到達、fail は不達なので all_paths/0、W-AGY-0/1 不在。
entry1+review1=2、failは rejected elapsed/work2、passはpost1を足し done3/3。
result_at=null、切断なし、2性質と全体 satisfied/exit0。未宣言 Cut の W-CUT-0 は必要。
