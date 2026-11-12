# F2: inactive exclusion and fulfilled response

Received 0 creates one unlabeled response, deadline 4. A returns ok at 1,
B starts 1 and finishes 2, fulfilling at 2. End done 2, work 1+1=2.
A.finish(bad) never occurs: excludes vacuous. Root vacuous, exit 5, despite
the fulfilled response. Obligations are opt-in because excludes is used.
Debtor is null/not_configured; no implicit debtor from B's Actor.
W-CUT-0 is required. No waiver metadata on fulfillment.
