"""ADR0035 independent hand oracles. Never executes or imports Relca.

Materializes only aset-* fixtures and caseset_{pairs,names}.json.
Pair edits are checked before writing. All durations and work are minutes.
"""
import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
PAIRS = []
NAMES = []
CONTROLS = []
ATOMIC = '''atomic {
        if key in keys { result rejected_duplicate }
        else { keys += key; registered = true; result committed }
      }'''
FINISH = 'finish { keys += key; registered = true }; result nondet { committed }'
SNAP_ATOMIC = '''atomic {
        if gate==true { keys += key; registered = true; result committed }
        else { result rejected_duplicate }
      }'''
POST = '''capability post(key: CaseId) -> Ack {
      commits
      writes [keys, registered]
      duration fixed 1min
      ATOMIC
      cancel forbidden_after_start
    }'''.replace('ATOMIC', ATOMIC)
CALLS = 'a = Ledger.post(case.id); b = Ledger.post(case.id); if b == rejected_duplicate { end dup } else { end twice }'
CONTRACT = 'contract C { at_most_once Ledger.post.commit per case.id }'


def change(s, old, new):
    assert old in s, (old, s)
    t = s.replace(old, new)
    assert s != t, old
    return t


def clean(s):
    return '\n'.join(line.rstrip() for line in s.split('\n'))


def legacy_control(source):
    """Erase only new type/effect primitives, retain all surrounding old grammar.

    This is a syntax preflight, never an equivalence or numeric oracle.
    Explicit erasures are stored with original bytes for independent audit.
    """
    s = source
    # Effects have nested braces; walk balanced blocks rather than regex-closing.
    for word in ['atomic', 'start', 'finish']:
        while (m := re.search(r'\b'+word+r'\s*\{', s)) is not None:
            depth, i = 1, m.end()
            while depth:
                depth += (s[i]=='{')-(s[i]=='}')
                i += 1
            replacement = 'result nondet{committed}' if word=='atomic' else ''
            s = s[:m.start()]+replacement+s[i:]
    s = re.sub(r'\bwrites\s+(?:\[[^\]]*\]|[\w.]+)\s*;?', '', s)
    s = re.sub(r'set\s*<\s*CaseId\s*>', 'bool=false', s)
    s = re.sub(r'\b\w+\s*:\s*CaseId\s*,?', '', s)
    s = re.sub(r'case\s*\.\s*id\s+in\s+[\w.]+', 'registered==false', s)
    s = re.sub(r'\b\w+\s+in\s+[\w.]+', 'registered==false', s)
    # Do not rewrite the at_most_once per case.id path.
    s = re.sub(r'\(case\.id\s*,?', '(', s)
    s = re.sub(r',\s*case\.id(?=[,)])', '', s)
    s = s.replace('PersonId','T')
    s = re.sub(r'==\s*c[12]\b', '==ok', s)
    s = re.sub(r',\s*\)', ')', s)
    if 'registered==false' in s and 'registered:' not in s and 'registered :' not in s:
        s = s.replace('observed case {', 'observed case {registered:bool=false;')
    return s


def model(actors=None, process=CALLS, declarations='', extra=CONTRACT,
          verify='verify reachable end.dup', cases=1, arrivals=None,
          names='Ledger', clock=20, observed='registered: bool = false'):
    actors = actors if actors is not None else 'actor Ledger { capacity 1; state keys: set<CaseId>; '+POST+' }'
    # join-any's fallback references an already declared end, it does not declare it.
    process = process.replace('no_result;b=', 'dup;b=').replace('no_result;if case', 'absent;if case').replace('no_result;if input', 'unset;if input')
    extra = re.sub(r'simulation\{replications (\d+);seed (\d+);confidence ([0-9.]+)\}', r'simulation replications \1 seed \2 confidence \3', extra)
    extra = re.sub(r'configuration (\w+)\{process (\w+);cut (\w+)\}', r'configuration \1{cut \3;process \2}', extra)
    return ('model CaseSet version "0.2" {\n'
            '  type Ack = enum { committed, rejected_duplicate }\n'
            '  type T = enum { ok, no }\n'
            '  people [c1,c2]\n'
            + (f'  observed case {{ {observed} }}\n' if observed else '')
            + f'  {declarations}\n  {actors}\n  process Flow {{ {process} }}\n'
            + f'  cut K {{ include [{names}] }}\n'
            + (f'  arrivals fixed [{arrivals}]\n  policy fifo tie_by case_id\n' if arrivals is not None else '')
            + f'  bounds {{ cases {cases}; live_tasks 3; clock {clock}min }}\n'
            + f'  {extra}\n  {verify}\n}}\n')


def ran(x):
    a, b = x if isinstance(x, tuple) else (x, x)
    return dict(min=a, max=b)


