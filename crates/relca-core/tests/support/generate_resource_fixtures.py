"""Materialize ADR 0033 hand-authored inputs/oracles; never runs Relca.

Only rsrc-* fixture files and resource_pairs.json are written. Numbers below are
interval sums, not captured production outputs. Each pair is one explicit edit.
"""
from fractions import Fraction
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
PAIRS = []


def scalar(lo, hi=None, reason=None):
    return {"min": str(lo) if lo is not None else None,
            "max": str(lo if hi is None else hi) if lo is not None else None,
            "reason": reason}


def row(name, cap, occupancy, peak, hi=None, peak_hi=None, reason=None):
    low_util = Fraction(occupancy, cap * 10) if cap and reason is None else None
    high_util = Fraction(occupancy if hi is None else hi, cap * 10) if cap and reason is None else None
    return {"resource": name, "capacity": cap,
            "occupancy_time": scalar(None if reason else occupancy, hi, reason),
            "peak_units": scalar(None if reason else peak, peak_hi, reason),
            "utilization": scalar(low_util, high_util, reason or ("zero_capacity" if cap == 0 else None))}


def oracle(elapsed, work, resources, verdict="satisfied", codes=(), result=None, multi=None):
    def bounds(x):
        return {"min": x[0], "max": x[1]} if isinstance(x, tuple) else {"min": x, "max": x}
    terms = [] if elapsed is None or multi else [{"end": "done", "elapsed": bounds(elapsed),
            "work": bounds(work), "result_at": None if result is None else bounds(result)}]
    out = {"verdict": verdict, "diagnostics": [{"code": c} for c in codes],
           "bounds": {"truncated": verdict == "inconclusive"}, "terminals": terms,
           "properties": [
               {"id": "safety no_resource_overcommit", "verdict": "inconclusive" if verdict == "inconclusive" else "satisfied"},
               {"id": "reachable end.done", "verdict": "inconclusive" if verdict == "inconclusive" else "violated" if verdict == "violated" else "satisfied"}],
           "metrics": {"rules": "business-metrics-p1-resource-1", "resources": resources}}
    if multi:
        out["case_outcomes"] = [{"case_id": i + 1, "arrival": 0, "terminals": [
            {"kind": "end", "end": "done", "elapsed": bounds(t), "completed_at": bounds(t),
             "work": bounds(work), "result_at": None}]} for i, t in enumerate(multi)]
    return out


def static(verdict, code):
    codes = [code] + (["W-CUT-0"] if code in ("U-SYN-0", "E-SYN-0") else [])
    return {"verdict": verdict, "diagnostics": [{"code": c} for c in codes], "properties": [], "terminals": []}


def cap(name, time, uses="R: 1", result="ok", extra="", params=""):
    return f"capability {name}({params}) -> T {{ " + (f"uses {uses}; " if uses else "") + f"duration fixed {time}min; result nondet {{ {result} }}; {extra} }}"


def actor(caps, name="A", capacity: int | None =1, extra=""):
    return f"actor {name} {{ " + (f"capacity {capacity}; " if capacity is not None else "") + extra + " " + " ".join(caps) + " }"


def model(actors, process, resources="resource R capacity 1", types="ok", extra="", names="A", cases=1, arrivals=False):
    return (f'model ResourceAcceptance version "0.2" {{\n  type T = enum {{ {types} }}\n  {resources}\n  {actors}\n'
            f'  process P {{ {process} }}\n  cut K {{ question "resource acceptance"; include [{names}] }}\n'
            + ("  arrivals fixed [" + ",".join("0min" for _ in range(cases)) + "]\n  policy fifo tie_by case_id\n" if arrivals else "")
            + f'  bounds {{ cases {cases}; live_tasks 3; clock 10min }}\n  metrics {{ window 10min; currency "JPY" }}\n'
            + (f"  {extra}\n" if extra else "\n")
            + "  verify safety no_resource_overcommit\n  verify reachable end.done\n}\n")


