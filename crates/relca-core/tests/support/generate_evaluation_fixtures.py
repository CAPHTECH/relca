"""ADR0047 hand-only materializer. Never invokes/imports Relca or captures values.

The old-language substrates are existing authored inputs. Arithmetic below is
the plan's rational arithmetic, not an oracle learned from an implementation.
Run with python3 -B. Output edits have exact, non-noop witnesses.
"""
import argparse
import copy
import hashlib
import json
from fractions import Fraction as F
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
SUPPORT = Path(__file__).resolve().parent
INVENTORY = {}
PAIRS = []
SOURCES = {}
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--fixture-root",type=Path,default=ROOT/"fixtures")
OUTPUT=parser.parse_args().fixture_root
OUTPUT.mkdir(parents=True,exist_ok=True)
S, V, I, U, X, Q = "satisfied violated inconclusive unsupported invalid vacuous".split()


def text(x):
    return str(F(x))


def edit(s, a, b):
    assert a in s, a
    r = s.replace(a, b, 1)
    assert r != s, a
    return r


def edit_all(s,a,b):
    assert a in s,a
    changed=s.replace(a,b)
    assert changed!=s,a
    return changed


def append(s, declaration):
    at = s.rfind("}")
    return s[:at] + declaration + "\n" + s[at:]


def inv(plans, mode="exhaustive", scenario=None):
    r = dict(command="compare", plans=plans, mode=mode)
    if scenario:
        r["scenario"] = scenario
    return r


def hard(names, cohort, op, bound, intervals, verdicts, basis="exact", reasons=None,
         points=None) -> dict[str, Any]:
    bound = text(bound)
    vals = []
    for j, name in enumerate(names):
        a = intervals[j]
        r = dict(configuration=name, verdict=verdicts[j],
                 basis=basis if a is not None else None,
                 interval=None if a is None else dict(lower=text(a[0]), upper=text(a[1])),
                 reason=None if reasons is None else reasons[j], source_reason=None)
        if points is not None:
            r["point"] = None if points[j] is None else text(points[j])
        vals.append(r)
    return dict(constraint=f"cohort.{cohort} {op} {bound}", cohort=cohort,
                op=op, bound=bound, values=vals)


def objective(names, ref, ranges, direction="minimize", basis="path_range", reasons=None,
              sampled=False) -> dict[str, Any]:
    vals = []
    for j, name in enumerate(names):
        a = ranges[j]
        r = dict(configuration=name)
        reason = None if reasons is None else reasons[j]
        if sampled and ref.startswith("cohort."):
            r.update(point=None if a is None else text(a[0]), reason=reason)
        elif sampled:
            r.update(sample=dict(mean=text(a[0]), min=text(a[0]), max=text(a[1]),
                                 samples=3, reason=reason), reason=reason)
        else:
            r.update(lower=None if a is None else text(a[0]),
                     upper=None if a is None else text(a[1]),
                     basis=None if a is None else basis, reason=reason)
        vals.append(r)
    return dict(objective=f"{direction} {ref}", direction=direction, values=vals)


def evaluation(names, hs=(), os=(), verify=None, nd=None, dominated=(), unknown=(),
               sampled=False, m=0, confidence="1/2", under=None, scenario=None, cut: str | None="C") -> dict[str,Any]:
    verify = verify or [S] * len(names)
    rank = {S: 0, Q: 1, I: 2, U: 3, V: 4, X: 5}
    plans = []
    for j, name in enumerate(names):
        hv = max([h["values"][j]["verdict"] for h in hs] or [S], key=lambda v: rank[v])
        status = "excluded" if V in [hv, verify[j]] else (
            "admissible" if hv == verify[j] == S else "undetermined")
        reasons = []
        if verify[j] != S:
            reasons.append(dict(kind="verify", constraint=None, verdict=verify[j], reason=None))
        for h in hs:
            c = h["values"][j]
            if c["verdict"] != S:
                reasons.append(dict(kind="hard", constraint=h["constraint"],
                                    verdict=c["verdict"], reason=c["reason"]))
        plans.append(dict(configuration=name, verify_verdict=verify[j], hard_verdict=hv,
                          status=status, reasons=reasons))
    among = [p["configuration"] for p in plans if p["status"] == "admissible"]
    reason = "simulation_estimates" if sampled else (
        "no_objectives" if not os else ("no_admissible_plans" if not among else None))
    p = dict(status="computed" if reason is None else "not_computed", reason=reason,
             among=among, non_dominated=(nd if nd is not None else among) if reason is None else [],
             dominated=[dict(configuration=n, dominated_by=by) for n, by in dominated],
             undetermined_pairs=[dict(from_=a, to=b, reason=r) for a, b, r in unknown],
             not_evaluated=[dict(configuration=p["configuration"], status=p["status"])
                            for p in plans if p["status"] != "admissible"])
    for pair in p["undetermined_pairs"]:
        pair["from"] = pair.pop("from_")
    r:dict[str,Any] = dict(rules="compare-evaluation-p1-1",
             declared=dict(plans=names, under=under, scenario=scenario, cut=cut),
             hard=list(hs), plans=plans, objectives=list(os), pareto=p)
    if sampled:
        r["statistics"] = dict(interval_method="clopper-pearson-per-case-sum",
                               multiplicity=dict(method="bonferroni", m=m,
                                                 alpha=text((1-F(confidence))/(2*m)) if m else None),
                               confidence=text(confidence))
    return r


def put(name, source, invocation, verdict, ev=None, code=None, calc="", ident="", **extra):
    assert name not in INVENTORY
    expected = dict(verdict=verdict)
    if ev is not None:
        expected["evaluation"] = ev
    SOURCES[name]=source
    directory = OUTPUT / name
    directory.mkdir(exist_ok=True)
    # Calculation is installed before the source/expectation.
    (directory / "calc.md").write_text(
        f"# {name} — ADR 0047 {ident}\n\n"
        "Hand oracle, authored before implementation. Stage C RED is not verified semantics.\n\n"
        + calc + "\n\nExpected root: " + verdict + ".\n"
        + ("Closed evaluation:\n\n```json\n" + json.dumps(ev, indent=2) + "\n```\n" if ev else ""))
    (directory / "model.relca").write_text(source)
    (directory / "invocation.json").write_text(json.dumps(invocation, indent=2)+"\n")
    (directory / "expected.json").write_text(json.dumps(expected, indent=2)+"\n")
    INVENTORY[name] = dict(invocation=invocation, code=code, id=ident, **extra)


def pair(base, broken, before, after):
    # One contiguous source replacement, or an explicit invocation-only fault.
    pre = 0
    while pre < min(len(before), len(after)) and before[pre] == after[pre]:
        pre += 1
    end = 0
    while end < min(len(before), len(after))-pre and before[-1-end] == after[-1-end]:
        end += 1
    PAIRS.append(dict(base=base, broken=broken, start=pre,
                      end=len(before)-end, old=before[pre:len(before)-end],
                      new=after[pre:len(after)-end], invocation_only=before == after))


def change(base, suffix, a, b, verdict, ev=None, code=None, calc="", invocation=None):
    source = SOURCES[base]
    changed = edit(source, a, b)
    name = base + ".broken-" + suffix
    put(name, changed, invocation or INVENTORY[base]["invocation"], verdict,
        ev, code, calc, INVENTORY[base]["id"])
    pair(base, name, source, changed)
    return name


QPLUS = (ROOT / "fixtures/qual-compare/model.relca").read_text()
QPLUS = append(QPLUS, '  metrics { window 30min; currency "JPY"; rate Checker per_minute 1; '
               'rate Ledger per_minute 1; rate Checker.check per_start 0; '
               'rate Checker.quick per_start 0; rate Ledger.post per_start 0 }')
