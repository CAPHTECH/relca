# F7 contest-only reader must remain active

Normal owner set Clerk,Gate→Extractor,Gate gives actor Extractor added /
Clerk removed, and relation Extractor.extract added / Clerk.enter removed.
Both E include rejected so Clerk.recheck is in A on both sides. Resource
Clerk stays common; only Extractor resource null→{2,null}. Readers S are
{Clerk}→{Clerk,Extractor}: 1 visibility row. Exactly6 rows, W1=W3=6.
Time0/meaning0/authority0/contest0. Business elapsed/work2 min.
Mutation deletes reads[S] only on recheck: To readers={Extractor}, From
still {Clerk}; all row counts same, but visibility payload changes.
