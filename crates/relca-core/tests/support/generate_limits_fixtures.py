"""Materialize ADR0039 hand oracles. Never invokes Relca or reads its output.

Time is minutes. The tables below are worked before execution. P17/P18 and
P17b depend on state invariants and use separate integration checks.
"""
import copy
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
BASE = '''model Limits version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1; capability run() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } } }
  process Flow { r = A.run(); end done }
  arrivals fixed [0min, 0min, 0min]
  policy fifo tie_by case_id
  LIMIT
  bounds { cases 3; live_tasks 1; clock 20min }
  verify safety no_resource_overcommit
  EXTRA
}
'''
METRICS = 'metrics { window 20min; currency "JPY" }'
WAIT1 = 'queue capacity 1 on_overflow wait_external fifo'
WAIT2 = 'queue capacity 2 on_overflow wait_external fifo'
CONCURRENT = 'concurrent cases 1 on_overflow wait_external fifo'
END = 'queue capacity 1 on_overflow end overflowed'
SAFETY = {'id': 'safety no_resource_overcommit', 'verdict': 'satisfied'}
PAIRS = []
FIXTURES = {}


def edit(s, a, b):
    t = s.replace(a, b)
    assert t != s, (a, b)
    return t


def source(limit=WAIT1, extra=''):
    return edit(edit(BASE, 'LIMIT', limit), 'EXTRA', extra)


def response(d, ends='end.done', trigger='case.received'):
    return f'contract C {{ response {trigger} -> one_of({ends}) by {d}min after {trigger} otherwise unresolved }}'


def rg(a, b=None):
    return {'min': a, 'max': a if b is None else b}


def scalar(n, reason=None, sample=False):
    s = {'min': None if n is None else str(n), 'max': None if n is None else str(n), 'reason': reason}
    if sample:
        s.update(mean=None if n is None else str(n), samples=0 if n is None else 10)
    return s


def count(n, sample=False):
    return {'stats': scalar(n, sample=sample), 'observed_total': n * 10 if sample else None}


def oracle(verdict='satisfied', arrivals=(0, 0, 0), admitted=(0, 0, 2),
           completions=(2, 4, 6), overflow=(), rejected=(), peak: int | None=2,
           contract=None, probability=None, clock=20, legacy=False, new=True):
    n = len(arrivals)
    h: dict[str, Any] = dict(verdict=verdict, mode='exhaustive', unit='min', cut='undeclared',
             bounds=dict(cases=n, live_tasks=1, clock=clock, truncated=clock < 6 and not legacy),
             diagnostics=[dict(code='W-CUT-0', severity='warning')],
             absent_diagnostics=['W-BUDGET-0', 'W-DEADLOCK-0'], properties=[copy.deepcopy(SAFETY)], terminals=[])
    h['bounds']['truncated'] = False
    if legacy:
        h['terminals'] = [dict(end='done', elapsed=rg(2), work=rg(2), result_at=None)]
        return h
    h['admission'] = {'rejected_count': rg(len(rejected))}
    if peak is not None:
        h['admission']['external_wait_max'] = rg(peak)
    if overflow:
        h['admission']['overflow_count'] = rg(len(overflow))
    cases = []
    for i, arrival in enumerate(arrivals):
        c: dict[str, Any] = dict(case_id=i+1, arrival=arrival, terminals=[])
        if admitted is not None:
            c['admitted_at'] = None if admitted[i] is None else rg(admitted[i])
        if i in rejected:
            c['terminals'] = [dict(kind='rejected', end=None, completed_at=None, elapsed=None, work=None, result_at=None)]
        elif completions[i] is not None:
            end, work = ('overflowed', 0) if i in overflow else ('done', 2)
            c['terminals'] = [dict(kind='overflow' if i in overflow else 'end', end=end,
                                   completed_at=rg(completions[i]), elapsed=rg(completions[i]-arrival),
                                   work=rg(work), result_at=None)]
        cases.append(c)
    h['case_outcomes'] = cases
    if contract is not None:
        h['properties'].append(dict(id='contract C #1 response', verdict=max(contract, key=['satisfied','vacuous','inconclusive','unsupported','violated'].index),
                                    cases=[dict(case_id=i+1, arrival=a, verdict=v) for i,(a,v) in enumerate(zip(arrivals,contract))]))
    if probability is not None:
        vals, cv = probability
        h['properties'].append(dict(id='probability reached #1', verdict=verdict,
                                    cases=[dict(case_id=i+1, arrival=a, verdict=v, probability=rg(lo, hi)) for i,(a,v,(lo,hi)) in enumerate(zip(arrivals,cv,vals))]))
        for c in cases:
            for t in c['terminals']:
                t['probability'] = rg('1')
    return h