N = ["Check", "Fast"]
H = hard(N, "WrongPerArrival", "<=", "1/20", [("1/50", "1/50"), ("1/10", "1/10")], [S,V])
WORK = objective(N, "metrics.overall.work", [(6,8),(4,4)])
DECL = "  compare [Check, Fast] cut C hard { cohort.WrongPerArrival <= 0.05 } objectives { minimize metrics.overall.work }"
src = append(QPLUS, DECL)
put("eval-hard", src, inv(N), V, evaluation(N,[H],[WORK]), calc=
    "T01: per arrival Wrong=1/50 for Check,1/10 for Fast. 1/50<=1/20<1/10. "
    "Each Check path is fail3/pass4; two cases give work[6,8]. Fast=2*(1+1)=4. "
    "Hard excludes Fast; among/non_dominated=[Check].", ident="T01")
change("eval-hard", "no-hard", " hard { cohort.WrongPerArrival <= 0.05 }", "", S,
       evaluation(N,os=[WORK],nd=["Fast"],dominated=[("Check",["Fast"])]), calc=
       "Remove only hard block. Both verify S;4<6, so Fast surely dominates Check.")
change("eval-hard", "latent-fast", "process Quick {", "process Quick { if correct == true {} else {};", X,
       code="E-POL-LATENT", calc="T01L: latent reads are evaluation-only, not process conditions.")

C = '''model Contention version "0.2" {
  type T = enum { pass }
  actor A { capacity 1; capability run() -> T { duration fixed 3min; result nondet { pass } } }
  actor B { capacity 2; capability run() -> T { duration fixed 4min; result nondet { pass } } }
  process P { a = A.run(); end done }
  process R { b = B.run(); end done }
  cut C { include [A, B] }
  configuration One { cut C; process P }
  configuration Two { cut C; process R }
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 2; clock 12min }
  policy fifo tie_by case_id
  verify reachable end.done
  property case.OnTime = reached(end.done) within 4min
  metric cohort.OnTimePerArrival = expected(count(case.OnTime)) / arrivals
  metrics { window 12min; currency "JPY"; rate A per_minute 1; rate B per_minute 1; rate A.run per_start 0; rate B.run per_start 0 }
}
'''
CN=["One","Two"]
CH=hard(CN,"OnTimePerArrival",">=","1/2",[("1/2","1/2"),(1,1)],[S,S])
CW=objective(CN,"metrics.overall.work",[(6,6),(8,8)])
CD="  compare [One, Two] cut C hard { cohort.OnTimePerArrival >= 0.5 } objectives { minimize metrics.overall.work }"
put("eval-contention",append(C,CD),inv(CN),S,
    evaluation(CN,[CH],[CW],nd=["One"],dominated=[("Two",["One"])]),calc=
    "T02: FIFO completions One3,6 and Two4,4. Inclusive deadline4 gives (1+0)/2=1/2 "
    "and (1+1)/2=1. work6<8. Both hard S;One dominates.",ident="T02")
ch=hard(CN,"OnTimePerArrival",">=","1/2",[(0,1),(1,1)],[I,S])
ch["values"][0]["basis"]="per_case_relaxation"
change("eval-contention","no-policy","  policy fifo tie_by case_id\n","",I,
       evaluation(CN,[ch],[CW]),calc="Each fixed One case may finish3 or6. Casewise [0,1] summed/2 "
       "is [0,1], not exact1/2, so hard I. Two stays S; among=[Two].")

# Computational clock control (no dependence on the deferred B6 horizon join).
HC=edit(QPLUS,"capacity 2; capability quick", "capacity 1; capability quick")
HC=edit(HC,"duration fixed 3min", "duration fixed 4min")
HC=edit(HC,"arrivals fixed [0min, 10min]","arrivals fixed [0min, 6min, 12min]")
HC=edit(HC,"cases 2;", "cases 3;")
HC=edit(HC,"configuration Fast { cut C; process Quick }","configuration Fast { cut C; process Flow }")
HD="  compare [Check, Fast] cut C hard { cohort.WrongPerArrival <= 0.3 }"
hh=hard(N,"WrongPerArrival","<=","3/10",[("1/50","1/50")]*2,[S,S])
put("cmp7-clock",append(HC,HD),inv(N),S,evaluation(N,[hh]),calc=
    "T03C: arrivals0,6,12; successful completion5,11,17 (return4,10,16). All finish by30. "
    "E[Wrong]=3/50; divided by3 gives1/50.",ident="T03C")
hh=hard(N,"WrongPerArrival","<=","3/10",[("1/75","26/75")]*2,[I,I],
        "with_undetermined",["truncated"]*2)
change("cmp7-clock","short-clock","clock 30min","clock 14min",I,
       evaluation(N,[hh],verify=[I,I]),calc="First two expected Wrong=1/25. Last unknown[0,1]. "
       "[(1/25)/3,(1/25+1)/3]=[1/75,26/75] crosses3/10. Computational reason truncated.")
# Two independent bounds on the same clock-cut substrate catch always-I and wrong-end decisions.
for suffix,b,ver in [("loose","1/2",S),("strict","1/100",V)]:
    base="cmp7-clock-"+suffix
    hs=copy.deepcopy(hh)
    put(base,edit(append(edit(HC,"clock 30min","clock 14min"),HD),"<= 0.3", "<= 0.5" if ver==S else "<= 0.01"),
        inv(N),I if ver==S else V,
        evaluation(N,[hard(N,"WrongPerArrival","<=",b,[("1/75","26/75")]*2,[ver]*2,
                           "with_undetermined",["truncated"]*2)],verify=[I,I]),
        calc="T03L/S computational counterpart: 26/75<=1/2 implies hard S; "
        "1/75>1/100 implies hard V, even with unknown. Verify I is never admissible.",ident="T03L/S-clock")
    change(base,"crossing", "<= 0.5" if ver==S else "<= 0.01", "<= 0.3",I,
           evaluation(N,[hh],verify=[I,I]),calc="Change only bound to3/10; both endpoints straddle it.")

Z=edit(QPLUS,"verify reachable end.posted","verify reachable end.returned")
Z=edit(Z,"configuration Fast { cut C; process Quick }","configuration Fast { cut C; process Flow }")
ZD="  compare [Check, Fast] cut C hard { cohort.WrongPerRegistered <= 0.05 }"
zh=hard(N,"WrongPerRegistered","<=","1/20",[("1/46","1/46")]*2,[S,S])
put("eval-zero",append(Z,ZD),inv(N),S,evaluation(N,[zh]),calc=
    "T04: E Wrong=1/25 and E Registered=46/25; ratio=(1/25)/(46/25)=1/46<1/20. "
    "Verify reachable returned is used on BOTH sides of the pair.",ident="T04")
change("eval-zero","all-fail",
       "0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false",
       "1: fail with correct = false",I,
       evaluation(N,[hard(N,"WrongPerRegistered","<=","1/20",[None]*2,[I,I],
                          reasons=["zero_denominator"]*2)]),calc=
       "One kernel distribution changes. Both verify returned S;N=D=0. 0/0 is null, not0 or S.")

