# F19 invoice calibration, not a rewrite of the design example

Clone of existing invoice executable declarations/processes, adding ToBeA intent.
Optional metrics/simulation declarations are omitted: the preexisting Report
metric assumption validator does not admit the invoice's responsibility-tail
placement. This acceptance does not change that unrelated Report contract.
Metric-recursive new-row/preservation validation uses separate small controls.
Normal actors add Extractor; relations add Extractor.extract then remove
Clerk.enter (keep old added-before-removed ordering). Active contest route
Clerk.recheck keeps Clerk among Invoice readers. Resource Extractor added
null→{2,null}; time2 (match_po,match_receipt); visibility Invoice Clerk→
Clerk+Extractor. Authority Clerk.enter removed: Clerk.enter has no permit,
so its Permission is Clerk's executors [c1,c2], no grant/basis/duty:
from={permission:[c1,c2],grants:[],authority_basis:[],responsibilities:[]},
to=null. Extractor.extract: anonymous executors, no permit, nothing else ->
null on both sides, no row. Approver.approve: [a1], grant, duty a1 on both
sides, equal. Exactly8 rows. No meaning/contest row.
Five target claims cover actor, extract, enter, resource, match_po. Three
unclaimed rows (authority Clerk.enter, time match_receipt, visibility) give
W1=3. Global preserve gives W3=0. Deleting only resource claim in broken pair
leaves8 rows, four claims, W1=4 (not propagation).

InvoiceControls has8 property ids: two commit invariants, once, basis,
separation, response, static contest, runtime contest response. In ToBeA
all except separation fire and satisfy:7 satisfied/1 vacuous. Separation
has no Clerk.enter pair. Neither safety nor quality probability is counted.
Quality remains violated: 0.8*0.02*0.20*0.15*0.10 +
0.2*0.15*0.50*0.40*0.30 = 231/125000 > 1/1000. Root1.
Posted elapsed/work AsIs17/17→A9/12 min; no numeric expectation is changed.
Three plans of examples/invoice/configurations.relca (no intent), recounted
with Permission: AsIs→P 2 rows (time match_po, match_receipt; Clerk.enter
stays active); AsIs→A 8 rows (actor Extractor, relation extract added,
relation enter removed, authority Clerk.enter removed, resource Extractor,
time2, visibility Invoice); P→A 6 rows (the same without time2). W1=W3=rows
on each. Reverse A→AsIs 8 rows: flips from/to and added/removed, not the
modified identity. A→A 0. A→A with claims is surplus (test uses
intent-free original for same-selection0). This is a count calibration
(8 computed / 5 claimed / 3 W1), not the design v0.2 §12.6
time-pair/fallback/maintain-row example (7/5/2).
