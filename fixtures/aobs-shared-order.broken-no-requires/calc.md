# F05 aobs-shared-order.broken-no-requires — hand oracle

ADR 0032; plan section 3. Values fixed before implementation, minutes.

Joint alternatives exactly (posted,denied) and (denied,posted), completed_at=(1,1). First work1, refused work0. Each joint path has one refusal, but aggregate refusal table has two case rows at1. Without requires, second finish at2 attempts 1+1=2>1, no second end and no requires refusal.

Single input transformation: '      requires posted == 0 else rejected\n' → ''.

Expected verdict: violated; command: check.
RED acceptance is not implementation verification.
