# F6: state norm and unknown noncommit writes

ADR 0018 U4 and ADR 0012. P=(registered==false || approval==pass), support
{registered,approval}. Writers Ledger.post and Approver.approve have Invoice
ancestry through entry; R={Invoice}. H(a1)={Approver}; direct=indirect={Invoice},
O present. kernel latent dependencies are not used as observations.

Ledger's commits write registered=true: abstract pre registered=false and
approval=fail satisfies P, post violates it. Its refusal is Approver.approve@ap
all_paths/0 because fail ends returned. BUT the noncommits Approver write can
change pass to fail with registered=true, also violating P. Total U therefore
not_derived/noncommit_state_write, keeping Ledger's known all_paths row and
unchecked responsibility_noncommit_writes/responsibility_refusal. No W-RESP-2.
Anonymous Ledger alone W-RESP-0. Scope includes all actors, no contest.
P is NOT a runtime invariant. done 3/3, returned 2/2min, result_at=null,
safety/root satisfied, exit 0. Partner removes only Approver's Invoice reads.
