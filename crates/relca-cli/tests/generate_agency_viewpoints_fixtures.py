"""Materialize ADR0034 hand oracles, never run or read a Relca binary.

The finite tables below contain independent hand expectations for A01..A23.
A16/A17 are identical DSL controls: their broken budget is injected privately,
not represented by invented language/CLI syntax. A19 is a saved-output fault.
"""
import copy
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[3]
F = ROOT / "fixtures"
PAIRS: list[dict[str, Any]] = []


def cap(actor, name="draw", result="pass,fail", ty="V", fields="", qualification="capacity 1", params=""):
    return f"actor {actor} {{ {qualification}; capability {name}({params}) -> {ty} {{ {fields} duration fixed 1min; result nondet {{ {result} }} }} }}"


def model(actors, body, extra="", people=(), cut=None):
    names = [a.split()[1] for a in actors]
    return ('model AgencyFirstHalf version "0.2" {\n'
            '  type V = enum { pass,fail }\n  type T = enum { ok }\n'
            + ("  people [" + ",".join(people) + "]\n" if people else "")
            + ("  " + extra + "\n" if extra else "") + "  " + "\n  ".join(actors)
            + "\n  process Flow { " + body + " }\n  "
            + (cut or ("cut K { include [" + ",".join(names) + "] }"))
            + "\n  bounds { cases 1; live_tasks 8; clock 100min }\n"
            + "  verify safety no_resource_overcommit\n}\n")


def end(name, selectors=(), read=(), reachable=True, state=False):
    if state:
        return dict(end=name, status="not_derived", reason="state_guards")
    return dict(end=name, status="derived", reachable=reachable,
                selectors=[dict(call=c, capability=k, selects=s) for c,k,s in selectors],
                read_only=[dict(call=c, capability=k) for c,k in read])


def sel(call, capability, which="all_contexts"):
    return (call, capability, which)


def retention(name, lo=1, hi=1, route=False, cancel=False, state=False) -> dict[str, Any]:
    row: dict[str, Any] = dict(capability=name, status="not_derived" if state else "derived")
    row.update(dict(reason="state_guards") if state else dict(normal=dict(min=lo,max=hi)))
    row.update(contest_route=route, basis="unknown", fallback="unchecked", join_any_cancellation=cancel)
    return row


def source(name="Invoice", actors=(), people=(), unattributed=(), ca=(), cp=(), cu=(), routes=()) -> dict[str, Any]:
    return dict(actor=name, normal=dict(actors=list(actors),people=list(people),unattributed_actors=list(unattributed)),
                contest=dict(actors=list(ca),people=list(cp),unattributed_actors=list(cu),routes=list(routes)))


def person(name, direct=(), indirect=(), contest=()) -> dict[str, Any]:
    return dict(person=name, normal=dict(direct=list(direct),indirect=list(indirect)),contest=dict(direct=list(contest)))


def view(ends, caps, sources=(), people=(), actors=()) -> dict[str, Any]:
    return dict(rules="agency-viewpoints-p1-1",scope="selected_process_lexical_calls_and_declared_contest_routes",
                observation=dict(status="derived",sources=list(sources),people=list(people),fallback="unchecked"),
                framing=dict(status="derived",ends=ends,actors=list(actors),complete=all(e["status"]=="derived" for e in ends),unchecked=["type_domain_authority","practical_framing"]),
                proposal=dict(status="not_derived",reason="no_proposal_contract"),
                retention=dict(status="derived",capabilities=caps,notice="行使回数から不要とは判定しない",unchecked=["fallback_dependency","retention_basis","actual_skill"]))


def replace(text, before, after):
    changed = text.replace(before, after)
    assert changed != text, (before, text)
    return changed


def pair(number, name, fault, a, b, va, vb, calc, diagnostics=((),()), verdicts=("satisfied","satisfied"), invocation=None):
    PAIRS.append(dict(id=f"A{number:02}",name="agy-"+name,fault=fault,models=[a,b],views=[va,vb],calc=calc,diagnostics=diagnostics,verdicts=verdicts,invocation=invocation))