SIM='''model Simulation version "0.2" {
  type V = enum { pass }
  type Ack = enum { committed }
  observed case { registered: bool = false }
  latent case { correct: bool = false }
  kernel good { * -> { 1: pass with correct = true } }
  kernel bad { * -> { 1: pass with correct = false } }
  actor Checker { capacity 2; capability check() -> V { duration fixed 3min; outcomes kernel good; writes correct }; capability quick() -> V { duration fixed 1min; outcomes kernel bad; writes correct } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }
  process GoodFlow { v = Checker.check(); a = Ledger.post(); end posted }
  process BadFlow { v = Checker.quick(); a = Ledger.post(); end posted }
  cut C { include [Checker, Ledger] }
  configuration Good { cut C; process GoodFlow }
  configuration Bad { cut C; process BadFlow }
  arrivals fixed [0min]
  bounds { cases 1; live_tasks 2; clock 20min }
  simulation replications 3 seed 1 confidence 0.5
  verify reachable end.posted
  property case.Wrong = reached(registered == true && correct == false)
  property case.Registered = reached(registered == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))
  metrics { window 20min; currency "JPY"; rate Checker per_minute 1; rate Ledger per_minute 1; rate Checker.check per_start 0; rate Checker.quick per_start 0; rate Ledger.post per_start 0 }
}
'''
SN=["Good","Bad"]
SW=objective(SN,"metrics.overall.work",[(4,4),(2,2)],sampled=True)
SD="  compare [Good, Bad] cut C hard { cohort.WrongPerArrival <= 0.6 } objectives { minimize metrics.overall.work }"
sh=hard(SN,"WrongPerArrival","<=","3/5",[(0,"1/2"),("1/2",1)],[S,I],"statistical",points=[0,1])
put("eval-sim",append(SIM,SD),inv(SN,"simulation"),I,
    evaluation(SN,[sh],[SW],sampled=True,m=2),calc=
    "T05: R3,confidence1/2,m=2*1*1*1=2,alpha1/8. x0 upper1-(1/8)^(1/3)=1/2; "
    "x3 lower(1/8)^(1/3)=1/2. Good[0,1/2] S at3/5;Bad[1/2,1] I. "
    "Samples work4 and2, three observations. No simulation Pareto.",ident="T05")
change("eval-sim","tight","<= 0.6","<= 0.4",V,
       evaluation(SN,[hard(SN,"WrongPerArrival","<=","2/5",[(0,"1/2"),("1/2",1)],
                           [I,V],"statistical",points=[0,1])],[SW],sampled=True,m=2),calc=
       "2/5<1/2. Good point0 is not a pass:upper1/2 crosses;Bad lower1/2 violates.")
SR=edit(SIM,"replications 3","replications 1")
RD="  compare [Good, Bad] cut C hard { cohort.WrongPerRegistered <= 0.1 }"
rh=hard(SN,"WrongPerRegistered","<=","1/10",[(0,15),("1/16",16)],[I,I],"statistical",points=[0,1])
put("eval-sim-ratio",append(SR,RD),inv(SN,"simulation"),I,
    evaluation(SN,[rh],sampled=True,m=4),calc=
    "T06: R1,m=2*1*1*2=4,alpha1/16. hits0:[0,15/16];hits1:[1/16,1]. "
    "Good N0,D1 =>[0,15];Bad N1,D1=>[1/16,16]. Neither upper is clipped at1.",ident="T06")
change("eval-sim-ratio","tight","<= 0.1","<= 0.05",V,
       evaluation(SN,[hard(SN,"WrongPerRegistered","<=","1/20",[(0,15),("1/16",16)],
                           [I,V],"statistical",points=[0,1])],sampled=True,m=4),calc=
       "Bad lower1/16>1/20, Good lower0<=1/20 and upper15>1/20.")

QOBJ=objective(N,"cohort.WrongPerArrival",[("1/50","1/50"),("1/10","1/10")],basis="exact")
PD="  compare [Check, Fast] cut C objectives { minimize cohort.WrongPerArrival; minimize metrics.overall.work }"
put("eval-pareto",append(QPLUS,PD),inv(N),S,evaluation(N,os=[QOBJ,WORK]),calc=
    "T07: Check improves Wrong1/50<1/10 but Fast improves work4<6. No domination or unknown pair. "
    "Direction is explicit; objectives never change verdict.",ident="T07")
change("eval-pareto","work-only","minimize cohort.WrongPerArrival; ","",S,
       evaluation(N,os=[WORK],nd=["Fast"],dominated=[("Check",["Fast"])]),calc=
       "Remove only the quality objective:Fast4<Check lower6, hence sure domination.")
PC=edit(PD,"metrics.overall.work","metrics.overall.observed_cost_total")
COST=objective(N,"metrics.overall.observed_cost_total",[(6,8),(4,4)])
put("cmp7-cost",append(QPLUS,PC),inv(N),S,evaluation(N,os=[QOBJ,COST]),calc=
    "T07-cost independent control:unit per-minute rate, zero starts gives cost=work;tradeoff remains.",ident="T07-cost")
change("cmp7-cost","missing-rate","rate Checker.check per_start 0; ","",S,
       evaluation(N,os=[QOBJ,objective(N,"metrics.overall.observed_cost_total",[None,(4,4)],
                                      reasons=["not_configured",None])],
                  unknown=[("Check","Fast","missing_value")]),calc=
       "Remove only Check's start rate. Check cost is null/not_configured,not0. Both remain non-dominated;one missing_value pair.")

M=(ROOT/"fixtures/met-compare/model.relca").read_text()
MN=["AsIs","ToBeP"]
MW=objective(MN,"metrics.overall.work",[(8,8)]*2)
MC=objective(MN,"metrics.overall.observed_cost_total",[(805,805)]*2)
MD="  compare [AsIs, ToBeP] cut C objectives { minimize metrics.overall.work; minimize metrics.overall.observed_cost_total }"
put("eval-shared-source",append(M,MD),inv(MN),S,evaluation(MN,os=[MW,MC]),calc=
    "T08: work3+5=8;cost100*8+2+3=805 on both. Equal points have no strict improvement, "
    "no unknown pair. compare.pareto must name work and observed_cost_total as shared observation.",ident="T08")
change("eval-shared-source","one-objective","; minimize metrics.overall.observed_cost_total","",S,
       evaluation(MN,os=[MW]),calc="Delete only the cost objective:equal8 remains non-dominated;no shared pair exists.")

VAC=edit(Z,"include [Checker, Ledger]","include [Checker, Ledger, Probe]")
VAC=append(VAC,'  actor Probe { capacity 2; capability mark() -> Ack { commits; duration fixed 1min; result nondet { committed } } }\n  contract K { invariant Probe.mark.commit -> registered == true }')
VAC=edit(VAC,"process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }",
         "process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); p = Probe.mark(); end posted }")
VAC=edit(VAC,"configuration Fast { cut C; process Flow }","configuration Fast { cut C; process Quick }")
# Quick must use the same probabilistic check, not lenient;quality is unchanged.
VAC=edit(VAC,"process Quick { v = Checker.quick()", "process Quick { v = Checker.check()")
put("eval-vacuous",append(VAC,ZD),inv(N),Q,evaluation(N,[zh],verify=[S,Q]),calc=
    "T09:Probe.mark occurs only in Check,after registered=true. K S there;Fast never triggers antecedent, "
    "K vacuous. WrongPerRegistered remains1/46 both. Fast is undetermined despite hard S.",ident="T09")
change("eval-vacuous","reached","configuration Fast { cut C; process Quick }",
       "configuration Fast { cut C; process Flow }",S,evaluation(N,[zh]),calc=
       "Only Off/Fast process reference changes. Both now trigger K after registration,verify S and admissible.")

AD="  compare [AsIs, ToBeP] cut C"
put("eval-agency",append(M,AD),inv(MN),S,evaluation(MN),calc=
    "T10:C5 differences pair AsIs->ToBeP. Both have read-only calls a,b with same actors/capabilities, "
    "no evidence roots/effects,retention counts1 for each. Differences derived,rows empty. "
    "No hard means hard_verdict S;no objectives means no_objectives.",ident="T10")
change("eval-agency","swapped","[AsIs, ToBeP]","[ToBeP, AsIs]",S,
       evaluation(list(reversed(MN))),invocation=inv(list(reversed(MN))),calc=
       "Order-control operation reverses declared and requested plans together;from/to must reverse,not sort.")