def metrics(h, admitted, external=None, sample=False, overflow=(), new=True):
    cs = h['case_outcomes']
    rejected = sum(bool(c['terminals']) and c['terminals'][0]['kind']=='rejected' for c in cs)
    completed = sum(bool(c['terminals']) and c['terminals'][0]['kind']!='rejected' for c in cs)
    counts: dict[str, Any] = {k: count(v, sample) for k,v in dict(offered=len(cs), received=len(cs)-rejected, completed=completed,
                                                rejected=rejected, unfinished=len(cs)-completed-rejected,
                                                deadlocked=0, not_arrived=0).items()}
    if new:
        counts['admitted'] = count(sum(x is not None for x in admitted), sample)
    if overflow:
        counts['overflowed'] = count(len(overflow), sample)
    inventory=['done']+(['overflowed'] if overflow else [])
    counts['end_counts']=[dict(end=end,count=count(sum(bool(c['terminals']) and c['terminals'][0]['end']==end for c in cs),sample)) for end in inventory]
    rows = []
    for i,c in enumerate(cs):
        row: dict[str, Any] = dict(case_id=i+1)
        if external is not None:
            row['external_wait'] = scalar(external[i], 'unfinished' if external[i] is None else None, sample)
        if c['terminals'] and c['terminals'][0]['kind']=='end':
            row['first_start_wait'] = scalar(c['terminals'][0]['completed_at']['min']-2-c['arrival'], sample=sample)
        if c['terminals'] and c['terminals'][0]['kind']!='rejected':
            row['work'] = scalar(c['terminals'][0]['work']['min'], sample=sample)
        if new:
            row['counts'] = {'admitted': count(int(admitted[i] is not None), sample)}
        if overflow:
            row.setdefault('counts', {})['overflowed'] = count(int(i in overflow), sample)
        row.setdefault('counts', {})['end_counts']=[dict(end=end,count=count(int(bool(c['terminals']) and c['terminals'][0]['end']==end),sample)) for end in inventory]
        if i in overflow:
            row['observed_cost_total'] = scalar(None, 'never_created', sample)
            row['tasks'] = [dict(binding='r', starts=scalar(0, sample=sample), work=scalar(0, sample=sample),
                                 wait=scalar(None,'never_created',sample), never_created_count=count(1,sample))]
        rows.append(row)
    work=sum(c['terminals'][0]['work']['min'] for c in cs if c['terminals'] and c['terminals'][0]['kind']!='rejected')
    starts=sum(bool(c['terminals']) and c['terminals'][0]['kind']=='end' for c in cs)
    h['metrics'] = dict(cases=rows, overall={'counts': counts, 'work':scalar(work,sample=sample)},
                        capabilities=[dict(capability='A.run',starts=scalar(starts,sample=sample))],
                        actors=[dict(actor='A',work=scalar(work,sample=sample),peak_tasks=scalar(int(starts>0),sample=sample))])
    return h


def failure(code):
    return dict(verdict='invalid' if code.startswith('E') else 'unsupported', mode='exhaustive',
                diagnostics=[dict(code=code,severity='error')], properties=[],terminals=[])


def put(name, s, h, calc, invocation=None):
    assert name not in FIXTURES
    s='\n'.join(line.rstrip() for line in s.splitlines())+'\n'
    assert '//' not in s and '#' not in s
    FIXTURES[name] = (s,h,calc,invocation)