invoice = "actor Invoice { source document }"
a=model([invoice,cap("Clerk","enter","ok","T","reads [Invoice];", "executors [c1,c2]"),cap("Approver","approve","ok","T",qualification="executors [a1]",params="e:T")],"e = Clerk.enter(); a = Approver.approve(e); end done",people=["c1","c2","a1"])
v=view([end("done")],[retention("Clerk.enter"),retention("Approver.approve")],[source(actors=["Clerk"],people=["c1","c2"])],[person("c1",["Invoice"]),person("c2",["Invoice"]),person("a1",indirect=["Invoice"])])
w=copy.deepcopy(v); w["observation"]=dict(status="derived",sources=[source()],people=[person(p) for p in ["c1","c2","a1"]],fallback="unchecked")
pair(1,"o-people","no-read",a,replace(a,"reads [Invoice];",""),v,w,"reads Invoice → enter → approve. c1/c2 directly reach Invoice; a1 only inherits it through e. Removing the only reads root empties direct AND indirect, not call counts. Both calls execute once; the unconditional end has no candidates.")
a=model([invoice,cap("Clerk","enter","ok","T","reads [Invoice];","executors [c1,c2]")],"e = Clerk.enter(); end done",extra="permit c1 to Clerk.enter\n contract C { permitted Clerk.enter }",people=["c1","c2"])
v=view([end("done")],[retention("Clerk.enter")],[source(actors=["Clerk"],people=["c1","c2"])],[person("c1",["Invoice"]),person("c2",["Invoice"])])
w=copy.deepcopy(v); w["observation"]["sources"][0]["normal"]["people"]=["c1"];w["observation"]["people"][1]=person("c2")
pair(2,"o-permit","one-executor",a,replace(a,"executors [c1,c2]","executors [c1]"),v,w,"Permission checks, but does not prune, the two executor choices. c2 still reaches Invoice in the original and violates permitted. Removing c2 from executors leaves its declared person row empty and satisfies permitted.",verdicts=("violated","satisfied"))
a=model([invoice,cap("Extractor","extract","ok","T","reads [Invoice];")],"x = Extractor.extract(); end done",people=["x1"])
v=view([end("done")],[retention("Extractor.extract")],[source(actors=["Extractor"],unattributed=["Extractor"])],[person("x1")])
w=copy.deepcopy(v);w["observation"]["sources"]=[source(actors=["Extractor"],people=["x1"])];w["observation"]["people"]=[person("x1",["Invoice"])]
pair(3,"o-unattributed","declared-executor",a,replace(a,"capacity 1","executors [x1]"),v,w,"One anonymous slot is not a person identity. Declaring x1 changes attribution only. extract executes once in both inputs.")
a=(ROOT/"fixtures/resp-contest/model.relca").read_text()
a=replace(replace(a,"people [c1,a1,cfo]","people [c1,c2,a1,cfo]"),"executors [c1]; capability recheck() -> Verdict {","executors [c1,c2]; capability recheck() -> Verdict { reads [Invoice];")
v=view([end("returned",[sel("approval","Approver.approve")]),end("posted",[sel("approval","Approver.approve")])],[retention("Reader.enter"),retention("Clerk.recheck",0,0,True),retention("Approver.approve")],[source(actors=["Reader"],unattributed=["Reader"],ca=["Clerk"],cp=["c1","c2"],routes=["Clerk.recheck"])],[person("c1",contest=["Invoice"]),person("c2",contest=["Invoice"]),person("a1",indirect=["Invoice"]),person("cfo")],["Approver"])
w=copy.deepcopy(v);w["observation"]["sources"][0]["contest"]=source()["contest"];w["observation"]["people"]=[person("c1"),person("c2"),person("a1",indirect=["Invoice"]),person("cfo")]
b=replace(a,"recheck() -> Verdict { reads [Invoice];","recheck() -> Verdict {")
pair(4,"o-contest","route-no-read",a,b,v,w,"Normal reads belongs only to anonymous Reader. Declared route recheck reaches Invoice for c1 and c2 without a process call. Hence route count is 0..0 and contest_route stays true after reads removal. approval pass/fail selects posted/returned. Response duration 1min is within 5min; a1 has Authority. Both roots satisfied, no diagnostics.")
contest_a,contest_b,contest_v,contest_w=a,b,v,w
a=model([cap("Checker","check")],"j = Checker.check(); if j == pass { end done } else { end returned }")
v=view([end("done",[sel("j","Checker.check")]),end("returned",[sel("j","Checker.check")])],[retention("Checker.check")],actors=["Checker"])
w=view([end("done",read=[("j","Checker.check")])],[retention("Checker.check")])
pair(5,"f-select","merged-ends",a,replace(a,"end returned","end done"),v,w,"Enumerate j=pass,fail: distinct ends are each selected. Merging labels means both values reach done, so j is read_only, one end row and no F Actor.")
a=model([cap("A"),cap("B")],"a = A.draw(); b = B.draw(); if a == pass || b == pass { end done } else { end returned }")
v=view([end("done",[sel("a","A.draw","some_contexts"),sel("b","B.draw","some_contexts")]),end("returned",[sel("a","A.draw"),sel("b","B.draw")])],[retention("A.draw"),retention("B.draw")],actors=["A","B"])
w=copy.deepcopy(v)
for e in w["framing"]["ends"]:
    for s in e["selectors"]:s["selects"]="all_contexts" if e["end"]=="done" else "some_contexts"