change("eval-agency","implicit-cut","compare [AsIs, ToBeP] cut C","compare [AsIs, ToBeP]",S,
       evaluation(MN,cut=None),calc="Only optional declared cut is omitted. Existing selected plan Cut C remains compatible;declared.cut is null,not silently filled.")

# T11 and T25 do not depend on C6's extra kinds. C6-specific budget tests are deferred.
MU=(ROOT/"fixtures/qual-base/model.relca").read_text()
MU=edit(MU,"  metric cohort.WrongPerArrival",
        "  property case.Resolved = reached(end.posted || end.returned)\n  metric cohort.WrongPerArrival")
MU=append(MU,"  metric cohort.WrongPerResolved = expected(count(case.Wrong)) / expected(count(case.Resolved))")
put("eval-mut7",MU,dict(command="mutate"),U,calc=
    "T11: WrongPerArrival uses lexical Flow ends[returned,posted];WrongPerResolved uses declared "
    "Ends[posted,returned]. Four independent removals,each changes exactly one denominator end. "
    "WrongPerRegistered State is skipped(state_condition). No execution or ratio value for mutants. "
    "Two existing guard mutants precede four kind7 records. Wrapper remains unsupported/4.",ident="T11",mutation=True)
lines="".join(l for l in MU.splitlines(True) if "metric cohort." in l)
MU_NO="".join(l for l in MU.splitlines(True) if "metric cohort." not in l)
put("eval-mut7.broken-no-cohort",MU_NO,dict(command="mutate"),U,calc=
    "Remove exactly all three metric declarations;properties remain. Old kind7 unsupported/quality_denominator_unavailable. "
    "No skipped_sites,no kind7 assumption. Other nine kinds unchanged.",ident="T11",mutation=True)
pair("eval-mut7","eval-mut7.broken-no-cohort",MU,MU_NO)

# Label control is a fixture pair,not a defect. Numeric/order oracle remains positional.
LABEL=append(C,CD)
for a,b in [("OnTimePerArrival","Z1"),("OnTime","A0"),("One","Zulu"),("Two","Alpha")]:
    LABEL=edit_all(LABEL,a,b)
LABEL=edit_all(LABEL,"actor A { capacity 1;", "actor Desk { grain team; capacity 1;")
LABEL=edit_all(LABEL,"A.run","Desk.run")
LABEL=edit_all(LABEL,"[A, B]","[Desk, B]")
LABEL=edit(LABEL,"rate A per_minute","rate Desk per_minute")
ln=["Zulu","Alpha"]
lh=hard(ln,"Z1",">=","1/2",[("1/2","1/2"),(1,1)],[S,S])
lw=objective(ln,"metrics.overall.work",[(6,6),(8,8)])
put("eval-label",LABEL,inv(ln),S,evaluation(ln,[lh],[lw],nd=["Zulu"],dominated=[("Alpha",["Zulu"])]),calc=
    "T12:Relabel One->Zulu,Two->Alpha,OnTime->A0,ratio->Z1,A->Desk/grain team. "
    "Physical durations/capacities/rates unchanged, so positional1/2,1 and6,8,non_dominated[Zulu]. "
    "Undo the five-name bijection;only old tables.metrics rows may be restored to the "
    "frozen pre-rename key order. Assert that this permutation actually occurs. Other arrays,especially declared "
    "plans,evaluation,admissibility and Pareto,are never sorted. Only the explicit grain,source hashes and four "
    "independently enumerated diagnostic coordinates are projected;all remaining bytes must match.",ident="T12")
change("eval-label","no-policy","  policy fifo tie_by case_id\n","",I,
       evaluation(ln,[dict(ch,cohort="Z1",constraint="cohort.Z1 >= 1/2",values=[dict(v,configuration=ln[j]) for j,v in enumerate(ch["values"])])],[lw]),
       calc="Only FIFO removed;Zulu interval[0,1]/per_case_relaxation I;Alpha S. Preserve declaration order.")

# Strict equality: a single stochastic draw with exact probability1/2 on two distinct processes.
EQ=edit(SIM,"1: pass with correct = true","0.5: pass with correct = true; 0.5: pass with correct = false")
EQ=edit(EQ,"1: pass with correct = false","0.5: pass with correct = true; 0.5: pass with correct = false")
for op,suffix,strict in [("<=","le","<"),(">=","ge",">")]:
    ed=f"  compare [Good, Bad] cut C hard {{ cohort.WrongPerArrival {op} 0.5 }}"
    eh=hard(SN,"WrongPerArrival",op,"1/2",[("1/2","1/2")]*2,[S,S])
    base="cmp7-equality-"+suffix
    put(base,append(EQ,ed),inv(SN),S,evaluation(SN,[eh]),calc=
        "T14:Each independent distribution has Wrong1/2 exactly. Inclusive comparison at1/2 S;strict comparison V.",ident="T14")
    change(base,"strict",f"{op} 0.5",f"{strict} 0.5",V,
           evaluation(SN,[hard(SN,"WrongPerArrival",strict,"1/2",[("1/2","1/2")]*2,[V,V])]),
           calc="Change only comparison token. Equality violates strict < or >.")

ZERO=edit(SR,"1: pass with correct = false","1: pass with correct = true")
for cases in [1,2]:
    zz=ZERO
    if cases==2:
        zz=edit(zz,"arrivals fixed [0min]","arrivals fixed [0min, 10min]")
        zz=edit(zz,"cases 1;","cases 2;")
    m=2*cases
    upper=1-F(1,4*m)
    h=hard(SN,"WrongPerArrival","<=","9/10",[(0,upper)]*2,[S if cases==1 else I]*2,"statistical",points=[0,0])
    dd="  compare [Good, Bad] cut C hard { cohort.WrongPerArrival <= 0.9 }"
    if cases==2:
        base="cmp7-case-multiplicity"
        put(base,append(zz,dd),inv(SN,"simulation"),I,evaluation(SN,[h],sampled=True,m=m),calc=
            "T15:R1,2plans*1row*2cases=4 intervals,alpha1/16;all x0 upper15/16>9/10. "
            "Do not pool cases into trials or omit rejected/unarrived slots.",ident="T15")
        one=hard(SN,"WrongPerArrival","<=","9/10",[(0,"7/8")]*2,[S,S],"statistical",points=[0,0])
        changed=edit(edit(append(zz,dd),"arrivals fixed [0min, 10min]","arrivals fixed [0min]"),"cases 2;","cases 1;")
        put(base+".broken-one-case",changed,inv(SN,"simulation"),S,evaluation(SN,[one],sampled=True,m=2),calc=
            "Single case-count transformation updates arrivals+bound together. m2 alpha1/8,upper7/8<=9/10.",ident="T15")
        pair(base,base+".broken-one-case",append(zz,dd),changed)
    else:
        base="cmp7-row-multiplicity"
        put(base,append(zz,dd),inv(SN,"simulation"),S,evaluation(SN,[h],sampled=True,m=2),calc=
            "T16:One case,R1,m2 alpha1/8 upper7/8<=9/10. A second distinct threshold is not a duplicate.",ident="T16")
        h1=hard(SN,"WrongPerArrival","<=","9/10",[(0,"15/16")]*2,[I,I],"statistical",points=[0,0])
        h2=hard(SN,"WrongPerArrival","<=","19/20",[(0,"15/16")]*2,[S,S],"statistical",points=[0,0])
        change(base,"another-row","<= 0.9 }","<= 0.9; cohort.WrongPerArrival <= 0.95 }",I,
               evaluation(SN,[h1,h2],sampled=True,m=4),calc=
               "2plans*2rows*1case=4 even with shared numerator. upper15/16>9/10 but <=19/20.")

# T18: observe a zero denominator without asserting its true probability is0.
ZDEN=edit(SR,"registered = true","registered = false")
rd="  compare [Good, Bad] cut C hard { cohort.WrongPerRegistered <= 1 }"
zero_h=hard(SN,"WrongPerRegistered","<=",1,[None]*2,[I,I],
            reasons=["denominator_may_be_zero"]*2,points=[None,None])