def oracle(end='dup', elapsed: int | tuple[int,int]=2, work: int | tuple[int,int]=2, verdict='satisfied', result=None, multi=None, truncated=False) -> dict[str,Any]:
    terms = [] if multi is not None or end is None else [dict(end=end, elapsed=ran(elapsed), work=ran(work), result_at=None if result is None else ran(result))]
    out:dict[str,Any] = dict(verdict=verdict, terminals=terms, bounds=dict(truncated=truncated), absent_diagnostics=['E-SYN-0','E-NAME-0','E-TYPE-0','E-CUT-1','E-POL-LATENT','U-SYN-0','U-SEM-0'])
    if multi is not None:
        out['case_outcomes'] = [dict(case_id=i+1, arrival=0, terminals=[] if t is None else [dict(kind='end',end=end,completed_at=ran(t),elapsed=ran(t),work=ran(work),result_at=None)]) for i,t in enumerate(multi)]
    return out


def static(code):
    return dict(verdict='unsupported' if code.startswith('U-') else 'invalid',
                diagnostics=[dict(code=code,severity='error')],properties=[],terminals=[],bounds=dict(truncated=None))


def pair(id_, name, s, old, new, good, bad, calc, fault='fault', invocation=None, bad_invocation=None):
    s,old,new=clean(s),clean(old),clean(new)
    t = change(s, old, new)
    name = 'aset-'+name
    broken = name+'.broken-'+fault
    PAIRS.append(dict(id=id_,good=name,bad=broken,from_=old,to=new,calc=calc))
    CONTROLS.append(dict(name=name,original=s,legacy_control=legacy_control(s),configuration='Left' if id_=='P18' else None))
    for n, source, want, inv, side in [(name,s,good,invocation,'control'),(broken,t,bad,bad_invocation or invocation,'single edit')]:
        assert '//' not in source and '#' not in source
        if (not inv and want.get('bounds',{}).get('truncated') is False
                and len(want.get('terminals',[]))==1
                and len(set(re.findall(r'\bend\s+(\w+)',source)))>1):
            want=dict(want,diagnostics=[dict(code='W-UNREACH-1',severity='warning'),dict(code='W-UNREACH-2',severity='warning')])
        d = ROOT/'fixtures'/n
        d.mkdir(exist_ok=True)
        (d/'model.relca').write_text(source)
        (d/'expected.json').write_text(json.dumps(want,indent=2)+'\n')
        (d/'calc.md').write_text(f'# {id_}: {n}\n\nHand oracle fixed before implementation ({side}). Unit: min.\n\n{calc}\n\nSingle mechanism edit: {old!r} -> {new!r}.\n\nStatic failures have no analyzed time or invented zero counts. Computational cuts are not business rejection. Atomic refusal is a normal finish, not a requires refusal. A RED checkpoint is not semantic verification.\n')
        if id_=='P18' and side=='single edit':
            (d/'wire_faults.json').write_text(json.dumps([
                {'notes':['unknown']},
                {'notes':['mechanism_unmodeled','mechanism_unmodeled']},
                {'notes':[]}, {'notes':None}],indent=2)+'\n')
        if id_=='P19' and side=='single edit':
            (d/'replay_faults.json').write_text(json.dumps([[2,1],[1,1],[1,2,2],[0,2],[1,3],[1],'[1,2]'],indent=2)+'\n')
        if inv:
            (d/'invocation.json').write_text(json.dumps(inv)+'\n')
        NAMES.append(n)