pair(6,"f-or","and",a,replace(a," || "," && "),v,w,"Four assignments pp,pf,fp,ff. OR done={pp,pf,fp}, returned={ff}; AND done={pp}, returned={pf,fp,ff}. Unreachable contexts do not enter the denominator. Each call executes once.")
or_a,or_v=a,v
a=model([cap("Gate"),cap("Writer",fields="writes flag;")],"g = Gate.draw(); if g == pass { end done } else { w = Writer.draw(); if flag == pass { end returned } else { end returned } }",extra="observed case { flag: optional<V> }")
v=view([end("done",[sel("g","Gate.draw")]),end("returned",state=True)],[retention("Gate.draw"),retention("Writer.draw",0,1)],actors=["Gate"])
w=view([end("done",[sel("g","Gate.draw")]),end("returned",[sel("g","Gate.draw")],[("w","Writer.draw")])],[retention("Gate.draw"),retention("Writer.draw",0,1)],actors=["Gate"])
pair(7,"f-state","binding-guard",a,replace(a,"if flag == pass","if w == pass"),v,w,"Only returned crosses a state guard; retain not_derived/state_guards without reachable or empty arrays. done remains derived. The state-dependent two inner branches have equal Writer counts and the same end label. Binding replacement makes w read_only and completes F.")
a=model([cap("Ledger","post","pass")],"p = Ledger.post(); if p == pass { end done } else { end returned }")
v=view([end("done",read=[("p","Ledger.post")]),end("returned",reachable=False)],[retention("Ledger.post")])
w=view([end("done",[sel("p","Ledger.post")]),end("returned",[sel("p","Ledger.post")])],[retention("Ledger.post")],actors=["Ledger"])
pair(8,"f-singleton","two-results",a,replace(a,"result nondet { pass }","result nondet { pass,fail }"),v,w,"The type has two values but the Capability support has only pass. Thus done is reachable, p read_only, returned unreachable. Adding fail makes each end selectable. No commits are declared.",diagnostics=(("W-UNREACH-1","W-UNREACH-2"),()))
a=model([cap("Gate"),cap("Worker","do","ok","T"),cap("Dormant","keep","ok","T")],"g = Gate.draw(); if g == pass { b = Worker.do() } else {}; end done")
v=view([end("done",read=[("g","Gate.draw")])],[retention("Gate.draw"),retention("Worker.do",0,1),retention("Dormant.keep",0,0)])
w=copy.deepcopy(v);w["retention"]["capabilities"][1]["normal"]={"min":1,"max":1}
pair(9,"l-counts","unconditional",a,replace(a,"if g == pass { b = Worker.do() } else {}","if g == pass {} else {}; b = Worker.do()"),v,w,"Two paths give Worker counts {1,0}, then {1,1} after relocation. Gate always 1; Dormant always 0 and its row must not disappear. done is reached for both g values. No removal recommendation follows from zero exercise.",diagnostics=(("W-UNREACH-0",),("W-UNREACH-0",)))
a=model([cap("Judge","check","pass,fail","T","writes approval;"),cap("Post","post","posted","T","requires approval == pass else denied;")],"j = Judge.check(); p = Post.post(); end done",extra="observed case { approval: optional<T> }")
a=replace(a,"type T = enum { ok }","type T = enum { pass,fail,posted,denied }")
v=view([end("done",read=[("j","Judge.check")])],[retention("Judge.check"),retention("Post.post",0,1)])
w=view([end("done")],[retention("Judge.check"),retention("Post.post")])
pair(10,"l-requires","no-requires",a,replace(a,"requires approval == pass else denied;",""),v,w,"j=pass starts Post once; j=fail defines p=denied without starting Post. Count range 0..1. With no requires Post always starts. Domain(p) is evaluation support, not context support (Q5); the unconditional end reads j but j is not a selector.")
a=model([cap("Worker","do",qualification="capacity 2")],"fork f { a = Worker.do(); b = Worker.do() } join all f; end done")
v=view([end("done")],[retention("Worker.do",2,2)])
w=view([end("done")],[retention("Worker.do")])
pair(11,"l-fork","missing-second-call",a,replace(a,"; b = Worker.do()",""),v,w,"Two binding sites of the same Capability each execute once: 1+1=2. A one-branch fork has one site. Time max is not the exercise count sum.")
a=model([cap("A",fields="cancel immediate;"),cap("B",fields="cancel immediate;")],"fork f { a = A.draw(); b = B.draw() } join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> returned; end done; end returned")
v=copy.deepcopy(or_v)
for r in v["retention"]["capabilities"]:r["join_any_cancellation"]=True
w=view([end("done",read=[("a","A.draw")])],[retention("A.draw"),retention("B.draw",0,1)])
b=replace(a,"fork f { a = A.draw(); b = B.draw() } join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> returned; end done; end returned","a = A.draw(); if a == pass {} else { b = B.draw() }; end done")
pair(12,"l-any","sequential",a,b,v,w,"Any success pp,pf,fp reaches done; ff reaches if_none returned. Both lexical siblings count once including cancellation, hence both 1..1 and flags true. Sequential short circuit makes B 0..1, no cancellation. Old DSL registers returned via an End after done; the reachable returned sink belongs to if_none, not that unreachable suffix. The IR-only case without a registering End is a separate private obligation.")
a=model([cap("Planner","work","ok","T")],"p = Planner.work(); end done")
v=view([end("done")],[retention("Planner.work")]);w=view([end("done")],[retention("Planner.propose")])
pair(13,"p-name","proposal-name",a,replace(a,"work","propose"),v,w,"Renaming work to propose changes references only. There is no proposal contract; both P objects must equal not_derived/no_proposal_contract. One call, no source/person/F candidates.")
p3=PAIRS[2];a=p3["models"][0];v=p3["views"][0]
w=json.loads(json.dumps(v).replace("Extractor","Worker"))
pair(14,"label","actor-rename",a,replace(a,"Extractor","Worker"),v,w,"Inverse Actor rename must recover the entire viewpoints value. Capacity is anonymous regardless of the Actor name; x1 does not gain direct reachability. Other metadata metamorphisms are tested separately.")
a=model([cap("Gate",fields="writes flag;"),cap("Worker","do","ok","T"),cap("Always","do","ok","T")],"g = Gate.draw(); if flag == pass { x = Worker.do() } else {}; z = Always.do(); end done",extra="observed case { flag: optional<V> }")
v=view([end("done",state=True)],[retention("Gate.draw"),retention("Worker.do",state=True),retention("Always.do")])
w=view([end("done",read=[("g","Gate.draw")])],[retention("Gate.draw"),retention("Worker.do",0,1),retention("Always.do")])
pair(15,"l-state-local","binding-guard",a,replace(a,"if flag == pass","if g == pass"),v,w,"Only Worker count depends on unknown state; prefix Gate and unconditional suffix Always each remain 1..1. F done crosses state and is not derived. Binding guard makes Worker 0..1 and done read_only(g).")
pair(16,"soft-budget","one-unit-short",or_a,or_a,or_v,or_v,"Identical DSL controls. The private limit injection (not a CLI flag) must compare each O/F/L used dimension k against k-1 and zero. Full expected payload is A06; shortage drops only that whole block to derivation_limit; report remains satisfied/0, no new diagnostic. P and all legacy fields stay equal. See agency_viewpoints_obligations.md.")
a=(ROOT/"fixtures/agency-origin/model.relca").read_text()
v=view([end("rejected",[sel("approval","Reviewer.review")]),end("done",[sel("approval","Reviewer.review")])],[retention("Reader.enter"),retention("Reviewer.review"),retention("Ledger.post",0,1)],[source(actors=["Reader","Reviewer"],unattributed=["Reader","Reviewer"])],actors=["Reviewer"])
pair(17,"old-budget","legacy-limit",a,a,v,v,"Identical agency-origin DSL controls. Reader and Reviewer each read Invoice directly; no people are declared, so both are unattributed. approval pass selects done and fail selects rejected; posted is not read by a condition, so it is not an F candidate. Reader=1, Reviewer=1, Ledger=0..1. The unchanged private injected_limits_accept_k_and_reject_k_plus_one_units_of_required_work uses this same input: k succeeds; k-1 is old U-SEM-0/unsupported/4 with agency null, not a soft viewpoints failure. Private derive does not prepend W-CUT-0; report does. See obligations file.",diagnostics=(("W-CUT-0",),("W-CUT-0",)))
a=(ROOT/"examples/invoice/configurations.relca").read_text()
b=replace(a,"process AsIsFlow {\n    entry = Clerk.enter()","process AsIsFlow {\n    entry = Extractor.extract()")
def invoice_view(ai):
    return view([end("returned",[sel("approvalV","Approver.approve","some_contexts")],[("poV","Clerk.match_po"),("receiptV","Clerk.match_receipt")]),end("posted",[sel("poV","Clerk.match_po"),sel("receiptV","Clerk.match_receipt"),sel("approvalV","Approver.approve")],[("posted","Ledger.post")]),end("exception",reachable=False)],
        [retention("Clerk.enter",int(not ai),int(not ai)),retention("Clerk.match_po"),retention("Clerk.match_receipt"),retention("Clerk.recheck",0,0,True),retention("Extractor.extract",int(ai),int(ai)),retention("Approver.approve",0,1),retention("Ledger.post",0,1)],
        [source(actors=["Extractor" if ai else "Clerk"],people=[] if ai else ["c1","c2"],unattributed=["Extractor"] if ai else [],ca=["Clerk"],cp=["c1","c2"],routes=["Clerk.recheck"]),source("PurchaseOrder",["Clerk"],["c1","c2"]),source("ReceiptRecord",["Clerk"],["c1","c2"])],
        [person(p,["PurchaseOrder","ReceiptRecord"] if ai else ["Invoice","PurchaseOrder","ReceiptRecord"],["Invoice"],["Invoice"]) for p in ["c1","c2"]]+[person("a1",indirect=["Invoice","PurchaseOrder","ReceiptRecord"]),person("cfo")], ["Clerk","Approver"])