def pair(base, s, old, new, good, bad, calc, fault='change', invocation=None):
    name = 'lim-'+base
    broken = name+'.broken-'+fault
    t = edit(s,old,new)
    put(name,s,good,calc,invocation)
    put(broken,t,bad,calc+f'\nSingle transformation: {old!r} -> {new!r}.',invocation)
    PAIRS.append(dict(good=name,bad=broken,old=old,new=new,invocation=invocation))


def build():
    ss = ['satisfied']*3
    sv = ['satisfied','satisfied','violated']
    vv = ['violated']*3
    times = 'Task A.run holds one slot on [0,2), [2,4), [4,6). C=(2,4,6), work=(2,2,2). '
    q = 'At b, capacity2 admits IDs1,2 and holds3; f starts1, then g admits3 at0. Capacity1 initially admits1, g admits2 at0, then after start2 at2 admits3 at2. Peak Held is respectively1 and2. '
    cw = 'Concurrent1 includes running cases until their business end: A=(0,2,4), C=(2,4,6), external_wait=(0,2,4). '
    ov = 'At b queue1 reserves case1; cases2,3 are received and end overflowed at0 without admission or Task. C=(2,0,0), work=(2,0,0). '
    hq = oracle('violated',admitted=(0,0,0),peak=1,contract=sv)
    hn = oracle('violated',contract=sv)
    pair('queue-wait',source(WAIT2,response(4)),WAIT2,WAIT1,hq,hn,times+q+'Receipt deadlines4 give S,S,V.',fault='narrow')
    pair('readmit',source(WAIT1,response(4)+'\n  '+METRICS),WAIT1,WAIT2,
         metrics(copy.deepcopy(hn),(0,0,2),(0,0,2)),metrics(copy.deepcopy(hq),(0,0,0),(0,0,0)),
         times+q+'First_start_wait is arrival-based (0,2,4), not admission-based (0,2,2). K01 is an implementation mutation, not a duplicate fixture.',fault='wide')
    ho = oracle(admitted=None,completions=(2,0,0),overflow=(1,2),peak=None,contract=ss)
    hov = oracle('violated',admitted=None,completions=(2,0,0),overflow=(1,2),peak=None,contract=sv[:1]+['violated']*2)
    pair('overflow',source(END,response(4,'end.done, end.overflowed')),'end.done, end.overflowed','end.done',ho,hov,ov+'Removing the explicit target violates both receipt obligations. Overflow end is excluded from F.',fault='hidden')
    hc = oracle(admitted=(0,2,4),contract=ss)
    hcv = oracle('violated',admitted=(0,2,4),contract=sv)
    pair('concurrent-wait',source(CONCURRENT,response(6)+'\n  '+METRICS),'by 6min','by 5min',
         metrics(copy.deepcopy(hc),(0,2,4),(0,2,4)),metrics(copy.deepcopy(hcv),(0,2,4),(0,2,4)),times+cw+'6<=6; 6>5.',fault='short')
    s = edit(source('concurrent cases 2 on_overflow reject',METRICS),'[0min, 0min, 0min]','[0min, 1min, 2min]')
    a = oracle(arrivals=(0,1,2),admitted=None,completions=(2,4,None),rejected=(2,),peak=None)
    b = oracle(arrivals=(0,1,2),admitted=None,completions=(2,4,6),peak=None)
    pair('count',s,'concurrent cases 2','queue capacity 2',metrics(a,(0,1,None)),metrics(b,(0,1,2),new=False),
         'At t2 b sees concurrent=2 before case1 finishes; queue sees only unstarted case2=1. Concurrent rejects3; queue admits3. C=(2,4) vs (2,4,6); elapsed=(2,3) vs (2,3,4).',fault='queue')
    allprob = ([('1','1')]*3,ss)
    pgood = oracle(probability=allprob)
    pbad = oracle('violated',probability=([('1','1'),('1','1'),('0','0')],sv))
    pair('within',source(WAIT1,'verify probability reached hit == true within 6min >= 1'),'within 6min','within 5min',pgood,pbad,times+q+'Within is measured from arrival0; case3 hits at6, outside5.',fault='strict')
    cut = oracle('inconclusive',admitted=(0,2,None),completions=(2,None,None),contract=['satisfied','inconclusive','inconclusive'],clock=3)
    cut['bounds']['truncated'] = True
    cut['properties'][0]['verdict'] = 'inconclusive'
    metrics(cut,(0,2,None),(0,2,None))
    def truncate(x):
        if isinstance(x,dict):
            if set(('min','max','reason')) <= x.keys():
                x.update(min=None,max=None,reason='truncated')
            for v in x.values(): truncate(v)
        elif isinstance(x,list):
            for v in x: truncate(v)
    truncate(cut['metrics'])
    pair('clock',source(CONCURRENT,response(6)+'\n  '+METRICS),'clock 20min','clock 3min',metrics(copy.deepcopy(hc),(0,2,4),(0,2,4)),cut,
         times+cw+'At clock3 case1 complete, case2 running, case3 Held: received3/admitted2/completed1/unfinished2/rejected0. Exhaustive metrics withdraw EVERY Scalar to null/truncated; admitted_at=(0,2,null).',fault='short-clock')
    mass = edit(edit(source(WAIT1,'verify probability reached hit == true >= 1'),'duration fixed 2min','duration {1min:0.5,3min:0.5}'),'[0min, 0min, 0min]','[0min, 1min, 2min]')
    m = oracle(arrivals=(0,1,2),admitted=None,probability=allprob)
    m.pop('case_outcomes')
    m['admission']={'rejected_count':rg(0)}
    mb=copy.deepcopy(m);mb['verdict']='inconclusive';mb['bounds'].update(clock=8,truncated=True)
    mb['properties'][1]['verdict']='inconclusive';mb['properties'][1]['cases'][2].update(verdict='inconclusive',probability=rg('7/8','1'))
    mass=edit(mass,'  verify safety no_resource_overcommit\n','')
    m['properties']=m['properties'][1:]
    mb['properties']=mb['properties'][1:]
    pair('mass',mass,'clock 20min','clock 8min',m,mb,
         'Eight equiprobable duration triples: 111->finishes123;113->125;131->145;133->147;311->345;313->347;331->367;333->369. Case3 admission=(2,2,2,2,3,3,3,3). Only333 exceeds clock8. Retain unknown1/8: probability[7/8,1], not1. Case3 C3..9, elapsed1..7, external_wait0..1.',fault='clock')
    renamed=edit(edit(source(END,response(4,'end.done, end.overflowed')),'actor A {','actor Desk { grain team;'),'A.run()','Desk.run()')
    renamed=edit(renamed,'overflowed','spill')
    hl=copy.deepcopy(ho)
    for c in hl['case_outcomes'][1:]: c['terminals'][0]['end']='spill'
    # A label transformation is intentionally not counted as an input-defect pair.
    put('lim-label',source(END,response(4,'end.done, end.overflowed')),copy.deepcopy(ho),ov)
    put('lim-label.broken-relabel',renamed,hl,ov+'Rename Actor to Desk, grain team and overflow terminal to spill; semantic time/counts unchanged.')
    PAIRS.append(dict(good='lim-label',bad='lim-label.broken-relabel',label=True))
    s=source('concurrent cases 1 on_overflow reject')
    s=edit(edit(edit(s,'  arrivals fixed [0min, 0min, 0min]\n',''),'  policy fifo tie_by case_id\n',''),'cases 3','cases 1')
    pair('legacy-route',s,'  concurrent cases 1 on_overflow reject\n','',oracle(arrivals=(0,),admitted=None,completions=(2,),peak=None),oracle(arrivals=(0,),legacy=True),
         'No arrivals, policy or queue. Concurrent alone forces multi-case JSON even for cases1. Removing concurrent restores legacy terminal done elapsed/work2.',fault='no-limit')
    hb=oracle(admitted=None,completions=(2,None,None),rejected=(1,2),peak=None)
    pair('both',source('queue capacity 1 on_overflow reject\n  concurrent cases 2 on_overflow reject'),'concurrent cases 2 on_overflow reject','concurrent cases 2 on_overflow wait_external fifo',hb,failure('U-SEM-0'),
         'Both predicates must hold. Queue1 rejects IDs2,3 at0; concurrent2 does not override it. Mismatched overflow actions are unsupported.',fault='disagree')
    pair('counts',source(END,response(4,'end.done, end.overflowed')+'\n  '+METRICS),'on_overflow end overflowed','on_overflow reject',metrics(copy.deepcopy(ho),(0,None,None),overflow=(1,2)),failure('E-NAME-0'),ov+'Removing the overflow declaration leaves an undeclared response target: E-NAME-0 before metrics.',fault='reject')
    bare=oracle(admitted=None,completions=(2,0,0),overflow=(1,2),peak=None)
    pair('counts-bare',source(END,METRICS),'on_overflow end overflowed','on_overflow reject',metrics(copy.deepcopy(bare),(0,None,None),overflow=(1,2)),metrics(copy.deepcopy(hb),(0,None,None),new=False),
         ov+'No contract on either side. Overflow offered3/received3/admitted1/completed3/overflowed2/rejected0. Reject offered3/received1/completed1/rejected2, admitted and overflowed keys absent.',fault='reject')
    rejectc=copy.deepcopy(hb);rejectc.update(verdict='vacuous');rejectc['properties'].append(dict(id='contract C #1 response',verdict='vacuous',cases=[dict(case_id=i+1,arrival=0,verdict=v) for i,v in enumerate(['satisfied','vacuous','vacuous'])]))
    pair('counts-obligations',source(END,response(4)+'\n  '+METRICS),'on_overflow end overflowed','on_overflow reject',metrics(copy.deepcopy(hov),(0,None,None),overflow=(1,2)),metrics(rejectc,(0,None,None),new=False),
         ov+'Both targets only done: overflow cases violate, rejected cases are vacuous. Root violated vs vacuous; do not confuse with E-NAME-0 of P12.',fault='reject')
    hs=oracle(admitted=None,peak=None)
    hs['mode']='simulation';hs['verdict']='inconclusive';hs['properties']=[dict(id=SAFETY['id'],verdict='inconclusive',basis='sample',witness=None)]
    hs['simulation']=dict(attempted_replications=10,completed_replications=10,truncated_replications=0)
    hs.pop('admission');hs.pop('case_outcomes')
    simbad=dict(verdict='unsupported',mode='simulation',diagnostics=[dict(code='U-SEM-0',severity='error')],simulation=dict(attempted_replications=1,completed_replications=0))
    # Sample times live only in metrics, not case_outcomes.
    sm=metrics(oracle(admitted=(0,0,0),peak=1),(0,0,0),(0,0,0),sample=True)['metrics']
    for i,c in enumerate(sm['cases']):
        c['terminals']=[dict(kind='end',end='done',lead_time=scalar(2*(i+1),sample=True),count=count(1,True)),
                        dict(kind='rejected',end=None,count=count(0,True))]
    hs['metrics']=sm
    pair('sim',source(WAIT2,METRICS+'\n  simulation replications 10 seed 1 confidence 0.95'),'  policy fifo tie_by case_id\n','',hs,simbad,
         times+q+'Ten deterministic replications: lead2,4,6; E0,0,0; first_start_wait0,2,4; samples10; received/admitted observed_total30. Safety remains inconclusive. Without FIFO first start at0 has two successors: unsupported, attempted1/completed0, no statistics.',fault='no-policy',invocation={'command':'simulate'})
    had=oracle(admitted=(0,2,4),contract=ss)
    hadbad=oracle('violated',admitted=(0,2,4),contract=['satisfied','violated','violated'])
    pair('admitted-deadline',source(CONCURRENT,response(2,trigger='case.admitted')),'case.admitted','case.received',had,hadbad,
         times+cw+'Admission-relative deadlines are2,4,6, exactly completion. Receipt-relative deadlines all2; cases2,3 violate.',fault='receipt')
    hz=oracle(admitted=None,completions=(0,0,0),overflow=(0,1,2),peak=None,contract=ss)
    for c in hz['case_outcomes']: c['terminals'][0]['end']='spill'
    hz['diagnostics'] += [dict(code=c,severity='warning') for c in ['W-UNREACH-0','W-UNREACH-2']]
    hzbad=copy.deepcopy(hz);hzbad['verdict']='violated';hzbad['properties'][1]['verdict']='violated'
    for c in hzbad['properties'][1]['cases']: c['verdict']='violated'
    pair('zero-deadline-overflow',source('queue capacity 0 on_overflow end spill',response(0,'end.spill')),'one_of(end.spill)','one_of(end.done)',hz,hzbad,
         'Queue0 overflows all3 at arrival0. Received precedes end at same0, deadline0 is inclusive. No Task starts; done unreachable. Changing only target violates all3.',fault='target')
    hd=oracle('violated',admitted=(0,None,None),completions=(None,None,None))
    hd['diagnostics'] += [dict(code=c,severity='warning') for c in ['W-DEADLOCK-0','W-UNREACH-0','W-UNREACH-2']]
    hd['absent_diagnostics']=['W-BUDGET-0'];hd['properties'][0]['verdict']='satisfied'
    metrics(hd,(0,None,None),(0,None,None))
    hd['metrics']['overall']['counts'].update(unfinished=count(0),deadlocked=count(3))
    for c in hd['metrics']['cases'][1:]: c['external_wait']=scalar(None,'deadlock')
    pair('deadlock-held',source(CONCURRENT,METRICS),'actor A { capacity 1','actor A { capacity 0',metrics(oracle(admitted=(0,2,4)),(0,2,4),(0,2,4)),hd,
         cw+'Capacity0: case1 admitted but no start; IDs2,3 Held at0; no future event. Deadlock not clock cut, received3/admitted1/deadlocked3/unfinished0. Held external_wait is null/deadlock, not0.',fault='zero-capacity')
    s=edit(source('concurrent cases 1 on_overflow reject'),'[0min, 0min, 0min]','[0min, 2min, 2min]')
    pair('same-time-boundary',s,'[0min, 2min, 2min]','[0min, 3min, 3min]',oracle(arrivals=(0,2,2),admitted=None,completions=(2,None,None),rejected=(1,2),peak=None),oracle(arrivals=(0,3,3),admitted=None,completions=(2,5,None),rejected=(2,),peak=None),
         'At t2 b runs before case1 end: both new cases rejected. At3 case1 already ended; case2 admitted and ends5, case3 rejected.',fault='later')
    he=copy.deepcopy(bare)
    for c in he['case_outcomes'][1:]:c['terminals'][0]['end']='spill'
    pair('both-end-id',source('queue capacity 1 on_overflow end spill\n  concurrent cases 2 on_overflow end spill'),'concurrent cases 2 on_overflow end spill','concurrent cases 2 on_overflow end other',he,failure('U-SEM-0'),ov+'Both limits share ONE EndId spill. Different end IDs are mismatched actions even though both variants are End.',fault='other-end')
    s=edit(source(CONCURRENT,response(6)+'\n  '+METRICS),'[0min, 0min, 0min]','[5min, 5min, 5min]')
    delayed=oracle(arrivals=(5,5,5),admitted=(5,7,9),completions=(7,9,11),contract=ss)
    db=copy.deepcopy(delayed);db['verdict']='violated';db['properties'][1]['verdict']='violated';db['properties'][1]['cases'][2]['verdict']='violated'
    pair('delayed-arrival',s,'by 6min','by 5min',metrics(delayed,(5,7,9),(0,2,4)),metrics(db,(5,7,9),(0,2,4)),
         'Shift arrivals by5: A5,7,9 C7,9,11; elapsed2,4,6; E/S0,2,4. Relative deadline6 is met,5 fails only case3.',fault='short')
    static_source=source(WAIT2,response(4))
    put('lim-static',static_source,copy.deepcopy(hq),'P14 valid control: '+times+q)
    static=[
        ('missing-fifo','wait_external fifo','wait_external','U-SYN-0'),
        ('lifo','wait_external fifo','wait_external lifo','U-SYN-0'),
        ('process','wait_external fifo','process Flow','U-SYN-0'),
        ('actor-queue','actor A {','actor A { queue capacity 2 on_overflow reject;','U-SYN-0'),
        ('reject-and-count','wait_external fifo','reject_and_count','U-SYN-0'),
        ('bounds-queue','live_tasks 1','live_tasks 1; queue 2','U-SYN-0'),
        ('live-tasks-per-case','live_tasks 1','live_tasks_per_case 1','U-SYN-0'),
        ('zero-queue',WAIT2,'queue capacity 0 on_overflow wait_external fifo','E-CON-0'),
        ('zero-concurrent',WAIT2,'concurrent cases 0 on_overflow wait_external fifo','E-CON-0'),
        ('end-conflict',WAIT2,'queue capacity 2 on_overflow end done','E-CON-0'),
        ('duplicate-queue',WAIT2,WAIT2+'\n  '+WAIT2,'U-SEM-0'),
        ('duplicate-concurrent',WAIT2,CONCURRENT+'\n  '+CONCURRENT,'U-SEM-0'),
        ('end-missing-name',WAIT2,'queue capacity 2 on_overflow end','E-SYN-0'),
        ('arrivals-missing','arrivals fixed [0min, 0min, 0min]','concurrent cases 2 on_overflow wait_external fifo','E-ARR-0'),
    ]
    for fault,a,b,code in static:
        name='lim-static.broken-'+fault
        put(name,edit(static_source,a,b),failure(code),'P14 single fault '+fault+': '+code+'; no exploration or numerical claims.')
        PAIRS.append(dict(good='lim-static',bad=name,old=a,new=b,static=True))
    # Extra fail-closed event contexts use valid supporting declarations.
    for stem,goodsrc,a,b,code in [
        ('admitted-reject',source('queue capacity 2 on_overflow reject',response(4)), 'case.received','case.admitted','U-SYN-0'),
        ('admitted-after',source(CONCURRENT,response(2,trigger='case.admitted')), 'after case.admitted','after case.received','E-CON-0'),
        ('admitted-invariant',source(WAIT1,'contract C { invariant A.run.finish -> hit == true }'), 'A.run.finish','case.admitted','U-SYN-0'),
        ('admitted-excludes',source(WAIT1,'contract C { excludes A.run.finish, A.run.start }'), ', A.run.start',', case.admitted','U-SYN-0'),
    ]:
        # Positive event contracts have their own hand oracle in dedicated tests;
        # fixtures fix static classification without inventing unrelated fields.
        good=dict(verdict='satisfied',diagnostics=[dict(code='W-CUT-0',severity='warning')])
        if stem=='admitted-reject':good['verdict']='vacuous'
        pair(stem,goodsrc,a,b,good,failure(code),'P14 '+stem+': exactly one event-context change; '+code+'.',fault='context')
    # Legal zero-limit controls, no received obligation in the rejected side.
    rz=oracle(admitted=None,completions=(None,None,None),rejected=(0,1,2),peak=None)
    rz['diagnostics']+= [dict(code=c,severity='warning') for c in ['W-UNREACH-0','W-UNREACH-2']]
    ez=copy.deepcopy(hz);ez['properties']=[copy.deepcopy(SAFETY)]
    pair('zero-limit',source('queue capacity 0 on_overflow reject'),'on_overflow reject','on_overflow end spill',rz,ez,
         'Queue0 reject is legal and rejects3 with no receipt. End spill is also legal: received3/admitted0/overflow3 at0, work0. Both leave run and done unreachable.',fault='end')
    # Overflow Admitted must be absent even though Received is present.
    a=oracle('vacuous',admitted=None,completions=(2,0,0),overflow=(1,2),peak=None,contract=['satisfied','vacuous','vacuous'])
    pair('overflow-admitted',source(END,response(4,trigger='case.admitted')), 'case.admitted','case.received',a,hov,
         ov+'Admitted obligation exists only for case1; overflow cases vacuous. Received origin obliges all3 and rejects the missing spill target.',fault='receipt')
    # Instant ends exercise repeated same-time readmission, including clock0.
    instant=edit(source(WAIT1),'r = A.run(); end done','end done')
    ih=oracle(admitted=(0,0,0),completions=(0,0,0))
    for c in ih['case_outcomes']: c['terminals'][0]['work']=rg(0)
    ih['diagnostics'].append(dict(code='W-UNREACH-0',severity='warning'))
    ib=copy.deepcopy(ih);ib['bounds']['clock']=0
    pair('instant',instant,'clock 20min','clock 0min',ih,ib,
         'b admits1, holds2,3 (peak2); end1 at0 frees queue; g admits2/end2 then3/end3 at0. All admitted and completed0/work0, even clock0; no fictitious future tick.',fault='clock-zero')
    standing=edit(source(END),'  process Flow', '  actor Party { control party; standing over [end.done] basis "claim" }\n  process Flow')
    pair('standing',standing,'standing over [end.done]','standing over [end.overflowed]',
         dict(verdict='satisfied'),failure('U-SEM-0'),
         'A control party may stand over the regular process end done; changing only that target to overflowed is unsupported, not a normal terminal Standing.',fault='overflow')
    contest=edit(standing,'  process Flow','  actor Route { capacity 1; capability reply() -> T { duration fixed 1min; result nondet { ok } } }\n  process Flow')
    contest=edit(contest,'  verify safety','  contract C { contest Party on end.done route Route.reply reversible_by [] respond_by 1min }\n  verify safety')
    # Correct nonempty executor/authority scaffolding is checked by the old
    # skeleton preflight rather than hidden behind the new limit parser.
    contest=edit(edit(contest,'  type T','  people [p]\n  type T'),'actor A { capacity 1;','actor A { executors [p];')
    contest=edit(contest,'reversible_by []','reversible_by [p]')
    contest=edit(contest,'  process Flow','  authority G { basis "decision"; grants p to A.run }\n  process Flow')
    pair('contest-overflow',contest,'contest Party on end.done','contest Party on end.overflowed',
         dict(verdict='satisfied'),failure('U-SEM-0'),
         'Party has Standing on done and a positive-time route with authorized reversible executor. Target overflowed is refused as unsupported before running a route.',fault='target')


def main():
    import sys
    build()
    support=Path(__file__).parent
    for name,(s,h,calc,inv) in ([] if '--tables-only' in sys.argv else sorted(FIXTURES.items())):
        directory=ROOT/'fixtures'/name
        directory.mkdir(exist_ok=True)
        # Calculation is written before its expectation and input, never learned.
        (directory/'calc.md').write_text('# '+name+'\n\nADR 0039; design v0.2 §22.3. Hand-derived before execution.\n\n'+calc.rstrip()+'\n\nP2 delay, P9 label and K mutations are not counted as single-input defect pairs.\n')
        (directory/'expected.json').write_text(json.dumps(h,indent=2)+'\n')
        (directory/'model.relca').write_text(s)
        if inv:
            (directory/'invocation.json').write_text(json.dumps(inv,indent=2)+'\n')
    (support/'limits_names.json').write_text(json.dumps(sorted(FIXTURES),indent=2)+'\n')
    (support/'limits_names.rs').write_text('&[\n'+''.join('    '+json.dumps(n)+',\n' for n in sorted(FIXTURES))+']\n')
    (support/'limits_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    print(len(FIXTURES),'fixtures;',len(PAIRS),'transformations; no binary invoked')


if __name__=='__main__':main()