def pair(id_, name, source, old, new, good, bad, derivation, fault, invocation=None):
    assert source.count(old) == 1, (id_, old, source.count(old))
    broken = source.replace(old, new, 1)
    assert broken != source
    name = "rsrc-" + name
    bad_name = name + ".broken-" + fault
    PAIRS.append({"id": id_, "good": name, "bad": bad_name, "from": old, "to": new})
    for n, s, expected, side in [(name, source, good, "control"), (bad_name, broken, bad, "single fault")]:
        d = ROOT / "fixtures" / n
        d.mkdir(exist_ok=True)
        (d / "model.relca").write_text(s)
        (d / "expected.json").write_text(json.dumps(expected, indent=2, ensure_ascii=False) + "\n")
        parse_note = ("\nThe static member/Actor-use side expects U-SYN-0 followed by W-CUT-0. "
                      "This is existing report behavior: parse failure returns before a later explicit Cut is resolved "
                      "(report.rs check_with). Both syntax positions were reproduced with the archived PRE-resource "
                      "binary, exit4 and these two codes. No resource semantic warning is added.\n"
                      if id_ in ("P16", "P17") else "")
        (d / "calc.md").write_text(f"# {id_}: {n}\n\nHand oracle fixed before implementation ({side}). Units: minutes and anonymous units.\n\n{derivation}\n{parse_note}\nSingle edit: {old!r} → {new!r}.\n\nNo resource price is configured; absence is not zero cost. A RED run is not semantic verification.\n")
        if invocation:
            (d / "invocation.json").write_text(json.dumps(invocation) + "\n")


ALL = "fork f { a = A.x(); b = A.y() }; join all f; end done"
DEAD = ("W-DEADLOCK-0", "W-UNREACH-0", "W-UNREACH-2")
ONE = model(actor([cap("x", 1)]), "x = A.x(); end done")
SINGLE = oracle(1, 1, [row("R", 1, 1, 1)])
EXACT = model(actor([cap("x", 3), cap("y", 5)], capacity=2), ALL, "resource R capacity 2")