for c in zero_h["values"]:
    c["source_reason"]="zero_denominator"
put("cmp7-sim-zero",append(ZDEN,rd),inv(SN,"simulation"),I,
    evaluation(SN,[zero_h],sampled=True,m=4),calc=
    "T18:R1,Registered hits0 =>lower0,not proof p=0. Interval null/denominator_may_be_zero. "
    "Wrong hits0 too,point0/0=null. Source zero_denominator is preserved in source_reason;"
    "the CP evaluation reason is denominator_may_be_zero.",ident="T18")
change("cmp7-sim-zero","registered","registered = false","registered = true",I,
       evaluation(SN,[hard(SN,"WrongPerRegistered","<=",1,[(0,15),("1/16",16)],[I,I],
                           "statistical",points=[0,1])],sampled=True,m=4),calc=
       "Only finish assignment changes;N is0/1,D1,intervals[0,15] and[1/16,16],not null.")

# T19 min interval overlap, including endpoints;positive times only.
OV='''model Overlap version "0.2" {
  type T = enum { pass }
  actor A { capacity 1; capability run() -> T { duration {3min:0.5,6min:0.5}; result nondet { pass } } }
  actor B { capacity 1; capability run() -> T { duration fixed 4min; result nondet { pass } } }
  process P { a = A.run(); end done }
  process R { b = B.run(); end done }
  cut C { include [A, B] }
  configuration One { cut C; process P }
  configuration Two { cut C; process R }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  metrics { window 10min; currency "JPY"; rate A per_minute 1; rate B per_minute 1; rate A.run per_start 0; rate B.run per_start 0 }
  compare [One, Two] cut C objectives { minimize metrics.overall.work }
}
'''
put("cmp7-overlap",OV,inv(CN),S,evaluation(CN,os=[objective(CN,"metrics.overall.work",[(3,6),(4,4)])],
                                         unknown=[("One","Two","overlapping_intervals")]),calc=
    "T19:minimize[3,6] versus[4,4]. 6<=4 false;4<=3 false. Both non-dominated,overlapping_intervals. "
    "Lower3<4 alone cannot establish domination.",ident="T19")
change("cmp7-overlap","narrow","duration {3min:0.5,6min:0.5}","duration fixed 3min",S,
       evaluation(CN,os=[objective(CN,"metrics.overall.work",[(3,3),(4,4)])],nd=["One"],dominated=[("Two",["One"])]),calc=
       "Replace only duration upper support:range becomes[3,3],3<4 sure domination.")

MAX=edit(SIM,"1: pass with correct = true","0.75: pass with correct = true; 0.25: pass with correct = false")
MAX=edit(MAX,"1: pass with correct = false","0.25: pass with correct = true; 0.75: pass with correct = false")
mx="  compare [Good, Bad] cut C objectives { maximize cohort.WrongPerArrival }"
mo=objective(SN,"cohort.WrongPerArrival",[("1/4","1/4"),("3/4","3/4")],direction="maximize",basis="exact")
put("cmp7-maximize",append(MAX,mx),inv(SN),S,evaluation(SN,os=[mo],nd=["Bad"],dominated=[("Good",["Bad"])]),calc=
    "T20:explicit maximize1/4 versus3/4 =>Bad dominates. Do not infer minimization from the name Wrong.",ident="T20")
change("cmp7-maximize","minimize","maximize cohort.","minimize cohort.",S,
       evaluation(SN,os=[dict(mo,objective="minimize cohort.WrongPerArrival",direction="minimize")],
                  nd=["Good"],dominated=[("Bad",["Good"])]),calc="Only direction changes;now1/4<3/4 gives Good.")

NONE=edit(SIM,"1: pass with correct = true","0.9: pass with correct = true; 0.1: pass with correct = false")
NONE=edit(NONE,"kernel bad { * -> { 1: pass with correct = false } }",
          "kernel bad { * -> { 0.8: pass with correct = true; 0.2: pass with correct = false } }")
nh=hard(SN,"WrongPerArrival","<=","1/20",[("1/10","1/10"),("1/5","1/5")],[V,V])
nw=objective(SN,"metrics.overall.work",[(4,4),(2,2)])
nd="  compare [Good, Bad] cut C hard { cohort.WrongPerArrival <= 0.05 } objectives { minimize metrics.overall.work }"
put("cmp7-no-admissible",append(NONE,nd),inv(SN),V,evaluation(SN,[nh],[nw]),calc=
    "T21:Both verify S;Wrong1/10 and1/5 both exceed1/20. Both excluded;among empty;no_admissible_plans.",ident="T21")
change("cmp7-no-admissible","one-admissible","<= 0.05","<= 0.1",V,
       evaluation(SN,[hard(SN,"WrongPerArrival","<=","1/10",[("1/10","1/10"),("1/5","1/5")],[S,V])],[nw]),calc=
       "Threshold1/10 makes Good admissible by equality,Bad excluded. One admissible still computed/non_dominated[Good].")

ABORT=edit(SIM,"type V = enum { pass }","type V = enum { pass, fail }")
ABORT=edit(ABORT,"outcomes kernel good; writes correct","result nondet { pass, fail }")
put("cmp7-abort",append(ABORT,SD),inv(SN,"simulation"),U,
    evaluation(SN,[hard(SN,"WrongPerArrival","<=","3/5",[None,("1/2",1)],[U,I],"statistical",
                        reasons=["aborted",None],points=[None,1])],[dict(SW,values=[
                            dict(configuration="Good",sample=dict(mean=None,min=None,max=None,samples=0,reason="aborted"),reason="aborted"),SW["values"][1]])],
               verify=[U,S],sampled=True,m=2),code=None,calc=
    "T22:Good encounters actual nondeterministic result. Nested U-SEM-0;no partial samples. "
    "Good hard U/aborted,point/interval null;Bad remains I. Root U outranks I.",ident="T22")
change("cmp7-abort","deterministic","result nondet { pass, fail }","result nondet { pass }",I,
       evaluation(SN,[hard(SN,"WrongPerArrival","<=","3/5",[("1/2",1),("1/2",1)],[I,I],
                           "statistical",points=[1,1])],[SW],sampled=True,m=2),calc=
       "Single outcome eliminates runtime nondeterminism. correct stays initialfalse,so both Wrong1;both CP[1/2,1].")