codes=("W-LIN-2","W-RESP-1","W-RESP-0","W-RESP-0","W-UNREACH-0","W-UNREACH-1","W-UNREACH-2")
pair(18,"invoice","ai-transfer",a,b,invoice_view(False),invoice_view(True),"Only AsIsFlow entry producer changes; parallelization is NOT part of this fault. Direct Invoice moves Clerk c1,c2 → anonymous Extractor, but contest c1,c2 and matching sources remain. Indirect Invoice persists through matching. Counts enter/extract 1/0 → 0/1; approval/post 0..1. posted requires all three pass, returned permits a new approval binding to fail (existential Q4): poV/receiptV read_only, approvalV some_contexts. committed is singleton so exception unreachable. Error mass AsIs=93/125000=.000744; AI=231/125000=.001848 versus .001. Roots satisfied/0 → violated/1. All joins are all, flags false.",diagnostics=(codes,codes),verdicts=("satisfied","violated"),invocation=dict(command="check",configuration="AsIs"))
p1=PAIRS[0]
pair(19,"report-compat","old-key",p1["models"][0],p1["models"][0],p1["views"][0],p1["views"][0],"Identical DSL. The fault edits the saved report legacy agency.rules after execution, never the DSL or product verdict. Strict comparator must reject that one byte change, unchecked/origin edits, whitespace, stderr and exit changes. A01 hand counts 1..1 remain. See compatibility negative controls.")
a=model([cap("Judge"),cap("Other")],"j = Judge.draw(); if j == pass { end done } else { x = Other.draw(); if x == pass { end done } else { end returned } }")
v=view([end("done",[sel("x","Other.draw","some_contexts")],[("j","Judge.draw")]),end("returned",[sel("j","Judge.draw"),sel("x","Other.draw")])],[retention("Judge.draw"),retention("Other.draw",0,1)],actors=["Judge","Other"])
w=view([end("done",[sel("j","Judge.draw")]),end("returned",[sel("j","Judge.draw")],[("x","Other.draw")])],[retention("Judge.draw"),retention("Other.draw",0,1)],actors=["Judge"])
b=replace(a,"actor Other { capacity 1; capability draw() -> V {  duration fixed 1min; result nondet { pass,fail } } }","actor Other { capacity 1; capability draw() -> V { duration fixed 1min; result nondet { fail } } }")
pair(20,"f-new-binding","only-fail",a,b,v,w,"j=pass reaches done without defining x. Changing j to fail may still reach done with newly defined x=pass: existential reach, so j read_only. x selects in some contexts. Restricting Other to fail removes that bypass; done j all_contexts, no x candidate on any satisfiable done path. returned x read_only. Counts Other 0..1.",diagnostics=((),("W-UNREACH-1",)))
pair(21,"mutate","off-path-viewpoints",contest_a,contest_b,contest_v,contest_w,"DSL control is A04 route reads removal, tested as remove_read mutation too. New contest reach contour (2 people → 0) is only other_changes, not assigned_changes/detection. Wrapper bpr-default unsupported/4; original and executed route mutant report satisfied/0. Saved-output fault moves viewpoints into model.viewpoints; projection must reject the noncanonical location, not recursively delete it.")
a=model([invoice,"actor ScopeAnchor { source record }",cap("Reader","enter","ok","T","reads [Invoice];")],"e = Reader.enter(); end done",cut='cut K { include [ScopeAnchor,Reader]; exclude Invoice reason "source outside" loses "source internals" }')
b=replace(a,'include [ScopeAnchor,Reader];','include [ScopeAnchor]; exclude Reader reason "caller outside" loses "call";')
v=view([end("done")],[retention("Reader.enter")],[source(actors=["Reader"],unattributed=["Reader"]),source("ScopeAnchor")])
pair(22,"cut-source","excluded-caller",a,b,v,None,"Excluded source remains in old origin and the transposed O (Q3); no new E-CUT-1 for reads. Excluding the calling Reader is the existing E-CUT-1/invalid/3 with null Agency. Reader count 1 in accepted control. ScopeAnchor is an unused declared record with empty normal/contest sets; it keeps the Cut include list nonempty on both sides, as required by the old parser.",diagnostics=((),("E-CUT-1",)),verdicts=("satisfied","invalid"))
a=model([invoice,cap("Gate"),cap("Worker","do","ok","T","reads [Invoice];")],"g = Gate.draw(); end done; x = Worker.do(); end late")
v=view([end("done"),end("late",reachable=False)],[retention("Gate.draw"),retention("Worker.do",0,0)],[source(actors=["Worker"],unattributed=["Worker"])])
w=view([end("done",[sel("g","Gate.draw")]),end("late",[sel("g","Gate.draw")])],[retention("Gate.draw"),retention("Worker.do",0,1)],[source(actors=["Worker"],unattributed=["Worker"])],actors=["Gate"])
pair(23,"no-end-after-end","enable-suffix",a,replace(a,"end done; x =","if g == pass { end done } else {}; x ="),v,w,"O is lexical so Worker reads Invoice even after an unconditional end. F/L are path-based: Worker 0 and late unreachable; Gate 1, done has no guard. Conditional early end enables suffix only for fail: Worker 0..1 and g selects both ends.",diagnostics=(("W-UNREACH-0","W-UNREACH-2"),()))


