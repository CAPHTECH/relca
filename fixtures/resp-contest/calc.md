# F1: structural contest

## D2 migration (ADR 0022; supersedes runtime exclusions below)

End returned at 2 now branches: declined closes 2; filed invokes c1's
Clerk.recheck 2..3, deadline 2+5=7, normal pass/fail both responded.
Runtime contest_response immediately follows static contest, satisfied.
Static judgment and both business end elapsed/work=2/2 are unchanged.
obligations=[] because there are no response clauses. No metrics declaration,
so no metrics key. Runtime assumptions distinguish target-relative response,
nondeterministic occurrence and execution-close from business end; scope is
model structure/runtime, not acceptance. Replace real-world occurrence/time
unchecked words, retain acceptance/remedy/route transfer/decision-coverage unchecked.
The paragraphs below retain the original structural derivation only.

Hand oracle: ADR 0018 including its addendum; ADR 0007/0009/0010/0011.
Reader.enter reads Invoice directly; Approver.approve reads no source, but entry
has Invoice ancestry. No commits, hence no U/E actor without a duty.
Reader runs 0..1, approval 1..2: returned and posted both elapsed=work=2min,
result_at=null. No probabilities are supplied.

Returned is directly controlled by approval from Approver.approve. Its executor
is a1, explicit Permission is {a1}, ApprovalGrant grants a1 to that same cap,
and Approver is included. Clerk.recheck exists and Clerk is included; it need
not be called and a1 need not execute Clerk. 5min > 0. All three checks true;
clause and root satisfied, exit 0. responsibilities=[]; contest_path alone is
removed from Agency unchecked. W-RESP-1 remains unchecked.

All actors belong to Scope. A contest suppresses W-CONTEST-0 even when broken.
Four broken partners each change exactly one semantic fact. This is structure,
not consent, actual route transfer, response timing, or coverage of every cause.