# T13 static table. Each defect has its own directory and exact witness.
static = [
 ("report",DECL,DECL+" report [work]",U,"U-SYN-0"),
 ("weight",DECL,DECL+" weight 1",U,"U-SYN-0"),
 ("score",DECL,DECL+" score 1",U,"U-SYN-0"),
 ("hard-metric","cohort.WrongPerArrival <= 0.05","metrics.overall.work <= 0.05",U,"U-SYN-0"),
 ("hard-quantile","cohort.WrongPerArrival <= 0.05","quantile.Median <= 0.05",U,"U-SYN-0"),
 ("hard-case","cohort.WrongPerArrival <= 0.05","case.Wrong <= 0.05",U,"U-SYN-0"),
 ("no-direction","minimize metrics.overall.work","metrics.overall.work",U,"U-SYN-0"),
 ("utilization","minimize metrics.overall.work","minimize metrics.overall.utilization",U,"U-SYN-0"),
 ("waiting","minimize metrics.overall.work","minimize metrics.overall.queue_wait_observed",U,"U-SYN-0"),
 ("agency","minimize metrics.overall.work","minimize agency.F",U,"U-SYN-0"),
 ("case-objective","minimize metrics.overall.work","minimize case.Wrong",U,"U-SYN-0"),
 ("quantile","minimize metrics.overall.work","minimize quantile.Median",X,"E-NAME-0"),
 ("duplicate-declaration",DECL,DECL+"\n"+DECL,U,"U-SEM-0"),
 ("duplicate-hard","<= 0.05 }","<= 0.05; cohort.WrongPerArrival <= 0.050 }",U,"U-SEM-0"),
 ("duplicate-objective","minimize metrics.overall.work","minimize metrics.overall.work; minimize metrics.overall.work",U,"U-SEM-0"),
 ("unknown-plan","[Check, Fast]","[Check, Missing]",X,"E-NAME-0"),
 ("unknown-cohort","cohort.WrongPerArrival <=","cohort.Missing <=",X,"E-NAME-0"),
 ("unknown-objective","minimize metrics.overall.work","minimize cohort.Missing",X,"E-NAME-0"),
 ("unknown-cut","compare [Check, Fast] cut C","compare [Check, Fast] cut Missing",X,"E-NAME-0"),
 ("out-of-range","<= 0.05","<= 1.5",X,"E-PROB-0"),
 ("fraction","<= 0.05","<= 1/2",U,"U-SYN-0"),
 ("exponent","<= 0.05","<= 5e-2",U,"U-SYN-0"),
 ("one-plan","[Check, Fast]","[Check]",X,"E-SYN-0"),
 ("same-plan","[Check, Fast]","[Check, Check]",X,"E-SYN-0"),
 ("empty-plans","[Check, Fast]","[]",X,"E-SYN-0"),
 ("empty-hard","hard { cohort.WrongPerArrival <= 0.05 }","hard {}",X,"E-SYN-0"),
 ("empty-objectives","objectives { minimize metrics.overall.work }","objectives {}",X,"E-SYN-0"),
 ("missing-bracket","[Check, Fast]","[Check, Fast",X,"E-SYN-0"),
 ("process","process Quick {","process Quick { compare [Check, Fast];",U,"U-SYN-0"),
 ("configuration","configuration Fast {","configuration Fast { compare [Check, Fast];",U,"U-SYN-0"),
]
for suffix,a,b,v,code in static:
    changed_name=change("eval-hard",suffix,a,b,v,code=code,calc=
           f"T13:{suffix}. Single local edit;{code};no evaluation/agency partial result. "
           "No runtime numeric oracle is changed.")
    message={"report":"compare report lists are unsupported","weight":"weighted scores are unsupported",
             "score":"weighted scores are unsupported","hard-metric":"hard constraints apply to cohort ratios only",
             "hard-quantile":"hard constraints apply to cohort ratios only","hard-case":"hard constraints apply to cohort ratios only",
             "no-direction":"objectives need minimize or maximize","utilization":"unsupported compare objective",
             "waiting":"unsupported compare objective","agency":"unsupported compare objective",
             "case-objective":"unsupported compare objective","quantile":"unknown quantile metric: Median",
             "duplicate-declaration":"duplicate compare declaration","duplicate-hard":"duplicate hard constraint",
             "duplicate-objective":"duplicate objective","unknown-plan":"unknown configuration",
             "unknown-cohort":"unknown cohort metric","unknown-objective":"unknown cohort metric",
             "unknown-cut":"unknown cut","out-of-range":"probability outside 0..1",
             "fraction":"exponent or fraction probability","exponent":"exponent or fraction probability",
             "one-plan":"plans require at least two distinct ASCII identifiers",
             "same-plan":"plans require at least two distinct ASCII identifiers",
             "empty-plans":"plans require at least two distinct ASCII identifiers",
             "process":"unsupported syntax: compare","configuration":"unsupported syntax: compare"}.get(suffix)
    if message:INVENTORY[changed_name]["message_contains"]=message
# Invocation-only faults are still explicit pairs.
for suffix,iv in [("plan-order",inv(["Fast","Check"])),("plan-set",inv(["Check","Other"]))]:
    source=src
    if suffix=="plan-set":
        # Other is known,not an unknown-name fault. Add to BOTH controls below instead.
        continue
    name="eval-hard.broken-"+suffix
    put(name,source,iv,X,code="E-CONF-0",calc="T13:only requested plan order reversed;declaration order binding must not be ignored.",ident="T13")
    INVENTORY[name]["message_contains"]="plans differ from the compare declaration"
    pair("eval-hard",name,source,source)

# Scenario inventory: no horizon required. Selected scenarios have identical arrival metadata.
SC=QPLUS
SC=edit(SC,"  arrivals fixed [0min, 10min]\n","")
SC=append(SC,"  scenario Day { arrivals fixed [0min, 10min] }\n  scenario Spare { arrivals fixed [0min, 10min] }\n  scenario Outside { arrivals fixed [0min, 10min] }\n  cut C2 { include [Checker, Ledger] }")
SCDECL=edit(DECL,"cut C hard", "under [Day, Spare] cut C hard")
sce=evaluation(N,[H],[WORK],under=["Day","Spare"],scenario="Day")
put("cmp7-under",append(SC,SCDECL),inv(N,scenario="Day"),V,sce,calc=
    "T13/U15:under[Day,Spare] is an allowed selector list,not two runs. Both scenarios resolved from original AST. "
    "Selected Day keeps T01 numbers. Outside is known but not under.",ident="T13-under")
for suffix,a,b,v,code in [
    ("duplicate","[Day, Spare]","[Day, Day]",U,"U-SEM-0"),
    ("unknown","[Day, Spare]","[Day, Missing]",X,"E-NAME-0"),
    ("empty","[Day, Spare]","[]",X,"E-SYN-0"),
    ("cut","under [Day, Spare] cut C hard","under [Day, Spare] cut C2 hard",X,"E-CMP-0"),
    ("scenario-body","scenario Spare {","scenario Spare { compare [Check, Fast];",U,"U-SYN-0"),
]:
    changed_name=change("cmp7-under",suffix,a,b,v,code=code,calc=f"T13:{suffix} local declaration edit;{code},evaluation absent.")
    msg={"duplicate":"duplicate compare scenario","unknown":"unknown scenario",
         "cut":"compare cut differs from plan cut","scenario-body":"unsupported syntax: compare"}.get(suffix)
    if msg:INVENTORY[changed_name]["message_contains"]=msg
for suffix,scenario,code,v in [("outside","Outside","E-CONF-0",X),("omitted",None,"E-CONF-0",X),
                             ("several","Day,Spare","U-SEM-0",U)]:
    name="cmp7-under.broken-"+suffix
    so=append(SC,SCDECL)
    put(name,so,inv(N,scenario=scenario),v,code=code,calc=f"Only scenario selector changes:{suffix};{code};no evaluation.",ident="T13-under")
    INVENTORY[name]["message_contains"]={"outside":"scenario is not under the compare declaration",
        "omitted":"a scenario selector is required","several":"compare under several scenarios is unsupported"}[suffix]
    pair("cmp7-under",name,so,so)
# Single allowed scenario still requires explicit selector.
solo=edit(append(SC,SCDECL),"[Day, Spare]","[Day]")
put("cmp7-under-one",solo,inv(N,scenario="Day"),V,evaluation(N,[H],[WORK],under=["Day"],scenario="Day"),
    calc="T13:one allowed scenario still uses explicit selector. Same T01 values.",ident="T13-under")
put("cmp7-under-one.broken-omitted",solo,inv(N),X,code="E-CONF-0",calc=
    "Remove only selector;one under name must not silently select it.",ident="T13-under")
pair("cmp7-under-one","cmp7-under-one.broken-omitted",solo,solo)
INVENTORY["cmp7-under-one.broken-omitted"]["message_contains"]="a scenario selector is required"