def main():
    names=[]
    for p in PAIRS:
        for i,name in enumerate([p["name"],p["name"]+".broken-"+p["fault"]]):
            names.append(name)
            d=F/name;d.mkdir(exist_ok=True)
            (d/"model.relca").write_text(p["models"][i])
            e=dict(verdict=p["verdicts"][i],diagnostics=[dict(code=c,severity="error" if c.startswith("E-") else "warning") for c in p["diagnostics"][i]],agency=None if p["views"][i] is None else dict(viewpoints=p["views"][i]))
            (d/"expected.json").write_text(json.dumps(e,ensure_ascii=False,indent=2)+"\n")
            (d/"calc.md").write_text(f'# {p["id"]} {name}\n\nHand oracle before implementation (ADR 0034; design v0.2 §5.6, v0.2 §22.3).\n\n{p["calc"]}\n\nThis side: {p["verdicts"][i]}; exit '+str(dict(satisfied=0,violated=1,invalid=3)[p["verdicts"][i]])+f'; diagnostic codes in report order/multiset: {list(p["diagnostics"][i])}. No new Agency diagnostic.\n\nAll ordinary durations are fixed minutes. P is always not_derived. L basis unknown, fallback unchecked; zero exercise does not imply unnecessary. Empty derived collections differ from not_derived. The expected viewpoints payload is complete, ordered and independent of actual output.\n')
            if p["invocation"]:(d/"invocation.json").write_text(json.dumps(p["invocation"])+"\n")
    dest=ROOT/"crates/relca-core/tests/support/agency_viewpoints_population.json"
    dest.write_text(json.dumps(dict(names=names,pairs=[{k:p[k] for k in ("id","name","fault")} for p in PAIRS]),indent=2)+"\n")
    print(f"{len(names)} hand-oracle controls, including private-budget and saved-output pairs")


if __name__=="__main__":main()