def main():
    pair("P01", "exact", EXACT, "R capacity 2", "R capacity 1", oracle(5,8,[row("R",2,8,2)]), oracle(8,8,[row("R",1,8,1)]), "Both start at 0: [0,3)+[0,5), E=max(3,5)=5; O=W=8, P=2, U=8/20=2/5. Capacity1 serializes either order: E=8, P=1, U=4/5.", "one-unit")
    s=model(actor([cap("x",3)])+"\n  "+actor([cap("y",5)],"B"), "fork f { a = A.x(); b = B.y() }; join all f; end done", names="A,B")
    pair("P02","shared",s,"capability y() -> T { uses R: 1;","capability y() -> T {",oracle(8,8,[row("R",1,8,1)]),oracle(5,8,[row("R",1,3,1)]),"Shared R serializes independent Actor slots: E=3+5=8, O8/P1. Removing B.y use permits overlap: E5, O3/P1, U3/10. Work8 on both.","missing-use")
    s=model(actor([cap("x",3,"R: 2"),cap("y",2)],capacity=2),ALL,"resource R capacity 3")
    pair("P03","weighted",s,"R capacity 3","R capacity 2",oracle(3,5,[row("R",3,8,3)]),oracle(5,5,[row("R",2,8,2)]),"Demand2+1=3; O=2*3+1*2=8 but W=3+2=5. E3/P3/U8/30=4/15. Capacity2 makes E5/P2/U8/20=2/5.","less-capacity")
    s=model(actor([cap("x",3,"R: 1, S: 1"),cap("y",2,"S: 1, R: 1")],capacity=2),ALL,"resource R capacity 1; resource S capacity 1")
    pair("P04","atomic",s,"uses R: 1, S: 1","uses R: 2, S: 1",oracle(5,5,[row("R",1,5,1),row("S",1,5,1)]),oracle(None,2,[row("R",1,2,1),row("S",1,2,1)],"violated",DEAD),"Atomic all-or-none acquisition, despite reversed order: E5/W5/O5/P1 for each. Fault: only y[0,2) can run. At2 x needs R2, available1, missing1. No terminal. O2 for both, safety satisfied; deadlock violated.","excess-demand")
    s=model(actor([cap("x",2,"R: 2"),cap("y",3,"R: 2")]),"x = A.x(); y = A.y(); end done","resource R capacity 2")
    pair("P05","reuse",s,"capability y() -> T { uses R: 2","capability y() -> T { uses R: 3",oracle(5,5,[row("R",2,10,2)]),oracle(None,2,[row("R",2,4,2)],"violated",DEAD),"x[0,2) releases both units at2, y[2,5): O=2*2+2*3=10, P2, U1/2. Fault y3>capacity2 deadlocks after x; O4, U1/5, missing1.","excess-next")
    cancel=model(actor([cap("a",2,result="pass",extra="cancel cooperative ack_within 1min"),cap("b",6,result="fail",extra="cancel cooperative ack_within 1min"),cap("c",1,"R: 2",result="pass")],capacity=2),"fork f { a = A.a(); b = A.b() }; join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done; c = A.c(); end done","resource R capacity 2",types="pass,fail")
    old="result nondet { fail }; cancel cooperative ack_within 1min"
    pair("P06","cooperative",cancel,old,"result nondet { fail }; cancel forbidden_after_start",oracle(4,6,[row("R",2,7,2)],result=2),oracle(7,9,[row("R",2,10,2)],result=2),"a[0,2), b[0,3) ack3, c[3,4) uses2: W2+3+1=6, O2+3+2=7, U7/20. Forbidden b[0,6), c[6,7): W9/O10/U1/2. Result adopted at2 in both.","drain")
    immediate=cancel.replace(old,"result nondet { fail }; cancel immediate"); assert immediate!=cancel
    pair("P07","immediate",immediate,"result nondet { fail }; cancel immediate",old,oracle(3,5,[row("R",2,6,2)],result=2),oracle(4,6,[row("R",2,7,2)],result=2),"Request2 retains b until distinct ack2. Then c[2,3). W2+2+1=5/O2+2+2=6/P2/U3/10. Cooperative ack3 gives E4/W6/O7.","cooperative")
    pair("P08","request-held",cancel,old,"result nondet { fail }; cancel cooperative ack_within 2min",oracle(4,6,[row("R",2,7,2)],result=2),oracle((4,5),(6,7),[row("R",2,7,2,hi=8)],result=2),"At request2 b still holds1. ack3 or4; c starts only after ack, lasts1 using2. E4..5/W6..7/O7..8/U7/20..2/5. Request alone cannot free a unit.","longer-ack")
    s=model(actor([cap("x",3,"R: 2",extra="requires flag == true else denied"),cap("y",2,"R: 2")]),"x = A.x(); y = A.y(); end done","resource R capacity 2",types="ok,denied",extra="observed case { flag: bool = false }")
    pair("P09","refusal",s,"flag: bool = false","flag: bool = true",oracle(2,2,[row("R",2,4,2)],codes=("W-UNREACH-0",)),oracle(5,5,[row("R",2,10,2)]),"False gate refuses x at0: starts0/work0/no occupancy; y[0,2) uses2 yields O4/P2/U1/5. True gate: x[0,3),y[3,5), O10/W5. No branch or unused end is added.","enabled")
    s=model(actor([cap("x",3,extra="writes flag; finish { flag = true }"),cap("y",1,extra="requires flag == true else denied")],capacity=2),ALL,types="ok,denied",extra="observed case { flag: bool = false }")
    pair("P10","gate-after-wait",s,"capability y() -> T { uses R: 1;","capability y() -> T {",oracle((3,4),(3,4),[row("R",1,3,1,hi=4)]),oracle(3,3,[row("R",1,3,1)],codes=("W-UNREACH-0",)),"x-first: x[0,3), y sees true at3 and runs[3,4). y-first: refuses at0, x ends3. Thus both E3 and4/O3 and4. Removing use lets y always refuse at0 before clock can advance.","no-use")
    s=model(actor([cap("x",2)],capacity=2)+"\n  "+actor([cap("y",3)],"B",2),"x = A.x(); y = B.y(); end done",names="A,B",cases=2,arrivals=True)
    # Keep the two declarations fixed; selecting A.y is the single boundary edit.
    # A.y duplicates B.y solely for this boundary; unused call warning is expected.
    previous=s
    s=s.replace("capability x()", "capability y() -> T { uses R: 1; duration fixed 3min; result nondet { ok } } capability x()")
    assert s!=previous
    pair("P11","competition",s,"y = B.y()","y = A.y()",oracle(None,5,[row("R",1,10,1)],codes=("W-UNREACH-0",),multi=[(5,7),10]),oracle(None,5,[row("R",1,10,1)],codes=("W-UNREACH-0",),multi=[7,10]),"FIFO is Actor-local. x1[0,2),y1[2,5),x2[5,7),y2[7,10) gives case1=5. x1[0,2),x2[2,4),y1[4,7),y2[7,10) gives7. case2=10. Same Actor y queues behind x2, leaving only7. Both O10/P1/U1. Inactive duplicate capability produces one existing never_started warning in each side, not a resource warning.","same-actor")
    s=model(actor([cap("x",2)],capacity=2),"x = A.x(); end done",cases=2,arrivals=True)
    pair("P12","global",s,"R capacity 1","R capacity 2",oracle(None,2,[row("R",1,4,1)],multi=[2,4]),oracle(None,2,[row("R",2,4,2)],multi=[2,2]),"FIFO x1[0,2),x2[2,4): cases2/4, O4/P1/U2/5. R2 permits both[0,2): cases2/2,O4/P2/U1/5. No resource copying per case.","two-units")
    s=model(actor([cap("x",3),cap("y",5)],capacity=None,extra="executors [p];"),ALL,"resource R capacity 2",extra="people [p,q]")
    pair("P13","people",s,"executors [p]","executors [p,q]",oracle(8,8,[row("R",2,8,1)]),oracle(5,8,[row("R",2,8,2)]),"One person cannot overlap tasks, although R2: E8/W8/O8/P1. Two people infer Actor capacity2: E5/P2. U=8/20=2/5 both. Anonymous units are not people.","two-people")
    pair("P14","over-demand",ONE,"uses R: 1","uses R: 2",SINGLE,oracle(None,0,[row("R",1,0,0)],"violated",DEAD),"Control x[0,1): E1/W1/O1/P1/U1/10. Demand2>capacity1 is legal lower, but no start: deadlock0, safety satisfied, no terminal. O0/P0/U0, required2/available1/missing1.","too-much")
    s=model(actor([cap("x",1,uses="")]),"x = A.x(); end done","resource R capacity 0")
    pair("P15","unused-zero",s,"capability x() -> T {","capability x() -> T { uses R: 1;",oracle(1,1,[row("R",0,0,0)]),oracle(None,0,[row("R",0,0,0)],"violated",DEAD),"Unused zero capacity is legal, E1/W1/O0/P0; utilization null/zero_capacity. Adding uses1 makes deadlock0, O0/P0 and same null reason. Unused assumption suffix only on control.","used")
    for id_,name,old,new,v,c,calc,fault in [
        ("P16","members","R capacity 1","R capacity 1 members [p]","unsupported","U-SYN-0","Reject members syntax before resolving undeclared p.","members"),
        ("P17","actor-use","actor A {","actor A { uses R;","unsupported","U-SYN-0","Actor-level uses is not Capability uses.","actor-use"),
        ("P18","collision","resource R capacity 1","resource R capacity 1; resource A capacity 1","unsupported","U-SEM-0","Resource A collides with Actor A, regardless declaration order.","same-name"),
        ("P19","zero-amount","uses R: 1","uses R: 0","invalid","E-CAP-0","Zero amount is not omission.","zero"),
        ("P20","unknown","uses R: 1","uses Missing: 1","invalid","E-NAME-0","Only use name changes; R remains declared.","unknown"),
        ("P21","duplicate","uses R: 1","uses R: 1, R: 1","unsupported","U-SEM-0","Duplicate entries are not sum2 or set1.","duplicate"),
        ("P22","second-use","uses R: 1;","uses R: 1; uses R: 1;","unsupported","U-SEM-0","Second clause rejected rather than merged or replaced.","second-use"),
        ("P23","actor-capacity","capacity 1;  capability"," capability","invalid","E-CAP-0","Positive duration needs Actor capacity even when R has capacity1.","no-capacity")]:
        pair(id_,name,ONE,old,new,SINGLE,static(v,c),"Control E1/W1/O1/P1/U1/10. "+calc+" Fault is static: no runtime numbers.",fault)
    s=model(actor([cap("a",2,result="pass",extra="cancel cooperative ack_within 1min"),cap("b",6,result="fail",extra="cancel forbidden_after_start"),cap("consume",1,uses="",result="pass",params="previous: T")],capacity=2),"fork f { a = A.a(); b = A.b() }; join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done; z = A.consume(b); end done","resource R capacity 2",types="pass,fail")
    pair("P24","all-start",s,"R capacity 2","R capacity 1",oracle(7,9,[row("R",2,8,2)],result=2),static("unsupported","U-SEM-0"),"R2 proves both start0; a finishes2, forbidden b finishes6, consume[6,7), E7/W9/O8/P2/U2/5. R1 cannot prove b started before adoption; b may be cancelled while queued, hence its later result read is unsupported, not a runtime value.","no-proof")
    pair("P25","clock",EXACT,"clock 10min","clock 4min",oracle(5,8,[row("R",2,8,2)]),oracle(None,0,[row("R",2,0,0,reason="truncated")],"inconclusive"),"Only clock changes, not metric window10. x finishes3, y unfinished at4; no terminal, no deadlock/budget warning. Every resource scalar null/truncated; safety inconclusive.","short-clock")
    s=model(actor([cap("x",2,"R: 2"),cap("y",3,"R: 2")],capacity=2),ALL,"resource R capacity 2")
    pair("H02","release-fork",s,"R capacity 2","R capacity 4",oracle(5,5,[row("R",2,10,2)]),oracle(3,5,[row("R",4,10,4)]),"Fork retains Done sibling while other task waits. Correct R2 serial E5/O10/P2; R4 overlaps E3/O10/P4. A Done-holder implementation deadlocks at2 or3, O4..6; both capabilities can start on different paths so only W-DEADLOCK-0/W-UNREACH-2, no never_started warning. Internal fault injection needs separate execution evidence.","parallel")
    s=model(actor([cap("a",2,uses="",result="pass",extra="cancel cooperative ack_within 1min"),cap("b",6,result="fail",extra="cancel cooperative ack_within 1min")],capacity=2),"fork f { a = A.a(); b = A.b() }; join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done; end done","resource R capacity 0",types="pass,fail")
    pair("H06","queued-cancel",s,"R capacity 0","R capacity 1",oracle(2,2,[row("R",0,0,0)],codes=("W-UNREACH-0",),result=2),oracle(3,5,[row("R",1,3,1)],result=2),"R0: b never starts and cancels queued at2, starts0/cancelled_before_start1. a consumes no R, W2/O0/P0/null zero_capacity. R1: b[0,3) with ack3, E3/W5/O3/P1. Queue is not holding.","startable")
    s=model(actor([cap("x",1,"R: 18446744073709551615"),cap("y",1)],capacity=2),ALL,"resource R capacity 18446744073709551615")
    pair("H08","huge",s,"uses R: 18446744073709551615","uses R: 18446744073709551614",oracle(2,2,[row("R",18446744073709551615,18446744073709551616,18446744073709551615)]),oracle(1,2,[row("R",18446744073709551615,18446744073709551615,18446744073709551615)]),"MAX plus1 is incompatible, not arithmetic abort: serial E2/W2/O=2^64/P=MAX/U=2^64/(10*MAX). Reducing x by1 fits both: E1/O=MAX/P=MAX/U1/10. Big rational metric products must not overflow u64.","fits")
    s=model(actor([cap("x",0),cap("y",1)],capacity=2),ALL)
    pair("H09","zero-time",s,"duration fixed 0min","duration fixed 1min",oracle(1,1,[row("R",1,1,1)]),oracle(2,2,[row("R",1,2,1)]),"Engine-only: zero task acquires1 at start and releases at finish at0, W0/O0 contribution. y contributes1: E1/W1/O1/P1. Positive control E2/W2/O2/P1. Independent reference deliberately excludes the zero-time input, not silently.","positive")
    s=model(actor([cap("x",2,"R: 2"),cap("y",1)],capacity=2),"x = A.x(); y = A.y(); end done","resource R capacity 3")
    pair("H16","half-open",s,"duration fixed 2min","duration fixed 3min",oracle(3,3,[row("R",3,5,2)]),oracle(4,4,[row("R",3,7,2)]),"[0,2)*2+[2,3)*1 yields O5/P2/U1/6, not closed-interval P3. Longer x: [0,3)*2+[3,4)*1 gives O7/P2/U7/30. Safety remains satisfied in both.","longer")
    # Keep the full single-edit list alongside the tests, independent of parser.
    Path(__file__).with_name("resource_pairs.json").write_text(json.dumps(PAIRS,indent=2)+"\n")
    print(f"{len(PAIRS)} hand pairs; {2*len(PAIRS)} directories")


if __name__ == "__main__":
    main()