# All five permitted metrics fields,opposite directions are NOT duplicates.
ALL="  compare [AsIs, ToBeP] cut C objectives { minimize metrics.overall.work; maximize metrics.overall.named_people_work; minimize metrics.overall.peak_tasks; minimize metrics.overall.observed_cost_total; minimize metrics.overall.observed_cost_per_arrival }"
all_os=[MW,objective(MN,"metrics.overall.named_people_work",[(0,0)]*2,direction="maximize"),
        objective(MN,"metrics.overall.peak_tasks",[(1,1),(2,2)]),MC,
        objective(MN,"metrics.overall.observed_cost_per_arrival",[(805,805)]*2)]
put("cmp7-overall",append(M,ALL),inv(MN),S,evaluation(MN,os=all_os,nd=["AsIs"],dominated=[("ToBeP",["AsIs"])]),calc=
    "U9:work8,named_people0(no named executors),peak1 serial/2 parallel,cost805,perarrival805. "
    "All are path_range,not expected values. Only peak improves for AsIs,so surely dominates.",ident="U9")
change("cmp7-overall","opposite-direction","minimize metrics.overall.peak_tasks","maximize metrics.overall.peak_tasks",S,
       evaluation(MN,os=[all_os[0],all_os[1],dict(all_os[2],objective="maximize metrics.overall.peak_tasks",direction="maximize"),*all_os[3:]],
                  nd=["ToBeP"],dominated=[("AsIs",["ToBeP"])]),calc="Only explicit peak direction reverses domination.")

def kind7(cohort, terminals, skipped=()):
    ms=[]
    for removed in terminals:
        after=[e for e in terminals if e!=removed]
        ms.append(dict(id=f"old_promise_denominator_removed:cohort.{cohort}:remove:{removed}",
                       operator="remove_denominator_end",result="detected",reason=None,report=None,
                       site=dict(cohort=cohort,removed_end=removed,process=None),other_changes=[],
                       assigned_changes=[dict(check=c,key="cohort."+cohort,field="denominator.terminals",
                                              before=terminals,after=after,direction="changed")
                                         for c in ["quality_denominator","comparison_mismatch"]]))
    return dict(status="generated" if ms else "not_applicable",
                reason=None if ms else ("state_condition" if skipped and skipped[0]["reason"]=="state_condition" else "no_matching_site"),
                mutants=ms,skipped_sites=list(skipped))

STATE=(ROOT/"fixtures/qual-base/model.relca").read_text()
STATE=edit(STATE,"  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals\n","")
skip=dict(cohort="WrongPerRegistered",status="not_applicable",reason="state_condition")
put("cmp7-mut-state",STATE,dict(command="mutate"),U,calc=
    "T25:only ratio denominator Registered is a State condition. No kind7 candidate;skipped one,state_condition;"
    "no mutation.kind7 assumption or numerator-subset marker. Other mutation kinds unchanged.",
    ident="T25",mutation=True,kind7_oracle=kind7("WrongPerRegistered",[],[skip]))
state2=edit(STATE,"property case.Registered = reached(registered == true)",
            "property case.Registered = reached(end.posted || end.returned)")
put("cmp7-mut-state.broken-ends",state2,dict(command="mutate"),U,calc=
    "T25:replace only property target by explicit Ends[posted,returned]. Two independent removals detected;"
    "name Registered carries no special meaning;no mutant ratio computed.",ident="T25",mutation=True,
    kind7_oracle=kind7("WrongPerRegistered",["posted","returned"]))
pair("cmp7-mut-state","cmp7-mut-state.broken-ends",STATE,state2)
single=edit(STATE,"property case.Registered = reached(registered == true)","property case.Registered = reached(end.posted)")
skip1=dict(cohort="WrongPerRegistered",status="not_applicable",reason="no_matching_site")
put("cmp7-mut-singleton",single,dict(command="mutate"),U,calc=
    "T25 singleton control:one denominator end is not a site;NA/no_matching_site,not state_condition.",ident="T25",mutation=True,
    kind7_oracle=kind7("WrongPerRegistered",[],[skip1]))
put("cmp7-mut-singleton.broken-union",state2,dict(command="mutate"),U,calc=
    "Add only returned to denominator property. Now two end-removal sites,each detected structurally.",ident="T25",mutation=True,
    kind7_oracle=kind7("WrongPerRegistered",["posted","returned"]))
pair("cmp7-mut-singleton","cmp7-mut-singleton.broken-union",single,state2)

simno="  compare [Good, Bad] cut C objectives { minimize metrics.overall.work }"
put("cmp7-sim-no-hard",append(SIM,simno),inv(SN,"simulation"),S,
    evaluation(SN,os=[SW],sampled=True,m=0),calc=
    "Empty hard family:hard_verdict S,m0,alpha null. Both verify S. No simulation Pareto even with objectives.",ident="m0")
change("cmp7-sim-no-hard","no-objectives"," objectives { minimize metrics.overall.work }","",S,
       evaluation(SN,sampled=True,m=0),calc="Only objective block removed;simulation_estimates has priority over no_objectives.")

OVERMAX=edit(OV,"duration {3min:0.5,6min:0.5}","duration {6min:0.5,9min:0.5}")
OVERMAX=edit(OVERMAX,"duration fixed 4min","duration fixed 8min")
OVERMAX=edit(OVERMAX,"minimize metrics.overall.work","maximize metrics.overall.work")
put("cmp7-overlap-max",OVERMAX,inv(CN),S,
    evaluation(CN,os=[objective(CN,"metrics.overall.work",[(6,9),(8,8)],direction="maximize")],
               unknown=[("One","Two","overlapping_intervals")]),calc=
    "T19 max control:One[6,9],Two[8,8]. Maximize requires lower>=other upper,not comparing upper endpoints. "
    "Both non-dominated,one overlapping_intervals pair.",ident="T19-max")
change("cmp7-overlap-max","narrow","duration {6min:0.5,9min:0.5}","duration fixed 9min",S,
       evaluation(CN,os=[objective(CN,"metrics.overall.work",[(9,9),(8,8)],direction="maximize")],
                  nd=["One"],dominated=[("Two",["One"])]),calc="Only duration lower support narrows to9;9>8 sure max domination.")

RESP=(ROOT/"fixtures/agy2-responsibility-diff/model.relca").read_text()
RESP=append(RESP,"  compare [From, To] cut Scope")
put("cmp7-responsibility",RESP,inv(["From","To"]),S,
    evaluation(["From","To"],cut="Scope"),calc=
    "T10/F32:responsible a1 for Approver.approve. Both consume entry indirectly from Doc;"
    "From never directly inspects Doc,so observation missing,required[Doc],direct[],indirect[Doc],missing[Doc]. "
    "To calls Approver.inspect reading Doc,so observation present,direct[Doc],missing[]. "
    "No within:time_frame remains unchecked/not_comparable on both. These responsibility values must reach C5,not None.",
    ident="T10-F32")
change("cmp7-responsibility","no-inspect","i = Approver.inspect(); ","",S,
       evaluation(["From","To"],cut="Scope"),calc=
       "Delete only To's inspection. Changed responsibility observation row disappears;time_frame not_comparable remains.")

THREE=append(QPLUS,"  configuration Middle { cut C; process Flow }")
TN=["Fast","Check","Middle"]
th=hard(TN,"WrongPerArrival","<=","1/20",[("1/10","1/10"),("1/50","1/50"),("1/50","1/50")],[V,S,S])
tw=objective(TN,"metrics.overall.work",[(4,4),(6,8),(6,8)])
td="  compare [Fast, Check, Middle] cut C hard { cohort.WrongPerArrival <= 0.05 } objectives { minimize metrics.overall.work }"
put("cmp7-three-plans",append(THREE,td),inv(TN),V,
    evaluation(TN,[th],[tw],unknown=[("Check","Middle","overlapping_intervals")]),calc=
    "Three plans in deliberately non-sorted order. Fast Wrong1/10 excluded;Check and Middle Wrong1/50 admissible. "
    "Both admissible work[6,8] overlap,so neither surely dominates and one unknown pair remains. "
    "Agency must include all three i<j pairs,including excluded Fast.",ident="T10-all-pairs")
