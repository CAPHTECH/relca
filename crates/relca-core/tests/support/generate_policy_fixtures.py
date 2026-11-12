"""Materialize independent hand oracles, never run Relca or import its output.

Every pair records one exact source edit. Units are minutes. This is a fixture
authoring tool; runtime acceptance reads committed artifacts, not this program.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
FIFO = 'policy fifo tie_by case_id'
BOTH = FIFO+' then call_order assign first_idle'
PAIRS = []


def edit(s, a, b):
    assert a in s and a != b, (a, b)
    t = s.replace(a, b)
    assert t != s
    return t


def model(decls, body, actors, policy=BOTH, cases=1, arrivals=None, clock=20,
          extra='', verify='verify safety no_resource_overcommit\nverify reachable end.done'):
    return ('model M version "0.2" {\n  type T = enum { ok, pass, bad, late }\n'+decls+
            '\n  process P { '+body+' }\n  cut C { include ['+','.join(actors)+'] }\n'+
            f'  bounds {{ cases {cases}; live_tasks 8; clock {clock}min }}\n'+
            (f'  arrivals fixed [{arrivals}]\n' if arrivals else '')+
            '  '+policy+'\n'+('  '+extra+'\n' if extra else '')+'  '+verify+'\n}\n')


def actor(name, cap, minutes, executor='capacity 1', more='', result='ok', use=''):
    return f'  actor {name} {{ {executor}; {more} capability {cap}() -> T {{ {use} duration fixed {minutes}min; result nondet {{ {result} }}; cancel cooperative ack_within 1min }} }}\n'


def rng(x):
    a, b = x if isinstance(x, tuple) else (x, x)
    return {'min':a, 'max':b}


def terminal(completed, work, result=None, end='done', arrival=0):
    r = rng(completed)
    return {'kind':'end', 'end':end, 'completed_at':r,
            'elapsed':{'min':r['min']-arrival, 'max':r['max']-arrival},
            'work':rng(work), 'result_at':None if result is None else rng(result)}


def check(times, work, verdict='satisfied', result=None, arrivals=None, diagnostics=None):
    arrivals = arrivals or [0]*len(times)
    return {'verdict':verdict, 'bounds':{'truncated':False},
            'diagnostics':diagnostics or [], 'terminals':[],
            'case_outcomes':[{'case_id':i+1, 'arrival':a,
                              'terminals':[terminal(t, work, result, arrival=a)]}
                             for i,(t,a) in enumerate(zip(times, arrivals))]}


def warning(person):
    return {'code':'W-UNREACH-3', 'severity':'warning',
            'message':f'process P: executor {person} is never assigned'}


def sim(ok=True, cases=2, arrivals=None):
    e = {'verdict':'satisfied' if ok else 'unsupported', 'mode':'simulation',
         'simulation':{'status':'completed' if ok else 'unsupported',
                       'attempted_replications':4 if ok else 1,
                       'completed_replications':4 if ok else 0,
                       'truncated_replications':0 if ok else None}, 'terminals':[]}
    if ok:
        e['diagnostics'] = []
        e['case_outcomes'] = [{'case_id':i+1,'arrival':a,
                              'terminals':[{'kind':'end','end':'done','count':4}],
                              'unfinished_count':0,'deadlocked_count':0}
                             for i,a in enumerate(arrivals or [0,1][:cases])]
    else:
        e['diagnostics'] = [{'code':'U-SEM-0','severity':'error'}]
    return e


CUT_UNDECLARED = {'code':'W-CUT-0', 'severity':'warning', 'span':None}
PARSE_CUT_NOTE = ('Observed once with the earlier binary: a parse failure ends '
                  'report::check_with before the Cut is projected, and report::finish adds W-CUT-0 (span null) '
                  'after the syntax error. This is existing behavior.')


def static(code):
    diagnostics = [{'code':code}]
    if code in ('E-SYN-0', 'U-SYN-0'):
        diagnostics.append(CUT_UNDECLARED)
    return {'verdict':'invalid' if code.startswith('E-') else 'unsupported',
            'diagnostics':diagnostics, 'bounds':{'truncated':None},
            'properties':[], 'terminals':[]}


def pair(name, source, before, after, good, bad, calc, invocation=None, bad_static=False):
    name = 'pol-'+name
    broken = name+'.broken-change'
    faulty = edit(source, before, after)
    PAIRS.append({'good':name,'bad':broken,'from':before,'to':after,
                  'static_bad':bad_static, 'invocation':invocation})
    for n,s,e,side in [(name,source,good,'positive'),(broken,faulty,bad,'single edit')]:
        d = ROOT/'fixtures'/n
        d.mkdir(exist_ok=True)
        (d/'model.relca').write_text(s)
        (d/'expected.json').write_text(json.dumps(e,ensure_ascii=False,indent=2)+'\n')
        (d/'calc.md').write_text(f'# {n}\n\nADR 0036; design v0.2 §22.3. Hand oracle, before implementation.\n\n'+calc+
            '\n\nThe pair changes only '+repr(before)+' to '+repr(after)+'.\n'+
            f'This is the {side} side. Expected verdict: {e["verdict"]}; exit '+
            str({'satisfied':0,'violated':1,'inconclusive':2,'invalid':3,'unsupported':4}[e['verdict']])+'.\n'+
            'Not an implementation observation. New-policy parsing is expected RED at Stage C.\n'+
            (PARSE_CUT_NOTE+'\n' if CUT_UNDECLARED in e['diagnostics'] else ''))
        if invocation:
            (d/'invocation.json').write_text(json.dumps(invocation)+'\n')


SAMPLE = 'simulation replications 4 seed 0 confidence 0.5'
assign = model('  people [c1,c2]\n'+actor('Clerk','work',3,'executors [c1,c2]'),
               'a = Clerk.work(); end done',['Clerk'],FIFO+' assign first_idle',2,'0min,1min',extra=SAMPLE,
               verify='verify reachable end.done')
pair('assign',assign,' assign first_idle','',sim(),sim(False),
     'P01: c1 occupies [0,3), c2 [1,4). Each elapsed/work=3. Four full replications give done count4 per case. Without assign, time0 has two executor choices and replication1 aborts, not a uniform draw.',{'command':'simulate'})
sep = model('  people [c1,c2,a1]\n'+actor('Clerk','enter',2,'executors [c1,c2]')+
            actor('Approver','approve',1,'executors [c2,a1]'),
            'a = Clerk.enter(); b = Approver.approve(); end done',['Clerk','Approver'],FIFO+' assign first_idle',
            extra='contract Ctl { separation Clerk.enter.finish, Approver.approve.finish }')
pair('separation',sep,'executors [c1,c2]','executors [c2,c1]',
     check([3],3,diagnostics=[warning('a1')]),
     check([3],3,'violated',diagnostics=[warning('c1'),warning('a1')]),
     'P02: enter [0,2), approve [2,3), work=2+1=3. First idle yields c1 then c2: separated. Swapping only Clerk order yields c2 then c2: violated. a1 is unused; in the bad side c1 is also unused. Agency potential people are unchanged.')
fork = 'fork f { a = A.x(); b = A.y() }; join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done; end done'
tie_actor = model(
    '  actor A { capacity 1; capability x() -> T { duration fixed 3min; result nondet { pass }; cancel cooperative ack_within 1min }; capability y() -> T { duration fixed 5min; result nondet { pass }; cancel cooperative ack_within 1min } }',
    fork,['A'],FIFO+' then call_order',extra='contract Due { response case.received -> one_of(end.done) by 4min after case.received otherwise unresolved }')
pair('tie-actor',tie_actor,'a = A.x(); b = A.y()','b = A.y(); a = A.x()',
     check([(3,4)],(3,4),result=3),check([(5,6)],(5,6),'violated',5),
     'P03: x wins at3; y is queued-cancelled (finish3,work3) or starts at3 before adoption and acknowledges at4 (finish4,work4). Reversed fork starts y, so result5 and completion/work5..6. Deadline4 holds only in positive. Tie list remains [a,b].')
tie_person = model('  people [p]\n'+actor('A','x',3,'executors [p]',result='pass')+actor('B','y',5,'executors [p]',result='pass'),
                   edit(fork,'A.y()','B.y()'),['A','B'],extra='contract Due { response case.received -> one_of(end.done) by 4min after case.received otherwise unresolved }')
pair('tie-person',tie_person,'a = A.x(); b = B.y()','b = B.y(); a = A.x()',
     check([(3,4)],(3,4),result=3),check([(5,6)],(5,6),'violated',5),
     'P04: same arithmetic as P03. Separate Actor slots do not create a second p. Occupancy is global across actors. x first gives3..4; y first gives5..6.')
resource = model('  resource R capacity 1\n'+actor('A','x',2,'capacity 2',use='uses R:1;')+actor('B','y',3,'capacity 2',use='uses R:1;'),
                 'a = A.x(); b = B.y(); end done',['A','B'],FIFO+' then call_order',2,'0min,0min',clock=10)
cut = check([7,10],5,'inconclusive'); cut['bounds']['truncated']=True; cut['case_outcomes'][1]['terminals']=[]
pair('resource',resource,'clock 10min','clock 9min',check([7,10],5),cut,
     'P05: R intervals x1[0,2),x2[2,4),y1[4,7),y2[7,10). At2 key(0,2) precedes(2,1), so completion7,10 and work5 each. Clock9 cuts case2 without an invented terminal. Resource release at endpoints is half-open.')
control = edit(assign,'executors [c1,c2];','executors [c1,c2]; control policy;')
pair('control',control,'control policy','control environment',sim(),sim(False),
     'P06: same occupancy as P01. Explicit environment has an empty policy set, valid lowering but two runtime executor successors at0. Reject in simulation, not statically.',{'command':'simulate'})
prob_decls = ('  people [c1,c2]\n  observed case { v: T = ok }\n'+
              '  kernel K { * -> { 0.9: ok; 0.1: bad } }\n'+
              '  actor Clerk { executors [c1,c2]; capability work() -> T { duration fixed 3min; outcomes kernel K; writes v } }')
prob = model(prob_decls,'a = Clerk.work(); end done',['Clerk'],BOTH,2,'0min,0min',verify='verify probability reached v == bad <= 0.1')
pair('label-cofinish',prob,' assign first_idle','',check([3,3],3),check([3,3],3),
     'P07: each independent draw has bad mass1/10. Both finish at3; their order is environment and retains a scheduler row. Positive fixed-policy; removing assignment leaves policy choices so full-information. Simulation positive rejects at3; bad at0.')
dtmc = edit(prob,'0min,0min','0min,1min')
pair('label-dtmc',dtmc,'0min,1min','0min,0min',check([3,4],3,arrivals=[0,1]),check([3,3],3),
     'P08: starts0,1 and finishes3,4 are distinct. Each bad=1/10; no unresolved choice so no scheduler row. One arrival changed to0 introduces two cofinishes and fixed-policy row, not a policy choice.')
env = model('  observed case { v: T = ok; flag: T = ok }\n  kernel K { * -> { 0.9: ok; 0.1: bad } }\n'+
            '  actor A { capacity 1; capability draw() -> T { duration fixed 2min; outcomes kernel K; writes v } }\n'+edit(actor('B','choose',1,result='ok, late'),'result nondet { ok, late };','result nondet { ok, late }; writes flag;'),
            'fork f { a = A.draw(); b = B.choose() }; join all f; end done',['A','B'],verify='verify probability reached v == bad && flag == late <= 0.1')
pair('label-env',env,' then call_order','',check([2],3),check([2],3),
     'P09: A[0,2),B[0,1),work3. Environment can always select ok (prob0) or late (prob1/10), independent of bad draw. Positive fixed-policy. Removing call order leaves a/b starts as policy alternatives: full-information, same probability/time.')
latent = model('  latent case { hard: bool ~ { true:0.5,false:0.5 } }\n  observed case { v: T = ok }\n'+
               '  kernel K given [hard] { true -> { 1: bad }; false -> { 1: ok } }\n'+
               '  actor Clerk { capacity 1; capability work() -> T { kernel_reads latent [hard]; duration fixed 2min; outcomes kernel K; writes v } }',
               'a = Clerk.work(); end done',['Clerk'],FIFO+' then call_order',2,'0min,0min',verify='verify probability reached v == bad <= 0.5')
pair('latent',latent,'then call_order','then hard',check([2,4],2),static('U-SYN-0'),
     'P10: (T,T),(T,F),(F,T),(F,F) have mass1/4. All start case1 at0, case2 at2, finish2,4 and work2. Each marginal bad1/2. In particular (F,T) must not reverse. then hard is a syntax refusal, not latent-policy semantic error.',bad_static=True)
refuse = edit((ROOT/'fixtures/aobs-shared-order/model.relca').read_text(),'  arrivals fixed','  '+FIFO+' then call_order\n  arrivals fixed')
refuse_good = {'verdict':'satisfied','diagnostics':[], 'bounds':{'truncated':False},'terminals':[],
              'case_outcomes':[{'case_id':1,'arrival':0,'terminals':[terminal(1,1,end='posted')]},
                               {'case_id':2,'arrival':0,'terminals':[terminal(1,0,end='denied')]}]}
pair('refuse',refuse,'      requires posted == 0 else rejected\n','',refuse_good,
     {'verdict':'violated','diagnostics':[],'bounds':{'truncated':False,'range_violation_stops':True},'terminals':[],
      'case_outcomes':[{'case_id':1,'terminals':[terminal(1,1,end='posted')]},{'case_id':2,'terminals':[]}]},
     'P11: case1 post starts0, writes posted=1 at1. Case2 is selected next and requires false refuses at1 with work0 and no occupancy. Removing requires attempts2 above upper1 at2; no successful case2 terminal. Refusal does not reserve an executor or increment the state.')
names = sep
for a,b in [('Clerk','Zebra'),('Approver','Minister'),('c1','z1'),('c2','y2'),('a1','x1')]:
    names = edit(names,a,b)
names = edit(names,'actor Zebra {','actor Zebra { grain table;')
pair('names',names,'executors [z1,y2]','executors [y2,z1]',
     check([3],3,diagnostics=[warning('x1')]),check([3],3,'violated',diagnostics=[warning('z1'),warning('x1')]),
     'P12: bijection Clerk->Zebra,Approver->Minister,c1->z1,c2->y2,a1->x1 preserves order, occupancy and work3. First idle uses z1,y2 even though lexical order differs. Swapping only Zebra executors gives y2 twice and violates separation. grain is uninterpreted.')
static_base = edit(assign,FIFO+' assign first_idle',BOTH)
for suffix,before,after,code in [
    ('round-robin','assign first_idle','assign round_robin','U-SYN-0'),
    ('random','assign first_idle','assign random','U-SYN-0'),
    ('skill','assign first_idle','assign by skill','U-SYN-0'),
    ('priority',BOTH,'policy priority','U-SYN-0'),
    ('optimize',BOTH,BOTH+'\n  optimize','U-SYN-0'),
    ('synthesize',BOTH,BOTH+'\n  synthesize','U-SYN-0'),
    ('duplicate-assign','assign first_idle','assign first_idle assign first_idle','U-SEM-0'),
    ('duplicate-then','then call_order','then call_order then call_order','U-SEM-0'),
    ('duplicate-policy',BOTH,BOTH+'\n  '+BOTH,'U-SEM-0'),
    ('block',BOTH,'policy { fifo }','E-SYN-0'),
]:
    pair('static-'+suffix,static_base,before,after,check([3,4],3,arrivals=[0,1]),static(code),
         f'P13 {suffix}: positive declaration fixes starts0,1, finishes3,4, work3 each. The single syntactic edit must fail closed with {code}, before exploration. No probability or timing is inferred for the static negative.',bad_static=True)
observable = edit(assign,'policy fifo','policy observable fifo')
pair('observable-assign',observable,' assign first_idle','',sim(),sim(False),
     'P14: observable changes only queue spelling, not semantics. Assignment retained yields P01 times/counts. Removing only assignment retains observable but cannot choose between c1/c2 at0.',{'command':'simulate'})
obsfifo = model(actor('A','work',1),'a = A.work(); end done',['A'],
                'policy observable fifo tie_by case_id',2,'0min,0min',extra=SAMPLE,verify='verify reachable end.done')
pair('observable-fifo',obsfifo,'observable ','',sim(arrivals=[0,0]),sim(arrivals=[0,0]),
     'P15: one anonymous slot serves case1[0,1),case2[1,2). Four replications complete both; no classes or assign row on either side. Removing observable must preserve exact census, times1,2 and all semantics.',{'command':'simulate'})
hol_decl = ('  resource R capacity 1\n  resource S capacity 1\n'+actor('H','hold',3,use='uses S:1;')+
            actor('A','work',2,use='uses R:1,S:1;')+actor('B','work',1,use='uses R:1;'))
hol_body = 'fork f { h = H.hold(); a = A.work(); b = B.work() }; join all f; end done'
hol = model(hol_decl,hol_body,['H','A','B'],FIFO+' then call_order',clock=10,extra='metrics { window 10min; currency "JPY" }')
pair('resource-hol',hol,'uses R:1,S:1','uses R:1',check([6],6),check([3],6),
     'P16: H[0,3), A[3,5), B[5,6). A waiting on S still heads R: B cannot start0. Work6; R quantity-time3/window10=3/10, S5/10=1/2. Remove A S demand: H[0,3),A[0,2),B[2,3),completion3; R3/10,S3/10.')
reserve = model('  people [p]\n  resource S capacity 1\n'+actor('H','hold',3,use='uses S:1;')+
                actor('A','work',2,'executors [p]',use='uses S:1;')+actor('B','work',1,'executors [p]'),
                hol_body,['H','A','B'])
pair('no-person-reservation',reserve,'duration fixed 1min','duration fixed 4min',check([5],6),check([6],9),
     'P17: H[0,3) blocks A on S, but does not reserve p. B takes p[0,1), A[3,5): finish5, work3+2+1=6. Make B4: B[0,4),A[4,6): finish6,work9. Reserving p would incorrectly defer B until5.')
cross = edit(tie_person,'actor B {','actor B { control environment;')
pair('cross-class-person',cross,'control environment','control policy',
     check([(3,6)],(3,6),'violated',(3,5)),check([(3,4)],(3,4),result=3),
     'P18: both A-first(result3,complete3..4) and environment B-first(result5,complete5..6) remain. Deadline4 violated. Changing B control to policy fixes A-first only. Neither class may be globally prioritized.')
crossr = edit(tie_person,'  people [p]','  resource R capacity 1')
crossr = edit(crossr,'executors [p]','capacity 1')
crossr = edit(crossr,'duration fixed','uses R:1; duration fixed')
crossr = edit(crossr,'actor B {','actor B { control environment;')
pair('cross-class-resource',crossr,'control environment','control policy',
     check([(3,6)],(3,6),'violated',(3,5)),check([(3,4)],(3,4),result=3),
     'P18R: anonymous actors share R1 instead of p. Same exclusive intervals and deadline calculation as P18. Environment-first must survive the resource HOL rule.')
tie = model('  latent case { coin: bool ~ { true:0.5,false:0.5 } }\n'+actor('A','work',2,result='pass')+actor('B','work',2,result='pass'),
            'fork f { a = A.work(); b = B.work() }; join any f where result == pass tie by [a,b] remaining drain if none -> done; end done',
            ['A','B'],FIFO+' then call_order',verify='verify probability reached coin == true <= 0.5')
pair('tie-nondet',tie,'tie by [a,b]','tie nondet',check([2],4,result=2),check([2],4,result=2),
     'P19: both work[0,2), work4, visible result2, complete2. Initial coin mass1/2 unaffected. Fixed tie has only environment completion-order choices so fixed-policy. Nondet tie has two winners once both finish, so full-information.')
permission = model('  people [p,q]\n'+actor('A','work',1,'executors [p,q]'),
                   'a = A.work(); end done',['A'],FIFO+' assign first_idle',
                   extra='permit q to A.work\n  contract Ctl { permitted A.work }')
pair('permission',permission,'executors [p,q]','executors [q,p]',
     check([1],1,'violated',diagnostics=[warning('q')]),check([1],1,diagnostics=[warning('p')]),
     'P20: first declared p starts[0,1), despite permit only q. permitted violated. Swap order: q works, satisfied. Permission/Authority are monitors, never candidate filters. Unused executor warning changes only person.')
late = model(actor('A','x',3,'capacity 2')+actor('B','y',2),
             'a = A.x(); b = B.y(); end done',['A','B'],FIFO+' then call_order',2,'0min,0min')
e = check([(5,7),(5,7)],5)
pair('late-enqueue',late,'capacity 2','capacity 1',e,check([5,8],5),
     'P21: x1,x2[0,3). If x2 finishes first and y2 starts before x1 finishes, case2 ends5 and case1 ends7; converse also allowed. Range5..7 each, work5. Capacity1 gives x1[0,3),x2[3,6),y1[3,5),y2[6,8). Never compare tasks not queued yet.')
noop = edit(assign,'  people [c1,c2]\n','')
noop = edit(noop,'executors [c1,c2]','capacity 2')
# people are deliberately declared in both sides; only capacity/executors changes.
noop = edit(noop,'  actor Clerk','  people [c1,c2]\n  actor Clerk')
pair('no-effect',noop,'capacity 2','executors [c1,c2]',sim(),sim(),
     'P22: anonymous capacity2 starts0,1 and ends3,4,work3 each. assign has no application target; assumption says so. Replacing only capacity with named executors uses c1,c2 and removes that no-op suffix. Both samples count4 each.',{'command':'simulate'})
dead = edit(hol,'resource S capacity 1','resource S capacity 0')
deadgood = {'verdict':'violated','bounds':{'truncated':False},'terminals':[],
            'case_outcomes':[{'case_id':1,'arrival':0,'terminals':[]}],
            'diagnostics':[{'code':'W-DEADLOCK-0'},{'code':'W-UNREACH-0'},{'code':'W-UNREACH-0'},{'code':'W-UNREACH-0'},{'code':'W-UNREACH-2'}]}
pair('deadlock',dead,'resource S capacity 0','resource S capacity 1',deadgood,check([6],6),
     'P23: H cannot obtain S0, A waits S0 and heads R, B cannot overtake A on R. No task starts, clock0, work0, genuine deadlock, not truncated. Three unstarted capabilities and done unreachable. Restoring S1 yields P16 completion6/work6.')
config = model('  people [p,q]\n'+actor('A','work',2,'executors [p,q]')+actor('B','work',3),
               'a = A.work(); end done',['A','B'],extra=SAMPLE,verify='verify reachable end.done')
config = edit(config,'  cut C','  process Q { b = B.work(); end done }\n  configuration CP { cut C; process P }\n  configuration CQ { cut C; process Q }\n  cut C')
pair('configuration',config,' assign first_idle','',sim(cases=1,arrivals=[0]),sim(False,cases=1),
     'P24: selecting CP yields A[0,2), CQ B[0,3). Default control follows selected process: CP policy[A],environment[B], CQ policy[B],environment[A]. Both simulations finish with assign; without it CP rejects at0 while CQ remains deterministic. Compare must carry exact selected reports, not reject configuration.',{'command':'simulate','configuration':'CP'})

# Additional scheduler controls in plan section 3.4, each with one actual source edit.
labelcut = edit(env,'  observed case','  latent case { coin: bool ~ { true:0.5,false:0.5 } }\n  observed case')
labelcut = edit(labelcut,'clock 20min','clock 2min')
cutbad = {'verdict':'inconclusive','bounds':{'truncated':True},'diagnostics':[],'terminals':[],
          'case_outcomes':[{'case_id':1,'terminals':[]}]}
pair('label-cut',labelcut,'clock 2min','clock 1min',check([2],3),cutbad,
     'LABEL-CUT: initial independent coin supplies chance before any work; B result at1 supplies environment nondeterminism. Clock2 completes A kernel and gives fixed-policy. Clock1 cuts before A completes, scheduler remains present but conservatively full-information. Do not erase row or infer no unresolved future policy.')
single = tie
single = edit(single,actor('B','work',2,result='pass'),actor('B','work',2,result='ok'))
pair('single-winner',single,'tie by [a,b]','tie nondet',check([2],4,result=2),check([2],4,result=2),
     'TIE single winner: only A returns pass; B ok does not qualify. Both complete2, work4, coin1/2. Nondet syntax alone does not create two policy winners; both sides fixed-policy.')
ack = edit(tie,actor('B','work',2,result='pass'),edit(actor('B','work',5,result='pass'),'ack_within 1min','ack_within 2min'))
ack = edit(ack,'remaining drain','remaining cancel_then_drain')
pair('tie-ack',ack,'tie by [a,b]','tie nondet',check([(3,4)],(5,6),result=2),check([(3,4)],(5,6),result=2),
     'TIE-ACK: A ends2, only winner; B starts0 and is cancelled at2 with ack at3 or4. End3..4,work A2+B3..4=5..6,coin1/2. Two ack children are environment, not two policy winners; fixed-policy on both sides.')

names = sorted(n for p in PAIRS for n in [p['good'],p['bad']])
(HERE/'policy_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
(HERE/'policy_names.json').write_text(json.dumps(names,indent=2)+'\n')
print(f'{len(PAIRS)} pairs; {len(names)} closed policy fixture directories')