def main():
    b = model()
    pair('P01','dup',b,ATOMIC,FINISH,oracle(),oracle('twice',verdict='violated'),
         'First finish t1 inserts {1}, registered=true, commit1. Second finish t2 sees1 and rejects: finish2/commit1, keys={1}, E=W=2, dup only. Without check both finish committed: commit2, twice only, contract and reachable dup violated. Notes absent/present respectively. Both sides have W-UNREACH-1 and W-UNREACH-2.', 'no-check')
    two = model(process='a = Ledger.post(case.id); end done', verify='verify reachable end.done',cases=2,arrivals='0min,0min')
    pair('P02','cases',two,'capacity 1','capacity 2',oracle('done',work=1,multi=[1,2]),static('U-SEM-0'),
         'FIFO capacity1: case1 [0,1), case2 [1,2). Each W1 and one commit; final shared keys={1,2}. Capacity2 is a static shared-writer refusal; no exploration.', 'capacity')
    g = change(two,'state keys: set<CaseId>','state seen: set<CaseId>; state keys: set<CaseId>')
    pair('P03','cut-rg',g,'if key in keys','if key in seen',oracle('done',work=1,multi=[1,2]),oracle('done',work=1,multi=[1,2]),
         'Both one-call cases commit once, E1/E2, total W2. Guarding the wrong empty set leaves seen={} and keys={1,2}, but cannot certify a mechanism: notes required only on faulty side.', 'wrong-set')
    r = model(actors='resource erp_slots capacity 1; actor Poster { capacity 2; capability post()->Ack {commits;uses erp_slots:1;duration fixed 1min;result nondet{committed}}}',process='a=Poster.post();end done',extra='contract C{at_most_once Poster.post.commit per case.id}',verify='verify reachable end.done',names='Poster',cases=2,arrivals='0min,0min')
    pair('P03R','cut-resource',r,'erp_slots capacity 1','erp_slots capacity 2',oracle('done',work=1,multi=[1,2]),oracle('done',work=1,multi=[1,1]),
         'R resource1 serializes two one-minute posts (1/2); resource2 overlaps (1/1). Work1 each. All contracts satisfied but notes mechanism_unmodeled on both: anonymous capacity is not duplicate prevention.', 'two-slots')
    policy = model(process='a=Ledger.post(case.id); b=Ledger.post(case.id); if case.id in Ledger.keys {end posted}else{end lost}',declarations='type Risk=enum{high,low}',verify='verify reachable end.posted')
    policy = change(policy,'capacity 1; state keys','capacity 1; latent risk:Risk=high; state keys')
    pair('P04','policy',policy,'if key in keys','if Ledger.risk == high',oracle('posted'),static('E-POL-LATENT'),
         'Membership remains true after first finish and through duplicate rejection: posted only at2, W2. Replacing only atomic guard with latent high must fail E-POL-LATENT, including unused capabilities.', 'latent')
    person = model(actors='actor Clerk {executors [c1,c2]; capability enter()->Ack {writes input_by;finish{input_by=executor.id};duration fixed 3min;result nondet{committed}}}',process='a=Clerk.enter();if input_by==c1 {end by_c1}else{end by_c2}',extra='',verify='verify reachable end.by_c1;verify reachable end.by_c2',names='Clerk',observed='input_by:optional<PersonId>')
    pgood = oracle('by_c1',3,3)
    pgood['terminals'].append(dict(end='by_c2',elapsed=ran(3),work=ran(3),result_at=None))
    pair('P05','executor',person,'executors [c1,c2]','capacity 1',pgood,static('U-SEM-0'),
         'Either declared person can start at0, occupies [0,3), writes their own ID at3. Both by_c1 and by_c2 exist, E=W3. Optional starts none. Anonymous executor cannot invent a PersonId.', 'anonymous')
    cut = change(b,CONTRACT,CONTRACT+'; contract I{invariant Ledger.post.finish -> case.id in Ledger.keys}')
    pair('P06','cut',cut,'include [Ledger]','exclude Ledger reason "outside" loses "keys"',oracle(),static('E-CUT-1'),
         'After both normal finishes keys contains1, including rejected duplicate. Excluding called Ledger is E-CUT-1; the separate read-only boundary isolates Contains support.', 'exclude')
    readonly = model(actors='actor Ledger {state keys:set<CaseId>};actor Probe{capacity 1;capability go()->Ack{duration fixed 1min;result nondet{committed}}}',process='a=Probe.go();end done',extra='contract I{invariant Probe.go.finish -> (case.id in Ledger.keys || registered==false)}',verify='verify reachable end.done',names='Ledger,Probe')
    pair('P06b','cut-read',readonly,'include [Ledger,Probe]','include [Probe];exclude Ledger reason "outside" loses "keys"',oracle('done',1,1),static('E-CUT-1'),
         'No call or evidence read touches Ledger. At1 keys empty so false OR true is true. Exclusion must still reject the membership leaf; no short-circuit Cut exemption.', 'exclude-reader')
    pair('P07','label',b,'Ledger','Book',oracle(),oracle(),
         'Actor alpha-renaming cannot alter E2/W2, dup, commit1 or mechanism recognition. Core separately varies grain document/ledger/person and normalizes source-derived spans, SHA and explicit identifier paths.', 'renamed')
    fast = 'actor Fast {capacity 1;capability go()->Ack{duration fixed 1min;result nondet{committed};cancel forbidden_after_start}}'
    cancelprocess = 'fork f {a=Ledger.post(case.id);q=Fast.go()};join any f where result==committed tie by [q,a] remaining cancel_then_drain if none -> no_result;b=Ledger.post(case.id);if b==committed{end posted}else{end dup}'
    slow = change(POST,'duration fixed 1min','duration fixed 3min')
    slow = change(slow,'cancel forbidden_after_start','cancel cooperative ack_within 1min')
    cancel = model(actors='actor Ledger {capacity 1;state keys:set<CaseId>;'+slow+'};'+fast,process=cancelprocess,names='Ledger,Fast',verify='verify reachable end.posted')
    pair('P08','cancel',cancel,'cancel cooperative ack_within 1min','cancel forbidden_after_start',oracle('posted',5,6,result=1),oracle('dup',6,7,'violated',result=1),
         'Fast [0,1); first post [0,2) cancelled at ack2 has no atomic effect. Next post [2,5) commits, E5/W(1+2+3)=6/result1. Forbidden first finishes3 and inserts, second [3,6) rejects: E6/W7/result1, commit1 still.', 'forbidden')
    agency = model(actors='actor Source{source record}; actor Seed{capacity 1;capability make()->T{reads[Source];duration fixed 1min;result nondet{ok}}};actor Writer{capacity 1;capability check(e:T)->T{writes decision;duration fixed 1min;result nondet{ok,no}}};actor Ledger{capacity 1;state keys:set<CaseId>;capability prime(key:CaseId)->Ack{writes keys;finish{keys+=key};duration fixed 1min;result nondet{committed}}};actor Poster{capacity 1;capability post(key:CaseId)->Ack{commits;requires key in Ledger.keys else rejected_duplicate;duration fixed 1min;result nondet{committed}}}',process='e=Seed.make();d=Writer.check(e);p=Ledger.prime(case.id);a=Poster.post(case.id);end done',observed='decision:T=no',names='Source,Seed,Writer,Ledger,Poster',extra='',verify='verify safety no_resource_overcommit')
    pair('P09','agency',agency,'requires key in Ledger.keys','requires decision==ok',oracle('done',4,4),oracle('done',(3,4),(3,4)),
         'Seed1 + Writer1 + prime1 + post1 =4, always membership true at runtime. Contains is Unknown statically, so no refusal proof. Control decision==ok is rewritten to unique Writer result, producing pass4/fail3 and a Writer all_paths judgement; do not claim equal timing.', 'scalar-requires')
    pair('P10','clock',two,'clock 20min','clock 1min',oracle('done',work=1,multi=[1,2]),oracle('done',work=1,multi=[1,None],verdict='inconclusive',truncated=True),
         'At clock1 first finish is processed; case1 done1/W1, case2 cannot finish at2. Contract and reachability case1 S/case2 I, aggregate I. No fabricated rejected terminal; no W-BUDGET-0 for clock alone.', 'short')
    limit = model(actors='actor Ledger{state keys:set<CaseId>}',process='end done',extra='',verify='verify reachable end.done',cases=64,arrivals=','.join(f'{i}min' for i in range(64)),clock=64)
    old = 'cases 64'
    # One coherent bound/arrival-size mechanism edit, not an arrivals-count fault.
    good = dict(verdict='satisfied',terminals=[],bounds=dict(truncated=False))
    limit65 = change(limit,'cases 64','cases 65')
    limit65 = change(limit65,'62min,63min]','62min,63min,64min]')
    pair('P12','limit',limit,limit,limit65,good,static('U-SEM-0'),
         '64 end-only cases each E=W0, keys empty. Set cardinality2^64=18446744073709551616, no power-set allocation. Coherently adding case65+arrival65 exceeds representation; U-SEM-0. Direct bit63 display oracle is [64].','sixty-five')
    toggle = 'actor Toggle{capacity 1;capability go()->Ack{writes gate;finish{gate=true};duration fixed 1min;result nondet{committed};cancel forbidden_after_start}}'
    snap_post = change(slow,ATOMIC,SNAP_ATOMIC)
    snapshot = model(actors='actor Ledger{capacity 1;state keys:set<CaseId>;'+snap_post+'};'+toggle,process='fork f{a=Ledger.post(case.id);q=Toggle.go()};join all f;if a==committed{end committed_end}else{end rejected}',names='Ledger,Toggle',observed='registered:bool=false;gate:bool=false',verify='verify reachable end.committed_end')
    pair('P13','finish-snapshot',snapshot,'finish{gate=true}','finish{gate=false}',oracle('committed_end',3,4),oracle('rejected',3,4,'violated'),
         'At start0 gate=false selects the reject branch. Toggle ends1 and writes true; post ends3, evaluates true and commits in the then branch. W3+1=4, keys{1}. Fault leaves false: reject only, keys{}, commit0 and at_most_once vacuous. This distinguishes finish evaluation from start snapshot with capacity1 and tests commit_when=true.', 'false-toggle')
    startpost = change(slow,ATOMIC,'start{keys+=key};result nondet{committed}')
    startpost = change(startpost,'writes [keys, registered]','')
    startprocess = 'fork f{a=Ledger.post(case.id);q=Fast.go()};join any f where result==committed tie by [q,a] remaining cancel_then_drain if none -> no_result;if case.id in Ledger.keys{end present}else{end absent}'
    start = model(actors='actor Ledger{capacity 1;state keys:set<CaseId>;'+startpost+'};'+fast,process=startprocess,names='Ledger,Fast',extra='',verify='verify reachable end.present')
    pair('P14','start-insert',start,'start{keys+=key}','',oracle('present',2,3,result=1),oracle('absent',2,3,'violated',result=1),
         'Start0 inserts1, Fast completes1, ack2 cancels post without undoing start. E2/W(2+1)=3/result1. Removing start leaves empty and absent. No at_most_once is declared, so vacuity cannot mask reachability.', 'no-start')
    ps = model(actors='actor Slow{executors[c1];capability go()->Ack{start{input_by=executor.id};duration fixed 3min;result nondet{committed};cancel cooperative ack_within 1min}};'+fast,process='fork f{a=Slow.go();q=Fast.go()};join any f where result==committed tie by [q,a] remaining cancel_then_drain if none -> no_result;if input_by==c1{end recorded}else{end unset}',names='Slow,Fast',extra='',observed='input_by:optional<PersonId>',verify='verify reachable end.recorded')
    pair('P15','person-start',ps,'start{input_by=executor.id}','writes input_by;finish{input_by=executor.id}',oracle('recorded',2,3,result=1),oracle('unset',2,3,'violated',result=1),
         'Slow starts with c1 at0 and records c1; cancellation ack2 does not roll back. Finish-only write never occurs and remains none. E2/W3/result1 in both. ID source and effect time are distinct obligations.', 'finish-only')
    rollback = change(b,'registered: bool = false','registered: bool = false; n:0..1=0')
    rollback = change(rollback,'writes [keys, registered]','writes [keys, registered,n]')
    rollback = change(rollback,'registered = true; result committed','registered = true; n+=1; result committed')
    bad_range = dict(verdict='violated',terminals=[],bounds=dict(truncated=False,range_violation_stops=True))
    pair('P16','atomic-rollback',rollback,'n:0..1=0','n:0..1=1',oracle(),bad_range,
         'Control: first bundle inserts key, sets registered and n1; second rejects without evaluating n+1. Fault attempts n2 at t1: no insertion, no result/commit, registered false, n remains1. state_in_range V, reachable/contract I, not computational truncation.', 'initial-one')
    simperson = change(person,'executors [c1,c2]','executors [c1]')
    simperson = change(simperson,'verify reachable end.by_c1;verify reachable end.by_c2','simulation replications 4 seed 17 confidence 0.95;verify reachable end.by_c1')
    simgood = dict(verdict='satisfied',simulation=dict(attempted_replications=4,completed_replications=4))
    simbad = dict(verdict='unsupported',diagnostics=[dict(code='U-SEM-0',severity='error')],simulation=dict(attempted_replications=1,completed_replications=0))
    pair('P17','simulation',simperson,'executors [c1]','executors [c1,c2]',simgood,simbad,
         'One executor gives four by_c1 replications, E=W3, no draws. Two executor starts differ in Task.executor already; first encounter aborts attempted1/completed0. No uniform random executor. A separate no-branch control preserves the same refusal.', 'two-people',invocation=dict(command='simulate'))
    compare = model(actors='actor Poster{capacity 1;capability post()->Ack{commits;duration fixed 1min;result nondet{committed}}}',process='a=Poster.post();end done',extra='contract C{at_most_once Poster.post.commit per case.id};simulation{replications 4;seed 17;confidence 0.95};configuration Left{process Flow;cut K};configuration Right{process Other;cut K};process Other{b=Poster.post();end done}',verify='verify reachable end.done',names='Poster')
    pair('P18','compare-notes',compare,'replications 4','replications 5',dict(verdict='satisfied'),dict(verdict='satisfied'),
         'Both old-syntax configurations execute one post in1, work1 and at_most_once S with mechanism_unmodeled. Exhaustive table excludes notes. Core/CLI separately compare simulation4 (I sample contract) and inject unknown, duplicate or wrong-property notes into projection. Replication count edit does not change exhaustive semantics.', 'replications',invocation=dict(command='compare',plans=['Left','Right'],mode='exhaustive'))
    replay = model(actors='actor Ledger{capacity 1;state keys:set<CaseId>;'+POST+'};actor Probe{capacity 1;capability go()->Ack{duration fixed 1min;result nondet{committed}}}',process='a=Ledger.post(case.id);wait 3min;q=Probe.go();end done',extra=CONTRACT+';contract I{invariant Probe.go.finish -> registered==false}',verify='verify reachable end.done',names='Ledger,Probe',cases=2,arrivals='0min,0min')
    pair('P19','replay-display',replay,'wait 3min','wait 2min',oracle('done',work=2,multi=[5,6],verdict='violated'),oracle('done',work=2,multi=[4,5],verdict='violated'),
         'Ledger finishes1/2 and shares keys[1,2]; waits3 then Probe1 finishes5/6. Earliest invariant counterexample case1/t5, one shortest trace, verified replay, value array[1,2]. Fault wait2 yields4/5. Replay tampering [2,1],[1,1],[0,2],[1,3] is separately API-negative, never normalized.', 'shorter-wait')
    requires = model(actors='actor Ledger{capacity 1;state keys:set<CaseId>;capability prime(key:CaseId)->Ack{writes keys;finish{keys+=key};duration fixed 1min;result nondet{committed}};capability work(key:CaseId)->Ack{requires key in keys else rejected_duplicate;duration fixed 1min;result nondet{committed}}}',process='a=Ledger.prime(case.id);b=Ledger.work(case.id);end done',extra='',verify='verify reachable end.done')
    pair('P20','requires-in',requires,'writes keys;finish{keys+=key};','',oracle('done',2,2),oracle('done',1,1),
         'Prime [0,1) inserts1; work starts1 and ends2. Removing prime insertion (and its writes contract) makes work refuse before start at1, refusal1/work0, total E=W1. Both reach done. Atomic refusal in P01 instead consumes a full task minute.', 'no-prime')
    branch = model(process='if case.id in Ledger.keys{end already}else{a=Ledger.post(case.id);end done}',verify='verify reachable end.done',cases=2,arrivals='0min,0min')
    pair('P21','branch-sharing',branch,'if case.id in Ledger.keys','if registered==true',oracle('done',work=1,multi=[1,2]),oracle('done',work=1,multi=[1,2]),
         'Each case begins false/empty own key and posts once; keys{1,2}, case completion1/2, W1 each. Replacing guard with per-case false registered control leaves runtime equal. Private transition seam, not this monotone fixture, must distinguish due shared finish vs Branch ordering.', 'scalar-guard')
    conditional = change(snapshot,'if a==committed{end committed_end}else{end rejected}','if a==committed{z=Next.go();end committed_end}else{end rejected}')
    conditional = change(conditional,'actor Toggle', 'actor Next{capacity 1;capability go()->Ack{duration fixed 2min;result nondet{committed}}};actor Toggle')
    conditional = change(conditional,'include [Ledger,Toggle]','include [Ledger,Toggle,Next]')
    conditional = change(conditional,'verify reachable end.committed_end','verify safety no_resource_overcommit')
    nondet = 'result nondet{committed,rejected_duplicate}'
    pair('P22','conditional-agency',conditional,'writes [keys, registered]\n      duration fixed 3min\n      '+SNAP_ATOMIC, 'duration fixed 3min;'+nondet,oracle('committed_end',5,6),dict(verdict='satisfied',terminals=[dict(end='committed_end',elapsed=ran(5),work=ran(6),result_at=None),dict(end='rejected',elapsed=ran(3),work=ran(4),result_at=None)],bounds=dict(truncated=False)),
         'Atomic sees gate true at3, calls Next [3,5): E5/W(3+1+2)=6. Static F guards Unknown; L(post)=1..1, L(Next) not_derived/state_guards. Independent nondet results admit rejected3/W4 and committed5/W6; F derived and L(Next)=0..1. Do not degrade O/P with Unknown.', 'nondet')
    sampling = model(extra='simulation{replications 4;seed 17;confidence 0.95}')
    pair('P23','atomic-sampling',sampling,ATOMIC,FINISH,dict(verdict='satisfied',simulation=dict(attempted_replications=4,completed_replications=4)),dict(verdict='inconclusive',simulation=dict(attempted_replications=4,completed_replications=4)),
         'Atomic makes deterministic dup, four replications, each E=W2, draws empty. Finish-only always twice; no dup witness means inconclusive sample reachability, not violated. Corresponding exhaustive probability1 vs0 is separately asserted.', 'no-check',invocation=dict(command='simulate'))
    probability = model(extra='',verify='verify probability reached end.dup >= 1')
    pair('P23P','atomic-probability',probability,ATOMIC,FINISH,oracle(),oracle('twice',verdict='violated'),
         'No random source: deterministic dup probability[1,1] vs[0,0] when check removed. E=W2 on both. Atomic results must not become nondeterministic outcomes.', 'no-check')
    static_pairs(b,person)
    (HERE/'caseset_names.json').write_text(json.dumps(sorted(NAMES),indent=2)+'\n')
    (HERE/'caseset_wire_names.rs').write_text('&[\n'+''.join('    '+json.dumps(n)+',\n' for n in sorted(NAMES))+']\n')
    (HERE/'caseset_pair_tests.rs').write_text('acceptance! {\n'+''.join('    pair_'+str(i+1).zfill(3)+' => '+json.dumps(p['good'])+',\n' for i,p in enumerate(PAIRS))+'}\n')
    (HERE/'caseset_legacy_controls.json').write_text(json.dumps(CONTROLS,indent=2)+'\n')
    (HERE/'caseset_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    print(f'{len(PAIRS)} independent pairs; {len(NAMES)} directories')


def static_pairs(b, person):
    def st(name,old,new,code='U-SYN-0',s=b,good=None):
        control=good or oracle()
        if control.get('case_outcomes'):
            arithmetic='One one-minute post per case, FIFO intervals [0,1) and [1,2): completion1/2 and work1 each.'
        elif control.get('terminals'):
            elapsed=control['terminals'][0]['elapsed']['min']
            arithmetic={0:'No task is invoked: end at0, elapsed0 and work0.',
                        1:'One one-minute task [0,1): elapsed=work=1.',
                        2:'Two one-minute tasks run serially (or serialize at capacity1): elapsed=1+1=2 and work=1+1=2.',
                        3:'Seed.make1 followed by post1 and post1: elapsed=work=1+1+1=3.'}[elapsed]
        else:
            arithmetic='Clerk chooses c1 or c2, one task [0,3): both endpoints have elapsed=work=3.'
        pair('P11','static-'+name,s,old,new,good or oracle(),static(code),
             'Positive control: '+arithmetic+f' Single misuse {name} must be statically refused with {code}, exit'+('4' if code.startswith('U-') else '3')+'. No numeric exploration after static failure.',name)
    for name,ty in [('person','PersonId'),('enum','T'),('bool','bool'),('integer','0..3'),('optional','optional<CaseId>'),('nested','set<CaseId>')]:
        st('element-'+name,'set<CaseId>','set<'+ty+'>')
    empty = model(actors='actor Ledger{state keys:set<CaseId>}',process='end done',extra='',verify='verify reachable end.done')
    zero = oracle('done',0,0)
    for name,old,new,code in [
        ('case-set','actor Ledger{state keys:set<CaseId>}','observed case{keys:set<CaseId>};actor Ledger{}','U-SYN-0'),
        ('latent-case','actor Ledger{state keys:set<CaseId>}','latent case{keys:set<CaseId>};actor Ledger{}','U-SYN-0'),
        ('latent-actor','state keys:set<CaseId>','latent keys:set<CaseId>','U-SYN-0'),
        ('per-case','state keys:set<CaseId>','instance per_case;state keys:set<CaseId>','U-SEM-0'),
        ('caseid-state','state keys:set<CaseId>','state k:CaseId','U-SYN-0'),
        ('caseid-optional','state keys:set<CaseId>','state k:optional<CaseId>','U-SYN-0'),
        ('person-latent','state keys:set<CaseId>','latent who:optional<PersonId>','U-SYN-0')]:
        st(name,old,new,code,empty,zero)
    unused = model(actors='actor Ledger{capacity 1;capability unused(key:CaseId)->Ack{duration fixed 1min;result nondet{committed}}}',process='end done',extra='',verify='verify reachable end.done')
    st('person-parameter','key:CaseId','key:PersonId','U-SYN-0',unused,zero)
    for name,old,new,code in [
        ('remove','keys += key','keys -= key','U-SYN-0'),
        ('assign','keys += key','keys = key','U-SYN-0'),
        ('assign-empty','keys += key','keys = {}','U-SYN-0'),
        ('default-empty','state keys: set<CaseId>','state keys: set<CaseId> = {}','U-SYN-0'),
        ('default-none','state keys: set<CaseId>','state keys: set<CaseId> = none','U-SYN-0'),
        ('distribution','state keys: set<CaseId>','state keys: set<CaseId> ~ { }','U-SYN-0'),
        ('builtin-case','type T = enum','type CaseId=enum{k1};type T = enum','U-SEM-0'),
        ('builtin-person','type T = enum','type PersonId=enum{c1};type T = enum','U-SEM-0'),
        ('nondet','commits','commits;result nondet{committed}','U-SEM-0'),
        ('finish','commits','commits;finish{}','U-SEM-0'),
        ('kernel','commits','commits;outcomes kernel K','U-SEM-0'),
        ('both-writes','if key in keys { result rejected_duplicate }','if key in keys { registered=false;result rejected_duplicate }','U-SEM-0'),
        ('no-writes','keys += key; registered = true; ','','U-SEM-0'),
        ('same-result','result rejected_duplicate','result committed','U-SEM-0'),
        ('duplicate-atomic',ATOMIC,ATOMIC+';'+ATOMIC,'U-SEM-0'),
        ('nested-atomic','result rejected_duplicate','atomic{if key in keys{result rejected_duplicate}else{keys+=key;result committed}};result rejected_duplicate','U-SYN-0'),
        ('nested-if','result rejected_duplicate','if key in keys{result rejected_duplicate}else{result committed}','U-SYN-0'),
        ('else-if','else { keys','else if key in keys { keys','U-SYN-0'),
        ('branch-nondet','result rejected_duplicate','result nondet{rejected_duplicate}','U-SYN-0'),
        ('after-result','result rejected_duplicate }','result rejected_duplicate;registered=false }','U-SYN-0'),
        ('missing-else','else { keys','{ keys','E-SYN-0'),
        ('missing-result-value','result rejected_duplicate','result','E-SYN-0'),
        ('missing-brace','result committed }','result committed','E-SYN-0'),
        ('writes-missing','writes [keys, registered]','writes [registered]','E-NAME-0'),
        ('writes-extra','writes [keys, registered]','writes [keys, registered,spare]','U-SEM-0'),
        ('duplicate-target','keys += key','keys += key;Ledger.keys+=key','U-SEM-0'),
        ('result-type','result committed','result ok','E-TYPE-0'),
        ('enum-argument','key: CaseId','key: T','E-TYPE-0'),
        ('undefined-insert','keys += key','keys += missing','E-NAME-0'),
        ('enum-insert','keys += key','keys += other','E-TYPE-0'),
        ('direct-case-insert','keys += key','keys += case.id','U-SYN-0'),
        ('case-equality','if b == rejected_duplicate','if case.id == b','U-SYN-0'),
        ('executor-condition','if b == rejected_duplicate','if executor.id == c1','U-SYN-0'),
        ('binding-membership','if b == rejected_duplicate','if b in Ledger.keys','U-SYN-0'),
        ('captured-parameter','if b == rejected_duplicate','if key in Ledger.keys','U-SYN-0'),
        ('missing-actor','if b == rejected_duplicate','if case.id in Missing.keys','E-NAME-0'),
        ('missing-set','if b == rejected_duplicate','if case.id in Ledger.missing','E-NAME-0'),
        ('nonset','if b == rejected_duplicate','if case.id in Ledger.nonset','E-TYPE-0'),
        ('negation','if b == rejected_duplicate','if !registered == false','U-SYN-0'),
        ('not-in','if b == rejected_duplicate','if case.id not in Ledger.keys','E-SYN-0'),
        ('foreign-write','keys += key','Other.keys += key','U-SYN-0'),
        ('idempotency','commits','commits;idempotency key','U-SYN-0'),
        ('retry','commits','commits;retry 1','U-SYN-0'),
        ('probability-membership','verify reachable end.dup','verify probability reached case.id in Ledger.keys >= 1','U-SYN-0'),
    ]:
        s = b
        if name=='kernel':
            s=change(s,'type T = enum { ok, no }','type T = enum { ok, no };kernel K{* -> {1:committed}}')
        if name=='writes-extra':
            s=change(s,'registered: bool = false','registered: bool = false;spare:bool=false')
        if name=='nonset':
            s=change(s,'capacity 1; state keys','capacity 1;state nonset:bool=false; state keys')
        if name=='foreign-write':
            s=change(s,'actor Ledger','actor Other{state keys:set<CaseId>};actor Ledger')
            s=change(s,'include [Ledger]','include [Ledger,Other]')
        if name=='enum-argument':
            # Isolate call type mismatch from membership misuse in the body.
            s=change(s,ATOMIC,'result nondet{committed}')
            s=change(s,'writes [keys, registered]','')
            s=change(s,'b == rejected_duplicate','b == committed')
            s=change(s,CONTRACT,'')
        if name=='enum-insert':
            s=change(s,'key: CaseId','key: CaseId,other:T')
            s=change(s,'a = Ledger.post(case.id)','a = Seed.make();p=Ledger.post(case.id,a)')
            s=change(s,'b = Ledger.post(case.id)','b = Ledger.post(case.id,a)')
            s=change(s,'actor Ledger','actor Seed{capacity 1;capability make()->T{duration fixed 1min;result nondet{ok}}};actor Ledger')
            s=change(s,'include [Ledger]','include [Ledger,Seed]')
        st(name,old,new,code,s,oracle('dup',3,3) if name=='enum-insert' else (oracle('dup') if name!='enum-argument' else oracle()))
    # Binding-to-CaseId is isolated by an unused, body-independent CaseId Cap.
    arg=model(actors='actor Ledger{capacity 1;capability make()->T{duration fixed 1min;result nondet{ok}};capability take(k:CaseId)->Ack{duration fixed 1min;result nondet{committed}}}',process='a=Ledger.make();b=Ledger.take(case.id);end done',extra='',verify='verify reachable end.done')
    st('binding-to-case','Ledger.take(case.id)','Ledger.take(a)','E-TYPE-0',arg,oracle('done',2,2))
    st('anonymous','executors [c1,c2]','capacity 1','U-SEM-0',person,dict(verdict='satisfied'))
    # Destination bool must be typed independently; retain the PersonId variable.
    bp=change(person,'input_by:optional<PersonId>','input_by:optional<PersonId>;flag:bool=false')
    bp=change(bp,'writes input_by','writes [input_by,flag]')
    bp=change(bp,'finish{input_by=executor.id}','finish{input_by=executor.id;flag=true}')
    st('executor-bool','flag=true','flag=executor.id','E-TYPE-0',bp,dict(verdict='satisfied'))
    # Kernel controls operate on a separate explicit observed PersonId column.
    kp=model(actors='actor Ledger{capacity 1;capability post()->Ack{duration fixed 1min;outcomes kernel K}}',process='a=Ledger.post();end done',extra='',verify='verify reachable end.done',observed='who:optional<PersonId>;flag:bool=false',declarations='kernel K{* -> {1:committed}}')
    for name,old,new in [('kernel-given','kernel K{* -> {1:committed}}','kernel K given [who]{none -> {1:committed};c1 -> {1:committed};c2 -> {1:committed}}'),('kernel-with','1:committed','1:committed with who=c1'),('kernel-reads','duration fixed 1min','kernel_reads latent[who];duration fixed 1min')]:
        s=kp
        if name=='kernel-with':
            s=change(s,'duration fixed 1min','writes who;finish{who=c1};duration fixed 1min')
        st(name,old,new,'U-SEM-0',s,oracle('done',1,1))
    resource = change(model(process='a=Ledger.post(case.id);end done',cases=2,arrivals='0min,0min',verify='verify reachable end.done'),'actor Ledger','resource R capacity 1;actor Ledger')
    resource = change(resource,'commits','commits;uses R:1')
    st('capacity-with-resource','capacity 1; state keys','capacity 2; state keys','U-SEM-0',resource,oracle('done',work=1,multi=[1,2]))
    fork=model(actors='actor Ledger{capacity 1;state keys:set<CaseId>;state other:set<CaseId>;capability a(k:CaseId)->Ack{writes keys;atomic{if k in keys{result rejected_duplicate}else{keys+=k;result committed}};duration fixed 1min;cancel forbidden_after_start};capability b(k:CaseId)->Ack{writes other;atomic{if k in other{result rejected_duplicate}else{other+=k;result committed}};duration fixed 1min;cancel forbidden_after_start}}',process='fork f{a=Ledger.a(case.id);b=Ledger.b(case.id)};join all f;end done',extra='',verify='verify reachable end.done')
    st('fork-target','writes other;atomic{if k in other{result rejected_duplicate}else{other+=k;result committed}}','writes keys;atomic{if k in other{result rejected_duplicate}else{keys+=k;result committed}}','U-SEM-0',fork,oracle('done',2,2))


if __name__ == '__main__':
    main()