change("cmp7-three-plans","another-excluded","configuration Middle { cut C; process Flow }",
       "configuration Middle { cut C; process Quick }",V,
       evaluation(TN,[hard(TN,"WrongPerArrival","<=","1/20",[("1/10","1/10"),("1/50","1/50"),("1/10","1/10")],[V,S,V])],
                  [objective(TN,"metrics.overall.work",[(4,4),(6,8),(4,4)])]),calc=
       "Only Middle's process reference changes. It is excluded;among/non_dominated[Check],unknown empty. All three Agency pairs remain.")

shared="  compare [Check, Fast] cut C objectives { minimize cohort.WrongPerArrival; minimize cohort.WrongPerRegistered }"
qo=objective(N,"cohort.WrongPerRegistered",[("1/46","1/46"),("1/10","1/10")],basis="exact")
put("cmp7-shared-cohort",append(QPLUS,shared),inv(N),S,
    evaluation(N,os=[QOBJ,qo],nd=["Check"],dominated=[("Fast",["Check"])]),calc=
    "Two ratios share numerator property Wrong,not just all-arrival count. "
    "Check1/50<1/10 and1/46<1/10,so surely dominates. compare.pareto must name both cohort refs as shared observation.",ident="U10-cohort")
change("cmp7-shared-cohort","one-objective","; minimize cohort.WrongPerRegistered","",S,
       evaluation(N,os=[QOBJ],nd=["Check"],dominated=[("Fast",["Check"])]),calc=
       "Remove only Registered ratio objective. Same dominance;the shared-source pair is no longer declared.")

metrics_line='  metrics { window 10min; currency "JPY"; rate Office per_minute 100; rate Office.short per_start 2; rate Office.long per_start 3 }\n'
NM=edit(M,metrics_line,"")
nmdecl="  compare [AsIs, ToBeP] cut C objectives { minimize metrics.overall.work }"
put("cmp7-no-metrics",append(NM,nmdecl),inv(MN),S,
    evaluation(MN,os=[objective(MN,"metrics.overall.work",[None,None],reasons=["not_configured"]*2)],
               unknown=[("AsIs","ToBeP","missing_value")]),calc=
    "U9:the target has a closed type but metrics were not declared. Values null/not_configured,not static error or0. "
    "Missing-value pair retained;both admissible and non-dominated;root S.",ident="U9-not-configured")
change("cmp7-no-metrics","configured","  bounds { cases 1; live_tasks 2; clock 10min }\n",
       '  bounds { cases 1; live_tasks 2; clock 10min }\n'+metrics_line,S,
       evaluation(MN,os=[MW]),calc="Add only the metrics declaration. Both work8;equal points are known non-dominating,not an unknown pair.")

NP=append(M,"  people [p, q]")
NP=edit(NP,"    capacity 2","    executors [p, q]")
npdecl="  compare [AsIs, ToBeP] cut C objectives { minimize metrics.overall.named_people_work }"
npo=objective(MN,"metrics.overall.named_people_work",[(8,8)]*2)
put("cmp7-named-work",append(NP,npdecl),inv(MN),S,evaluation(MN,os=[npo]),calc=
    "Two named executors supply Office. short3+long5 gives named_people_work8 in serial and parallel;"
    "do not substitute total work by name or always return0. No strict improvement.",ident="U9-named")
change("cmp7-named-work","anonymous","    executors [p, q]","    capacity 2",S,
       evaluation(MN,os=[objective(MN,"metrics.overall.named_people_work",[(0,0)]*2)]),calc=
       "Only executor pool becomes anonymous capacity2. Named occupancy0,while total work remains8;objective values change,not verdict.")

VX=append(QPLUS,"  contract K { invariant Ledger.post.commit -> registered == false }")
put("cmp7-verify-excluded",append(VX,DECL),inv(N),V,
    evaluation(N,[H],[WORK],verify=[V,V]),calc=
    "U3 structural verify constraint:post commits with registered=true,so invariant registered==false is violated "
    "in both plans. Check's hard1/50<=1/20 still S,but verify V excludes it. Fast excluded too. "
    "No admissible plans,not a work-based rescue. Verify reason precedes hard reason.",ident="U3-verify")
change("cmp7-verify-excluded","verify-satisfied","Ledger.post.commit -> registered == false",
       "Ledger.post.commit -> registered == true",V,evaluation(N,[H],[WORK]),calc=
       "Change only invariant's boolean. Both verify S;Check admissible,Fast excluded solely by hard;root remains V.")

# Public F14 aggregate with a spec-derived fair initial draw,not runtime-copied
# observations. ADR0016 key contract and exact 64-bit half split imply2/4 hits.
draws=[int.from_bytes(hashlib.sha256(
    f"relca-sim-v1|seed=2|rep={r}|case=1|site=init.case.correct|n=1".encode()).digest()[:8],"big")
       for r in range(1,5)]
assert draws==[13870960371742070810,16087836130209217323,926088661298817727,4275795123798739053]
assert sum(k < 2**63 for k in draws)==2
ND=edit(SIM,"latent case { correct: bool = false }","latent case { correct: bool ~ { true:0.5, false:0.5 } }")
ND=edit(ND,"outcomes kernel good; writes correct","result nondet { pass }")
ND=edit(ND,"outcomes kernel bad; writes correct","result nondet { pass }")
ND=edit(ND,"replications 3 seed 1 confidence 0.5","replications 4 seed 2 confidence 0.75")
nddecl="  compare [Good, Bad] cut C hard { cohort.WrongPerArrival <= 0.9 }"
ndh=hard(SN,"WrongPerArrival","<=","9/10",[("28863/262144","233281/262144")]*2,[S,S],"statistical",points=["1/2"]*2)
put("cmp7-nondyadic",append(ND,nddecl),inv(SN,"simulation"),S,
    evaluation(SN,[ndh],sampled=True,m=2,confidence="3/4"),calc=
    "F14 public aggregate. R4,seed2,fair initial correct distribution,no later writes. "
    "ADR0016 SHA256 init.case.correct keys yield first64-bit integers " + ','.join(str(k) for k in draws) +
    ". Exactly two are below2^63 and two above,so Wrong hits2,unknown0,trials4 in BOTH plans;point1/2. "
    "m2,confidence3/4 =>alpha1/16. G(2;p)=6p^2-8p^3+3p^4;at D=2^20 the root lies between "
    "k=115452 and115453. Outward CP[28863/262144,233281/262144]. Upper<9/10,both hard S. "
    "The independent BigInt polynomial test certifies the lattice without calling production interval().",ident="F14-public")
change("cmp7-nondyadic","tight","<= 0.9","<= 0.88",I,
       evaluation(SN,[hard(SN,"WrongPerArrival","<=","22/25",[("28863/262144","233281/262144")]*2,[I,I],
                           "statistical",points=["1/2"]*2)],sampled=True,m=2,confidence="3/4"),calc=
       "Only bound changes to22/25. Lower<22/25<upper,so both hard I;interval endpoints remain the non-dyadic hand values.")

names=sorted(INVENTORY)
(SUPPORT/"evaluation_names.json").write_text(json.dumps(names,indent=2)+"\n")
(SUPPORT/"evaluation_names.rs").write_text("[\n"+"".join('    '+json.dumps(n)+',\n' for n in names)+"]\n")
(SUPPORT/"evaluation_inventory.json").write_text(json.dumps(INVENTORY,indent=2)+"\n")
(SUPPORT/"evaluation_pairs.json").write_text(json.dumps(PAIRS,indent=2)+"\n")
print(len(names),"hand-authored fixtures;",len(PAIRS),"edit/control witnesses")
