"""ADR0045 hand-oracle materializer. Never imports or executes Relca.

Fraction arithmetic below is the specification calculation, not a snapshot of
production output. Each edit has a witness and every source has no comments.
Run from the repository root with python3 -B followed by this path.
"""
from copy import deepcopy
from fractions import Fraction as F
import difflib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
ROWS = {}
PAIRS = []
BASE = '''model QualityBase version "0.2" {
  type V = enum { pass, fail }
  type Ack = enum { committed }
  observed case { registered: bool = false }
  latent case { correct: bool = false }
  kernel judge { * -> { 0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false } }
  actor Checker { capacity 2; capability check() -> V { duration fixed 3min; outcomes kernel judge; writes correct } }
  actor Ledger { capacity 2; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }
  process Flow { v = Checker.check(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }
  cut C { include [Checker, Ledger] }
  arrivals fixed [0min, 10min]
  bounds { cases 2; live_tasks 2; clock 30min }
  verify reachable end.posted
  property case.Wrong = reached(registered == true && correct == false)
  property case.Registered = reached(registered == true)
  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals
  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))
}
'''
WRONG = 'registered == true && correct == false'
REGISTERED = 'registered == true'
CHECKED = ['case_reachability', 'expected_counts', 'ratio_of_expected_counts',
           'denominator_disclosure', 'outcomes_per_arrival']
UNCHECKED = ['expectation_of_ratio', 'cohort_any_probability', 'metric_intervals',
             'metric_constraints', 'numerator_subset_of_denominator',
             'business_value', 'rework_loss']


def edit(s, old, new, count=1):
    assert s.count(old) >= count, (old, s)
    t = s.replace(old, new, count)
    assert t != s, old
    return t


def append(s, clauses):
    i = s.rfind('}')
    return s[:i] + '  ' + clauses.replace('\n', '\n  ') + '\n' + s[i:]


def no_quality(s):
    return ''.join(l for l in s.splitlines(True)
                   if not l.strip().startswith(('property case.', 'metric cohort.')))


def rng(lo, hi=None, reason=None, basis=None):
    hi = lo if hi is None else hi
    lo, hi = str(F(lo)), str(F(hi))
    return dict(lower=lo, upper=hi,
                basis=basis or ('exact' if lo == hi and reason is None else 'per_case_relaxation'),
                reason=reason)


def absent(reason):
    return dict(lower=None, upper=None, basis=None, reason=reason)


def prop(name, condition, ps, arrivals=(0, 10), within=None, reasons=None):
    reasons = reasons or [None] * len(ps)
    cases = []
    lo = hi = F(0)
    for i, (p, at, why) in enumerate(zip(ps, arrivals, reasons)):
        a, b = p if isinstance(p, tuple) else (p, p)
        a, b = F(a), F(b)
        lo += a
        hi += b
        cases.append(dict(case_id=i+1, arrival=at, min=str(a), max=str(b), reason=why))
    reason = 'truncated' if 'truncated' in reasons else ('undetermined' if 'undetermined' in reasons else None)
    return dict(name=name, condition=condition, within=within, cases=cases,
                expected_count=rng(lo, hi, reason))


def ratio(name, numerator, denominator, properties, arrivals):
    p = next(p for p in properties if p['name'] == numerator)
    n = p['expected_count']
    if denominator is None:
        den = dict(kind='arrivals', count=arrivals)
        d = rng(arrivals)
    else:
        q = next(p for p in properties if p['name'] == denominator)
        ends = q['condition'].split(' || ') if q['condition'].startswith('end.') else None
        den = dict(kind='property', property=denominator,
                   covers=[e[4:] for e in ends] if ends else None,
                   reason=None if ends else 'state_condition')
        d = q['expected_count']
    why = ('truncated' if 'truncated' in (n['reason'], d['reason']) else
           'undetermined' if 'undetermined' in (n['reason'], d['reason']) else None)
    nl, nu, dl, du = (F(n['lower']), F(n['upper']), F(d['lower']), F(d['upper']))
    bounds = None
    if why:
        value = absent(why)
        if dl > 0:
            bounds = dict(lower=str(nl/du), upper=str(nu/dl))
    elif du == 0:
        value = absent('zero_denominator')
    elif dl == 0:
        value = absent('denominator_may_be_zero')
    else:
        basis = 'exact' if n['basis'] == d['basis'] == 'exact' else 'per_case_relaxation'
        value = rng(nl/du, nu/dl, basis=basis)
    return dict(name=name, form='ratio_of_expected_counts', numerator=numerator,
                denominator=den, value=value, bounds_with_undetermined=bounds)


def outcome(kind, end, count, n=2):
    c = count if isinstance(count, dict) else rng(count)
    rate = absent(c['reason']) if c['reason'] else rng(F(c['lower'])/n, F(c['upper'])/n, basis=c['basis'])
    return dict(kind=kind, end=end, expected_count=c, rate=rate)


def comparison_rows(left, right, comparing):
    def flatten(q):
        rows=[]
        for p in q['properties']:
            rows.append((dict(scope='property',subject=p['name'],end=None,field='expected_count'),None,p['expected_count']))
            if q['analysis']=='simulation': rows.append((dict(scope='property',subject=p['name'],end=None,field='unknown_count'),None,p['unknown_count']))
        for m in q['cohort']:
            for field in ['value','bounds_with_undetermined']:
                rows.append((dict(scope='cohort',subject=m['name'],end=None,field=field),m['denominator'],m[field]))
        for o in q['outcomes_per_arrival']:
            for field in ['expected_count','rate']:
                rows.append((dict(scope='outcome',subject=o['kind'],end=o['end'],field=field),None,o[field]))
        return rows
    result=[]
    for (key,den,a),(other,other_den,b) in zip(flatten(left),flatten(right)):
        assert key==other and den==other_den
        row=dict(key=key)
        if den is not None: row['denominator']=den
        if comparing:
            row['values']=[dict(configuration='Check',status='reported',value=a),dict(configuration='Fast',status='reported',value=b)]
        else:
            row.update({'from':dict(status='reported',value=a),'to':dict(status='reported',value=b)})
        result.append(row)
    return result


def quality(w='1/50', r='23/25', arrivals=(0, 10), ends=None, reasons=None):
    n = len(arrivals)
    ws = w if isinstance(w, list) else [w] * n
    rs = r if isinstance(r, list) else [r] * n
    ps = [prop('Wrong', WRONG, ws, arrivals, reasons=reasons),
          prop('Registered', REGISTERED, rs, arrivals, reasons=reasons)]
    if ends is None:
        ends = [('end', 'returned', F(2)*F('2/25')), ('end', 'posted', F(2)*F('23/25'))]
    os = [outcome(k, e, c, n) for k, e, c in ends]
    kinds = [k for k, _, _ in ends]
    for kind in ['rejected', 'unfinished', 'deadlock']:
        if kind not in kinds:
            os.append(outcome(kind, None, 0, n))
    os.sort(key=lambda o: ['end', 'overflow', 'rejected', 'unfinished', 'deadlock', 'cut_off'].index(o['kind']))
    return dict(rules='quality-cohort-p1-1', analysis='exhaustive', properties=ps,
                cohort=[ratio('WrongPerArrival', 'Wrong', None, ps, n),
                        ratio('WrongPerRegistered', 'Wrong', 'Registered', ps, n)],
                outcomes_per_arrival=os, checked=CHECKED.copy(),
                unchecked=UNCHECKED + ['deadline_attainment'])


def add_property(q, name, condition, ps, arrivals, within=None, metric=None, denominator=None, reasons=None):
    q['properties'].append(prop(name, condition, ps, arrivals, within, reasons))
    if metric:
        q['cohort'].append(ratio(metric, name, denominator, q['properties'], len(arrivals)))
        if within is not None:
            q['unchecked'] = [x for x in q['unchecked'] if x != 'deadline_attainment']
            if 'deadline_attainment' not in q['checked']:
                q['checked'].append('deadline_attainment')


def sample(x, n=4):
    return dict(mean=str(F(x)), min=str(F(x)), max=str(F(x)), samples=n, reason=None)


def simulation(q, hits, unknown, reason=None, aborted=False):
    q = deepcopy(q)
    q['analysis'] = 'simulation'
    n = len(q['properties'][0]['cases'])
    for p, hs, us in zip(q['properties'], hits, unknown):
        for c, h, u in zip(p['cases'], hs, us):
            c.pop('min'); c.pop('max'); c.pop('reason')
            c.update(hits=4*h, unknown=4*u, trials=4)
        p['expected_count'] = sample(sum(hs))
        p['unknown_count'] = sample(sum(us))
    for m in q['cohort']:
        p = next(p for p in q['properties'] if p['name'] == m['numerator'])
        num = p['expected_count']
        d = m['denominator']
        den = sample(n) if d['kind'] == 'arrivals' else next(p for p in q['properties'] if p['name'] == d['property'])['expected_count']
        old = m['value']
        m['value'] = dict(point=old['lower'] if old['lower'] == old['upper'] else None,
                          numerator=num, denominator=den, samples=4, reason=old['reason'])
    for o in q['outcomes_per_arrival']:
        c = o['expected_count']
        assert c['lower'] == c['upper'], 'simulation uses observed outcome counts'
        o['expected_count'] = sample(c['lower'])
        o['rate'] = dict(point=str(F(c['lower'])/n), samples=4, reason=None)
    if aborted:
        empty = dict(mean=None, min=None, max=None, samples=0, reason='aborted')
        for p in q['properties']:
            for c in p['cases']:
                c.update(hits=0, unknown=0, trials=0)
            p['expected_count'] = deepcopy(empty)
            p['unknown_count'] = deepcopy(empty)
        for m in q['cohort']:
            m['value'] = dict(point=None, numerator=deepcopy(empty), denominator=deepcopy(empty), samples=0, reason='aborted')
            m['bounds_with_undetermined'] = None
        for o in q['outcomes_per_arrival']:
            o['expected_count'] = deepcopy(empty)
            o['rate'] = dict(point=None, samples=0, reason='aborted')
        q['unchecked'] = q['checked'] + q['unchecked']
        q['checked'] = []
    return q


def save(name, source, q=None, verdict='satisfied', invocation=None, code=None, calc='', deferred=None):
    assert name not in ROWS, name
    assert '//' not in source and '#' not in source
    expected = dict(verdict=verdict)
    if q is not None:
        expected['quality'] = q
    if code:
        expected['diagnostics'] = [dict(code=code, severity='error')]
    ROWS[name] = dict(source=source, expected=expected, invocation=invocation,
                      code=code, calc=calc, deferred=deferred)


def pair(name, source, changed, q, bq, calc, verdict='satisfied', broken_verdict=None,
         fault='changed', invocation=None, code=None, deferred=None):
    assert source != changed
    save(name, source, q, verdict, invocation, calc=calc, deferred=deferred)
    bn = name + '.broken-' + fault
    save(bn, changed, bq, broken_verdict or verdict, invocation, code, calc, deferred)
    PAIRS.append(dict(base=name, broken=bn, fault=fault, calculation=calc))


def main():
    q = quality()
    nc = edit(BASE, '0.02: pass with correct = false; 0.08: fail with correct = false', '0.1: pass with correct = false')
    nq = quality('1/10', 1, ends=[('end','returned',0),('end','posted',2)])
    pair('qual-base', BASE, nc, q, nq,
         'T02: P(W)=0.02=1/50; P(R)=0.90+0.02=23/25. Two arrivals give 1/25 and 46/25; ratios 1/50 and 1/46. No-check gives W=1/10,R=1; ratios 1/10. Quality never changes reachable verdict.', fault='no-check')
    pair('qual-latent-policy', BASE, edit(BASE,'if v == pass','if correct == true'), q, None,
         'T03: Same hand oracle as T02. Property is evaluation, but process reads latent correct: E-POL-LATENT, invalid, no quality.', fault='latent', broken_verdict='invalid', code='E-POL-LATENT')
    zero = edit(edit(BASE,'0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false','1: fail with correct = false'),'reachable end.posted','reachable end.returned')
    half = edit(zero,'1: fail with correct = false','0.5: pass with correct = false; 0.5: fail with correct = false')
    pair('qual-zero-denominator',zero,half,quality(0,0,ends=[('end','returned',2),('end','posted',0)]),quality('1/2','1/2',ends=[('end','returned',1),('end','posted',1)]),
         'T04: 0/2=0 but 0/0 is null zero_denominator. Half version gives N=D=1, N/2=1/2,N/D=1.',fault='half')
    cont = edit(edit(BASE,'actor Checker { capacity 2','actor Checker { capacity 1'),'[0min, 10min]','[0min, 0min]')
    fifo = append(cont,'policy fifo tie_by case_id')
    clause='property case.OnTime = reached(end.posted || end.returned) within 4min\nmetric cohort.OnTimePerArrival = expected(count(case.OnTime)) / arrivals'
    cq = quality(arrivals=(0,0)); bq = deepcopy(cq)
    add_property(cq,'OnTime','end.posted || end.returned',[1,0],(0,0),4,'OnTimePerArrival')
    add_property(bq,'OnTime','end.posted || end.returned',[(0,1),(0,1)],(0,0),4,'OnTimePerArrival')
    pair('qual-contention',append(fifo,clause),append(cont,clause),cq,bq,
         'T05: FIFO first case ends at3 or4 (inclusive), second at6 or7. OnTime=[1,0], expected1/rate1/2 exact. Without policy each case [0,1], sum[0,2]/2=[0,1] per_case_relaxation, not unknown. Joint count is1 but is not claimed by per-case relaxation.',fault='no-policy')
    rejected=append(fifo,'queue capacity 1 on_overflow reject')
    rq=quality(['1/50',0],['23/25',0],(0,0),[('end','returned','2/25'),('end','posted','23/25'),('rejected',None,1)])
    allq=quality(0,0,(0,0),[('end','returned',0),('end','posted',0),('rejected',None,2)])
    pair('qual-rejected',rejected,edit(rejected,'queue capacity 1','queue capacity 0'),rq,allq,
         'T07: case2 is rejected; N=1/50,D_R=23/25, arrivals2. A=1/100,B=1/46; rejected1/2, posted23/50, returned1/25. All-reject keeps arrivals2,A=0,B=null zero_denominator, rejected1. Reachable posted fails for rejected case.',verdict='violated',fault='reject-all')
    dl=edit(fifo,'actor Checker { capacity 1','actor Checker { capacity 0')
    dq=quality(0,0,(0,0),[('end','returned',0),('end','posted',0),('deadlock',None,2)])
    pair('qual-deadlock',rejected,dl,rq,dq,
         'T08 ADR pair: remove queue and set Checker capacity0 (one business concept: no progress). See qual-deadlock-single for isolated capacity fault. Closed deadlock gives misses0 not unknown; deadlock2/2=1 and arrivals remains2.',verdict='violated',fault='zero-capacity')
    pair('qual-deadlock-single',fifo,dl,quality(arrivals=(0,0)),dq,
         'T08 isolated capacity-only control: capacity1 completes both; capacity0 cannot start, deadlock1. A remains0 rather than removing deadlocked cases from denominator. W-DEADLOCK-0, violated.',fault='zero-capacity',broken_verdict='violated')
    det=edit(rejected,'0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false','1: pass with correct = false')
    sim=append(det,'simulation replications 4 seed 1 confidence 0.5')
    sq=quality([1,0],[1,0],(0,0),[('end','returned',0),('end','posted',1),('rejected',None,1)])
    sfull=quality(1,1,ends=[('end','returned',0),('end','posted',2)])
    pair('qual-sim',sim,edit(sim,'[0min, 0min]','[0min, 10min]'),simulation(sq,[[1,0],[1,0]],[[0,0],[0,0]]),simulation(sfull,[[1,1],[1,1]],[[0,0],[0,0]]),
         'T09: four deterministic repetitions. Every repetition N=1,D_A=2,D_R=1: point1/2 and1; samples4 (not8). Spaced arrivals N=D_R=2: both points1; case hits4 each. Original reachable lacks a witness for rejected case: simulation inconclusive, exhaustive violated.',verdict='inconclusive',broken_verdict='satisfied',fault='spaced',invocation={'command':'simulate'})
    pair('qual-sim-check',det,edit(det,'[0min, 0min]','[0min, 10min]'),sq,sfull,
         'T09 exhaustive counterpart: N=1,D_R=1,arrivals2 then N=D_R=2. Ratios1/2,1 then1,1 exact. Rejected case makes first reachable violated.',verdict='violated',broken_verdict='satisfied',fault='spaced')
    label=BASE
    for a,b in [('WrongPerRegistered','ARate'),('WrongPerArrival','ZRate'),('Registered','Zebra'),('Wrong','Aardvark')]: label=edit(label,a,b, label.count(a))
    lq=deepcopy(q)
    lq['properties'][0]['name']='Aardvark'; lq['properties'][1]['name']='Zebra'
    for m, name in zip(lq['cohort'],['ZRate','ARate']):
        m['name']=name; m['numerator']='Aardvark'
        if m['denominator']['kind']=='property': m['denominator']['property']='Zebra'
    pair('qual-label',BASE,label,q,lq,'T12 meaning-preserving renaming reverses dictionary order, not declaration order. First property has1/25, second46/25; ZRate1/50,ARate1/46. This is a label control, not an input defect.',fault='renamed')
    actorlabel=edit(edit(BASE,'Checker','Desk',BASE.count('Checker')),'actor Desk {','actor Desk { grain team;')
    pair('qual-actor-label',BASE,actorlabel,q,q,'T12 independent Actor/grain label control. Neither Desk nor grain team supplies quality semantics. Only exact Actor metadata/name/span positions may differ.',fault='team')
    metrics='metrics { window 30min; currency "JPY"; rate Checker per_minute 1; rate Ledger per_minute 1; rate Checker.check per_start 0; rate Ledger.post per_start 0 }'
    legacy=append(no_quality(BASE),metrics)
    legacy=append(legacy,'\n'.join(l.strip() for l in BASE.splitlines() if l.strip().startswith(('property case.','metric cohort.'))))
    pair('qual-legacy',legacy,no_quality(legacy),q,None,'T13 quality appended at end: old spans do not move. Remove only model.sha256, quality and quality.* assumptions for this explicit pair; all old fields, metrics.unchecked quality, RNG keys and census remain identical. Never apply projection to historical manifest.',fault='no-quality')
    resolved=edit(edit(BASE,'enum { committed }','enum { committed, duplicate }'),'result nondet { committed }','outcomes kernel ledger')
    resolved=append(resolved,'kernel ledger { * -> { 0.95: committed; 0.05: duplicate } }')
    resolved=edit(resolved,'a = Ledger.post(); end posted','a = Ledger.post(); if a == committed { end posted } else { end exception }')
    resolved=append(resolved,'property case.Resolved = reached(end.posted || end.returned)\nmetric cohort.ResolvedPerArrival = expected(count(case.Resolved)) / arrivals')
    resq=quality(ends=[('end','returned','4/25'),('end','posted','437/250'),('end','exception','23/250')])
    add_property(resq,'Resolved','end.posted || end.returned',['477/500']*2,(0,10),metric='ResolvedPerArrival')
    pair('qual-resolved',resolved,edit(resolved,'reached(end.posted || end.returned)','reached(end.posted || registered == true)'),resq,None,
         'T14: posted=(23/25)(19/20)=437/500,returned40/500,exception23/500. Resolved477/500,E=477/250. Finish sets registered even on duplicate: Wrong/Registered stay1/50,23/25. Mixed End/state union unsupported U-SYN-0.',fault='mixed',broken_verdict='unsupported',code='U-SYN-0')
    covers=append(resolved,'metric cohort.WrongPerResolved = expected(count(case.Wrong)) / expected(count(case.Resolved))')
    covq=deepcopy(resq); covq['cohort'].append(ratio('WrongPerResolved','Wrong','Resolved',covq['properties'],2))
    covb=deepcopy(resq); covb['cohort'].append(ratio('WrongPerResolved','Wrong','Registered',covb['properties'],2))
    pair('qual-covers',covers,edit(covers,'metric cohort.WrongPerResolved = expected(count(case.Wrong)) / expected(count(case.Resolved))','metric cohort.WrongPerResolved = expected(count(case.Wrong)) / expected(count(case.Registered))'),covq,covb,
         'T19: N=1/25,D_Resolved=477/250,ratio10/477,covers[posted,returned]. State denominator D=46/25,ratio1/46,covers null/reason state_condition. No subset proof is implied.',fault='state-denominator')
    comp=edit(BASE,'capability check() -> V','capability quick() -> V { duration fixed 1min; outcomes kernel lenient; writes correct }; capability check() -> V')
    comp=append(comp,'kernel lenient { * -> { 0.9: pass with correct = true; 0.1: pass with correct = false } }\nprocess Quick { v = Checker.quick(); if v == pass {} else { end returned }; a = Ledger.post(); end posted }\nconfiguration Check { cut C; process Flow }\nconfiguration Fast { cut C; process Quick }')
    other=append(edit(comp,'configuration Fast { cut C;','configuration Fast { cut C2;'),'cut C2 { include [Checker, Ledger] }')
    for name, inv in [('qual-compare',{'command':'compare','plans':['Check','Fast'],'mode':'exhaustive'}),('qual-diff',{'command':'diff','from':'Check','to':'Fast'})]:
        pair(name,comp,other,None,None,'T11/T20: Check A=1/50,B=1/46; Fast A=B=1/10. Fixed property/cohort/outcome rows, no difference/rank/verdict cells. Distinct Cut name C2 invalid E-CMP-0, quality table empty.',fault='other-cut',invocation=inv,broken_verdict='invalid',code='E-CMP-0')
        ROWS[name]['plans']={'Check':q,'Fast':nq}
        rows=comparison_rows(q,nq,name=='qual-compare')
        if name=='qual-compare':
            ROWS[name]['expected'].update(plans=[dict(configuration='Check',report=dict(quality=q)),dict(configuration='Fast',report=dict(quality=nq))],tables=dict(quality=rows))
            ROWS[name+'.broken-other-cut']['expected']['tables']=dict(quality=[])
        else:
            ROWS[name]['expected'].update({'from':dict(report=dict(quality=q)),'to':dict(report=dict(quality=nq)),'quality':rows})
            ROWS[name+'.broken-other-cut']['expected']['quality']=[]
    cs=edit(edit(comp,'0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false','1: pass with correct = true'),'0.9: pass with correct = true; 0.1: pass with correct = false','1: pass with correct = false')
    cs=append(cs,'simulation replications 4 seed 1 confidence 0.5')
    csb=append(edit(cs,'configuration Fast { cut C;','configuration Fast { cut C2;'),'cut C2 { include [Checker, Ledger] }')
    csq=simulation(quality(0,1,ends=[('end','returned',0),('end','posted',2)]),[[0,0],[1,1]],[[0,0],[0,0]])
    csf=simulation(quality(1,1,ends=[('end','returned',0),('end','posted',2)]),[[1,1],[1,1]],[[0,0],[0,0]])
    pair('qual-compare-sim',cs,csb,None,None,'T11 simulation transport supplement: Check correct=true always,N_W0;Fast correct=false always,N_W2. Four replications each,arrivals2/replication. Check A=B0,Fast A=B1;unknown samples0. Closed table also includes unknown_count rows,with4 samples not8.',fault='other-cut',invocation={'command':'compare','plans':['Check','Fast'],'mode':'simulation'},broken_verdict='invalid',code='E-CMP-0')
    ROWS['qual-compare-sim']['plans']={'Check':csq,'Fast':csf}
    ROWS['qual-compare-sim']['expected'].update(plans=[dict(configuration='Check',report=dict(quality=csq)),dict(configuration='Fast',report=dict(quality=csf))],tables=dict(quality=comparison_rows(csq,csf,True)))
    ROWS['qual-compare-sim.broken-other-cut']['expected']['tables']=dict(quality=[])
    pair('qual-explain',BASE,no_quality(BASE),q,None,'T20: explain is configuration-free. Quality transported in root report only, never made a counterexample target. Explicit no-quality pair preserves old report after hash/quality assumptions projection.',fault='no-quality',invocation={'command':'explain'})
    clock=edit(edit(edit(fifo,'duration fixed 3min','duration fixed 4min'),'actor Ledger { capacity 2','actor Ledger { capacity 1'),'[0min, 0min]','[0min, 6min, 12min]')
    clock=edit(clock,'cases 2; live_tasks 2; clock 30min','cases 3; live_tasks 2; clock 14min')
    why=[None,None,'truncated']
    ends=[('end','returned',rng('4/25','29/25','truncated')),('end','posted',rng('46/25','71/25','truncated')),('unfinished',None,rng(0,1,'truncated')),('deadlock',None,rng(0,1,'truncated'))]
    clq=quality(['1/50','1/50',(0,1)],['23/25','23/25',(0,1)],(0,6,12),ends,why)
    closed=edit(clock,'clock 14min','clock 30min')
    full3=quality(arrivals=(0,6,12),ends=[('end','returned','6/25'),('end','posted','69/25')])
    pair('qual-clock',closed,clock,full3,clq,'T06 clock control: third case starts12, judge would finish16,cut14. N=[1/25,26/25],D_R=[46/25,71/25]. A null truncated with[1/75,26/75]; B null truncated with[1/71,13/23]. Outcomes retain known first two and conservative third, never assert a final unfinished probability1/3.',fault='short-clock',broken_verdict='inconclusive')
    for suffix, deadline in [('',1),('-edge',2)]:
        eq=deepcopy(clq); eb=deepcopy(clq)
        add_property(eq,'Early','registered == false',[1]*3,(0,6,12),deadline,'EarlyPerArrival')
        add_property(eb,'Early','registered == true',[0]*3,(0,6,12),deadline,'EarlyPerArrival')
        early=append(clock,f'property case.Early = reached(registered == false) within {deadline}min\nmetric cohort.EarlyPerArrival = expected(count(case.Early)) / arrivals')
        pair('qual-closed-within'+suffix,early,edit(early,'reached(registered == false)','reached(registered == true)'),eq,eb,
             f'T16: arrival false hit gives1 even with cut; earliest post arrival+5. Within{deadline} closes by clock14 for all3, so true version0, not unknown. Nonzero1 and zero0 coexist with Wrong null truncated.',verdict='inconclusive',fault='true')
    # Simulation clock: observed terminal counts, unlike conservative exhaustive futures.
    sc=append(edit(clock,'0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false','1: pass with correct = true'),'simulation replications 4 seed 1 confidence 0.5')
    scbase=quality([0,0,(0,1)],[1,1,(0,1)],(0,6,12),[('end','returned',0),('end','posted',2),('unfinished',None,1)],why)
    scq=simulation(scbase,[[0,0,0],[1,1,0]],[[0,0,1],[0,0,1]])
    scfull=simulation(quality(0,1,(0,6,12),[('end','returned',0),('end','posted',3)]),[[0,0,0],[1,1,1]],[[0,0,0],[0,0,0]])
    pair('qual-sim-clock',edit(sc,'clock 14min','clock 30min'),sc,scfull,scq,
         'T10 clock: hits W0,0,0;R4,4,0;unknown0,0,4 each. Four replications, N_W=0,U_W=4,D_A=12,N_R=8,U_R=4. A null truncated bounds[0,1/3], B[0,1/2]. Samples4 not12. Observed unfinished1/3 does not claim eventual outcome.',fault='short-clock',invocation={'command':'simulate'},verdict='satisfied',broken_verdict='inconclusive')
    seq=deepcopy(scbase); seb=deepcopy(scbase)
    add_property(seq,'Early','registered == false',[1]*3,(0,6,12),1,'EarlyPerArrival')
    add_property(seb,'Early','registered == true',[0]*3,(0,6,12),1,'EarlyPerArrival')
    es=append(sc,'property case.Early = reached(registered == false) within 1min\nmetric cohort.EarlyPerArrival = expected(count(case.Early)) / arrivals')
    pair('qual-closed-within-sim',es,edit(es,'reached(registered == false)','reached(registered == true)'),simulation(seq,[[0,0,0],[1,1,0],[1,1,1]],[[0,0,1],[0,0,1],[0,0,0]]),simulation(seb,[[0,0,0],[1,1,0],[0,0,0]],[[0,0,1],[0,0,1],[0,0,0]]),
         'T16 simulation: Early false hits4 for all3 and no unknown, rate1; true hits0,unknown0,rate0. Other properties retain third-case unknown4. Do not blank all quality for one cut.',fault='true',verdict='inconclusive',invocation={'command':'simulate'})
    one='''model MaxOnly version "0.2" {
  type Choice = enum { short, long }
  type Ack = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1
    capability choose() -> Choice { duration fixed 1min; result nondet { short, long } }
    capability mark() -> Ack { duration fixed 5min; result nondet { ok }; writes hit; finish { hit = true } }
  }
  process P { c = A.choose(); if c == short { end miss } else { a = A.mark(); end done } }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
  property case.Hit = reached(hit == true)
  metric cohort.Rate = expected(count(case.Hit)) / arrivals
}
'''
    def single(p, outcomes, metric='Rate', denominator=None, properties=None):
        ps=properties or [p]
        return dict(rules='quality-cohort-p1-1',analysis='exhaustive',properties=ps,cohort=[ratio(metric,p['name'],denominator,ps,1)],outcomes_per_arrival=[outcome(k,e,c,1) for k,e,c in outcomes],checked=CHECKED.copy(),unchecked=UNCHECKED+['deadline_attainment'])
    zero_out=[('rejected',None,0),('unfinished',None,0),('deadlock',None,0)]
    oneq=single(prop('Hit','hit == true',[(0,1)],(0,)),[('end','miss',rng(0,1)),('end','done',rng(0,1))]+zero_out)
    oneb=single(prop('Hit','hit == true',[(0,1)],(0,),reasons=['truncated']),[('end','miss',rng(0,1,'truncated')),('end','done',rng(0,1,'truncated')),('rejected',None,0),('unfinished',None,rng(0,1,'truncated')),('deadlock',None,rng(0,1,'truncated'))])
    pair('qual-max-only-unknown',one,edit(one,'clock 10min','clock 3min'),oneq,oneb,
         'T17: normal root actions miss0 or hit1 give b=[0,0,1,1],known schedule width, rate[0,1]. Clock3 changes only long hit suffix to unknown: b=[0,0,0,1],max-only unknown. Rate null truncated,bounds[0,1]. Inspect both min and max endpoint pairs.',fault='short-clock',broken_verdict='inconclusive')
    minonly=edit(one,'capability choose() -> Choice','capability fast() -> Ack { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } }; capability choose() -> Choice')
    minonly=edit(minonly,'if c == short { end miss }','if c == short { f = A.fast(); end miss }')
    minq=deepcopy(oneq); minq['properties'][0]=prop('Hit','hit == true',[1],(0,)); minq['cohort'][0]=ratio('Rate','Hit',None,minq['properties'],1)
    pair('qual-min-only-unknown',minonly,edit(minonly,'clock 10min','clock 3min'),minq,oneb,
         'T17 symmetric endpoint control: normal both branches hit1; clock3 leaves fast branch hit1 and long branch unknown. b=[0,1,1,1],min-only unknown; rate null truncated,bounds[0,1]. Do not inspect only max endpoints.',fault='short-clock',broken_verdict='inconclusive')
    union='''model Union version "0.2" {
  type Choice = enum { left, right }
  actor A { capacity 1; capability choose() -> Choice { duration fixed 1min; result nondet { left, right } } }
  process P { c = A.choose(); if c == left { end a } else { end b } }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.a
  property case.P = reached(end.a || end.b)
  metric cohort.Rate = expected(count(case.P)) / arrivals
}
'''
    outs=[('end','a',rng(0,1)),('end','b',rng(0,1))]+zero_out
    uq=single(prop('P','end.a || end.b',[1],(0,)),outs)
    ub=single(prop('P','end.a',[(0,1)],(0,)),outs)
    pair('qual-union-nondet',union,edit(union,'reached(end.a || end.b)','reached(end.a)'),uq,ub,
         'T18: every schedule reaches exactly one of a,b. Solve union once:1 exact, not min(a)+min(b)=0 or max sum2. Single End has known scheduler range[0,1],not undetermined.',fault='single-end')
    may=append(edit(union,'property case.P = reached(end.a || end.b)','property case.P = reached(end.a)'), 'property case.All = reached(end.a || end.b)\nmetric cohort.AllPerP = expected(count(case.All)) / expected(count(case.P))')
    mq=deepcopy(ub); mq['properties'].append(prop('All','end.a || end.b',[1],(0,))); mq['cohort'].append(ratio('AllPerP','All','P',mq['properties'],1))
    mb=deepcopy(mq); mb['cohort'][1]=ratio('AllPerP','All','All',mb['properties'],1)
    pair('qual-may-zero',may,edit(may,'metric cohort.AllPerP = expected(count(case.All)) / expected(count(case.P))','metric cohort.AllPerP = expected(count(case.All)) / expected(count(case.All))'),mq,mb,
         'T19: All=1,P=[0,1] known nondeterminism. All/P null denominator_may_be_zero,not zero_denominator; no unknown bounds. All/All=1 exact. P/P would not be algebraically cancelled under relaxation.',fault='all-denominator')
    coin=append(edit(union,'result nondet { left, right }','outcomes kernel coin'),'kernel coin { * -> { 0.5: left; 0.5: right } }\nproperty case.Half = reached(end.a)\nmetric cohort.MoreThanOne = expected(count(case.P)) / expected(count(case.Half))')
    cq2=single(prop('P','end.a || end.b',[1],(0,)),[('end','a','1/2'),('end','b','1/2')]+zero_out)
    cq2['properties'].append(prop('Half','end.a',['1/2'],(0,))); cq2['cohort'].append(ratio('MoreThanOne','P','Half',cq2['properties'],1))
    cb2=deepcopy(cq2); cb2['cohort'][1]=ratio('MoreThanOne','P','P',cb2['properties'],1)
    pair('qual-above-one',coin,edit(coin,'metric cohort.MoreThanOne = expected(count(case.P)) / expected(count(case.Half))','metric cohort.MoreThanOne = expected(count(case.P)) / expected(count(case.P))'),cq2,cb2,
         'T19: numerator1,denominator1/2 gives2 exact. No numerator-subset check or clamp to probability[0,1]. Change denominator to1 gives1.',fault='unit-denominator')
    plain='''model Legacy version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability c() -> T { duration fixed 1min; result nondet { ok } } }
  process P { a = A.c(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  property case.Done = reached(end.done)
  metric cohort.Rate = expected(count(case.Done)) / arrivals
}
'''
    plainq=single(prop('Done','end.done',[1],(0,)),[('end','done',1)]+zero_out)
    pair('qual-legacy-route',plain,append(plain,'arrivals fixed [0min]'),plainq,plainq,
         'T22.1: one fixed1min operation,Done1,rate1. No arrivals uses legacy_route and uses_probability=false; adding arrivals[0] uses new route,quality same. Original terminals must retain absence of probability.',fault='arrivals')
    person=append(plain,'people [p,q]\nobserved case { owner: optional<PersonId> }\nproperty case.Owned = reached(owner == p)\nmetric cohort.OwnedPerArrival = expected(count(case.Owned)) / arrivals')
    person=edit(edit(person,'actor A { capacity 1','actor A { executors [p]'),'result nondet { ok }','result nondet { ok }; writes owner; finish { owner = executor.id }')
    pq2=deepcopy(plainq); add_property(pq2,'Owned','owner == p',[1],(0,),metric='OwnedPerArrival')
    pb2=deepcopy(plainq); add_property(pb2,'Owned','owner == p',[0],(0,),metric='OwnedPerArrival')
    pair('qual-person',person,edit(person,'executors [p]','executors [q]'),pq2,pb2,
         'T22.5: named executor p writes owner at1,quality owner==p reaches1. Sole executor q writes q instead:0. Verify reachable done stays satisfied; PersonId identity is not Actor name.',fault='other-executor')
    actorstate=append(plain,'actor Observer { state flag: bool = true }\nproperty case.Flag = reached(Observer.flag == true)\nmetric cohort.FlagPerArrival = expected(count(case.Flag)) / arrivals')
    actorstate=edit(actorstate,'include [A]','include [A, Observer]')
    aq=deepcopy(plainq); add_property(aq,'Flag','Observer.flag == true',[1],(0,),metric='FlagPerArrival')
    cutstate=edit(actorstate,'include [A, Observer]','include [A]; exclude Observer reason "outside" loses "flag observation"')
    pair('qual-cut-state',actorstate,cutstate,aq,None,'T15 Cut: property may read included Actor observed flag,probability1. Excluding Observer while property alone reads it must raise E-CUT-1,invalid. Quality reads do not become Agency business reads.',fault='excluded',broken_verdict='invalid',code='E-CUT-1')
    fork=edit(edit(plain,'actor A { capacity 1','actor A { capacity 2'),'a = A.c(); end done','fork jobs { a = A.c(); b = A.c() }; join all jobs; end done')
    fork=edit(fork,'live_tasks 1','live_tasks 2')
    fqb=single(prop('Done','end.done',[(0,1)],(0,),reasons=['truncated']),[('end','done',rng(0,1,'truncated')),('rejected',None,0),('unfinished',None,rng(0,1,'truncated')),('deadlock',None,rng(0,1,'truncated'))])
    pair('qual-live-tasks',fork,edit(fork,'live_tasks 2','live_tasks 1'),plainq,fqb,
         'T22.2: two sibling tasks with legal live_tasks2 complete. Limit1 cuts before completion: b=[0,1,0,1],ratio null truncated,bounds[0,1]. No uniform scheduler or Horizon classification.',fault='one-task',broken_verdict='inconclusive')
    overflow=edit(rejected,'queue capacity 1 on_overflow reject','queue capacity 1 on_overflow end spill')
    overflow=edit(overflow,'verify reachable end.posted','verify safety no_resource_overcommit')
    overflow=append(overflow,'property case.Spill = reached(end.spill)\nmetric cohort.SpillPerArrival = expected(count(case.Spill)) / arrivals')
    oq=quality(['1/50',0],['23/25',0],(0,0),[('end','returned','2/25'),('end','posted','23/25'),('overflow','spill',1)])
    add_property(oq,'Spill','end.spill',[0,1],(0,0),metric='SpillPerArrival')
    ob=quality(0,0,(0,0),[('end','returned',0),('end','posted',0),('overflow','spill',2)])
    add_property(ob,'Spill','end.spill',[1,1],(0,0),metric='SpillPerArrival')
    pair('qual-overflow',overflow,edit(overflow,'queue capacity 1','queue capacity 0'),oq,ob,
         'T22.4: case2 overflow spill,rate1/2; rejected0,A1/100. Queue0 spills both,rate1. Spill is not duplicated as ordinary end. Safety still satisfied; overflow not computational truncation.',fault='spill-all')
    # Invoice uses the exact existing substrate but changes only AsIsFlow's first call.
    invoice=(ROOT/'fixtures/prob-invoice/model.relca').read_text()
    invoice=append(invoice,'property case.Wrong = reached(registered == true && extraction_correct == false)\nproperty case.Registered = reached(registered == true)\nmetric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals\nmetric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))')
    iq=quality('93/125000','122593/125000',(0,),[('end','returned','2407/125000'),('end','posted','122593/125000')])
    ib=quality('231/125000','119481/125000',(0,),[('end','returned','5519/125000'),('end','posted','119481/125000')])
    for v in [iq,ib]: v['properties'][0]['condition']='registered == true && extraction_correct == false'
    pair('qual-invoice',invoice,edit(invoice,'entry = Clerk.enter()','entry = Extractor.extract()'),iq,ib,
         'T01: human wrong routine=(4/5)(1/100)(1/5)(3/20)(1/10)=3/125000; difficult=(1/5)(3/50)(1/2)(2/5)(3/10)=90/125000;total93/125000. Correct49/50 gives registered122593/125000. AI errors6+225=231/125000;correct119250/125000,registered119481/125000,ratio77/39827. Old verify threshold0.001 means good satisfied,bad violated. Only AsIs first entry call changes.',fault='ai-transfer',broken_verdict='violated',invocation={'command':'check','configuration':'AsIs'})
    # Deliberately unsupported cases are separate single-fault files, not a bag of errors.
    static=[]
    def fault(label,old,new,code='U-SYN-0',verdict='unsupported'):
        static.append((label,edit(BASE,old,new),code,verdict))
    metric='expected(count(case.Wrong)) / arrivals'
    for label,form in [
        ('expectation-of-ratio','expected(count(case.Wrong) / count(case.Registered))'),
        ('quantile','quantile(count(case.Wrong))'),('p95','p95(count(case.Wrong))'),('median','median(count(case.Wrong))'),
        ('number-denominator','expected(count(case.Wrong)) / 100'),('multiply',metric+' * 2'),('double-divide',metric+' / arrivals'),
        ('bare-count','count(case.Wrong) / arrivals'),('admitted','expected(count(case.Wrong)) / admitted'),
        ('received','expected(count(case.Wrong)) / received'),('completed','expected(count(case.Wrong)) / completed'),
        ('business-value','business_value per_case 1'),('loss','loss per_error 1')]: fault(label,metric,form)
    fault('cohort-property','property case.Wrong','property cohort.Any')
    for label,req in [('cohort-probability','verify probability cohort reached registered == true <= 1'),('verify-union','verify probability reached end.posted || end.returned >= 1'),('hard','hard { WrongPerArrival <= 1 }')]: fault(label,'verify reachable end.posted',req)
    for label,expr in [('property-reference','case.Wrong'),('always','always(registered == true)'),('eventually','eventually(registered == true)'),('bare-bool','registered'),('bang','!registered')]: fault(label,'reached(registered == true && correct == false)',f'reached({expr})')
    static.append(('duplicate-property',append(BASE,'property case.Wrong = reached(registered == true && correct == false)'),'U-SEM-0','unsupported'))
    fault('duplicate-metric','metric cohort.WrongPerRegistered','metric cohort.WrongPerArrival','U-SEM-0')
    for name in ['received','admitted','id']:
        static.append(('reserved-'+name,append(BASE,f'property case.{name} = reached(registered == true)'),'U-SEM-0','unsupported'))
    fault('binding-condition','reached(registered == true && correct == false)','reached(v == pass)','U-SEM-0')
    fault('unknown-property',metric,'expected(count(case.Nope)) / arrivals','E-NAME-0','invalid')
    fault('unknown-state','reached(registered == true && correct == false)','reached(nope == true)','E-NAME-0','invalid')
    fault('unknown-end','reached(registered == true && correct == false)','reached(end.nope)','E-NAME-0','invalid')
    static.append(('unused-unknown-state',append(BASE,'property case.Unused = reached(nope == true)'),'E-NAME-0','invalid'))
    for label,a,b in [('missing-name','property case.Wrong','property case.'),('missing-equal','property case.Wrong =','property case.Wrong'),('missing-open','= reached(registered == true && correct == false)','= reached registered == true && correct == false)'),('missing-close','reached(registered == true && correct == false)','reached(registered == true && correct == false')]: fault(label,a,b,'E-SYN-0','invalid')
    fault('wrong-type','reached(registered == true && correct == false)','reached(registered == pass)','E-TYPE-0','invalid')
    fault('empty-reached','reached(registered == true && correct == false)','reached()','E-SYN-0','invalid')
    membership=append(BASE,'actor SetHolder { state keys: set<CaseId> }')
    membership=edit(edit(membership,'include [Checker, Ledger]','include [Checker, Ledger, SetHolder]'),'reached(registered == true && correct == false)','reached(case.id in SetHolder.keys)')
    static.append(('membership',membership,'U-SYN-0','unsupported'))
    fault('no-verify','  verify reachable end.posted\n','','U-SEM-0')
    # These old fallback diagnostics must stay at the first word.
    static.append(('old-property',append(no_quality(BASE),'property p = x'),'U-SYN-0','unsupported'))
    static.append(('old-metric',append(no_quality(BASE),'metric m = elapsed'),'U-SYN-0','unsupported'))
    decl='property case.Wrong = reached(registered == true && correct == false)'
    mdecl='metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals'
    for item, line in [('property',decl),('metric',mdecl)]:
        cutline=edit(BASE,'  '+line+'\n','')
        for scope,body in [('configuration',f'configuration Selected {{ cut C; process Flow; {line} }}'),('process',None),('contract',f'contract K {{ {line} }}')]:
            s=edit(cutline,'process Flow {',f'process Flow {{ {line};') if scope=='process' else append(cutline,body)
            label=f'nested-{scope}-{item}'
            static.append((label,s,'U-SYN-0','unsupported'))
    save('qual-static',BASE,q,calc='T15 normal control: W=1/50,R=23/25,N=1/25,D=46/25,A1/50,B1/46. Every broken source changes only the named construct. Expected diagnostic classification is specified in plan T15, not sampled from new parser.')
    for label,s,code,verdict in static:
        bn='qual-static.broken-'+label
        save(bn,s,None,verdict,code=code,calc=f'T15 single fault {label}: {code}/{verdict}. No quality numeric output. The normal control has A1/50,B1/46. See plan section3 T15 for the language boundary.')
        PAIRS.append(dict(base='qual-static',broken=bn,fault=label))
    keywords=edit(edit(BASE,'v = Checker.check()','property = Checker.check()'),'if v == pass','if property == pass')
    keywords=edit(keywords,'a = Ledger.post()','metric = Ledger.post()')
    pair('qual-context-words',BASE,keywords,q,q,'T15: property and metric remain process identifiers,not reserved keywords; quality unchanged. Distinct context determines declaration,not the token alone.',fault='bindings')
    forward=edit(BASE,'  property case.Wrong = reached(registered == true && correct == false)\n  property case.Registered = reached(registered == true)\n','')
    forward=append(forward,'property case.Wrong = reached(registered == true && correct == false)\nproperty case.Registered = reached(registered == true)')
    pair('qual-forward',BASE,forward,q,q,'T15: resolve all property names before metrics; moving declarations after metrics is legal. Preserve declaration order and values1/50,1/46.',fault='forward')
    pon=edit(BASE,'  metric cohort.WrongPerArrival = expected(count(case.Wrong)) / arrivals\n  metric cohort.WrongPerRegistered = expected(count(case.Wrong)) / expected(count(case.Registered))\n','')
    pq=deepcopy(q); pq['cohort']=[]; pq['checked']=[x for x in CHECKED if x not in ['ratio_of_expected_counts','denominator_disclosure']]
    pair('qual-property-only',BASE,pon,q,pq,'T15/U6: property-only still reports all outcome rates. Do not claim ratio/denominator checked when no metric declared.',fault='no-metric')
    dup=edit(union,'reached(end.a || end.b)','reached(end.a || end.b || end.a)')
    pair('qual-union-dedup',union,dup,uq,uq,'T15: repeated End deduplicated in first-occurrence order; union remains1 not2. Canonical condition remains end.a || end.b.',fault='duplicate-end')
    # Late simulation abort: first two repetitions finish, third selects B.
    keys=(ROOT/'fixtures/sim-keys/model.relca').read_text()
    late=edit(keys,'process P { a = A.c(); if a == first { end yes } else { end no } }','process P { a = A.c(); if a == first { b = B.c(); end yes } else { end no } }')
    late=append(late,'actor B { capacity 1; capability c() -> T { duration fixed 1min; result nondet { first } } }\nproperty case.Finished = reached(end.yes || end.no)\nmetric cohort.Rate = expected(count(case.Finished)) / arrivals')
    # Random end counts are intentionally not a fabricated seed oracle. Assert
    # the deterministic Finished totals and complete abort withdrawal in tests.
    pair('qual-late-abort',late,edit(late,'result nondet { first }','result nondet { first, second }'),None,None,
         'F29: source and seed come from existing report::metrics_tests. Deterministic B control is satisfied. First two replications complete; third calls nondeterministic B and refuses. Finished has no unknown in completed runs. On abort retract all quality estimates: reason aborted,samples0,cases hits/unknown/trials0; no retained samples2.',fault='nondeterminism',verdict='satisfied',broken_verdict='unsupported',invocation={'command':'simulate'})
    # End precedes contest-route state write; same old verify oracle is added by the probe.
    contest=(ROOT/'fixtures/ctx-contest/model.relca').read_text()
    contest=append(contest,'observed case { reviewed: bool = false }\nproperty case.Reviewed = reached(reviewed == true)\nproperty case.Returned = reached(end.returned)\nmetric cohort.ReviewedRate = expected(count(case.Reviewed)) / arrivals\nmetric cohort.ReturnedRate = expected(count(case.Returned)) / arrivals')
    contest=edit(contest,'duration fixed 2min; result nondet { pass }','duration fixed 2min; result nondet { pass }; writes reviewed; finish { reviewed = true }')
    ccq=single(prop('Reviewed','reviewed == true',[(0,1)],(0,)),[('end','returned',1),('end','posted',0)]+zero_out,metric='ReviewedRate')
    add_property(ccq,'Returned','end.returned',[1],(0,),metric='ReturnedRate')
    ccb=deepcopy(ccq); ccb['properties'][0]=prop('Reviewed','reviewed == true',[0],(0,)); ccb['cohort'][0]=ratio('ReviewedRate','Reviewed',None,ccb['properties'],1)
    pair('qual-post-end-state',contest,edit(contest,'finish { reviewed = true }','finish { reviewed = false }'),ccq,ccb,
         'F35: business End returned at1 fixes Returned1. Contest optional nondeterministic branch either absent (Reviewed0) or route finishes3 and writes reviewed=true (Reviewed1). Reviewed range[0,1] known per_case_relaxation. Broken route writes false: Reviewed0. Business End does not close state-property observation.',fault='no-hit')
    # Deferred fixtures retain full source and independent expected wire here;
    # they are not installed in the current fixtures population.
    horizon=edit(clock,'  arrivals fixed [0min, 6min, 12min]\n','')
    horizon=edit(horizon,'  bounds { cases 3; live_tasks 2; clock 14min }\n','')
    horizon=append(horizon,'scenario Day { arrivals fixed_every 6min during [0min, 18min); horizon 14min; bounds { cases 3; live_tasks 2; clock 30min } }')
    hq=quality(['1/50','1/50',(0,1)],['23/25','23/25',(0,1)],(0,6,12),[('end','returned','4/25'),('end','posted','46/25'),('cut_off',None,1)],[None,None,'undetermined'])
    pair('qual-horizon',horizon,clock,hq,clq,'T06: horizon14 cuts case3,not a computational cut. A null undetermined[1/75,26/75],B[1/71,13/23]. Observed outcomes posted46/75,returned4/75,cut_off1/3,unfinished0. Clock counterpart identical property bounds but truncated and no cut_off row.',fault='clock',verdict='inconclusive',deferred='ADR0042 scenario/horizon')
    ROWS['qual-horizon']['invocation']={'command':'check','scenario':'Day'}
    sh=edit(edit(horizon,'0.9: pass with correct = true; 0.02: pass with correct = false; 0.08: fail with correct = false','1: pass with correct = true'),'scenario Day {','scenario Day { simulation replications 4 seed 1 confidence 0.5;')
    shq=quality([0,0,(0,1)],[1,1,(0,1)],(0,6,12),[('end','returned',0),('end','posted',2),('cut_off',None,1)],[None,None,'undetermined'])
    pair('qual-sim-horizon',sh,sc,simulation(shq,[[0,0,0],[1,1,0]],[[0,0,1],[0,0,1]]),scq,'T10: hits/unknown from deterministic correct kernel: N_W0,U_W4,N_R8,U_R4,arrivals12. Undetermined A[0,1/3],B[0,1/2]. Clock counterpart truncated. Replications4,not12.',fault='clock',verdict='inconclusive',invocation={'command':'simulate'},deferred='ADR0042 scenario/horizon')
    ROWS['qual-sim-horizon']['invocation']={'command':'simulate','scenario':'Day'}
    for p in PAIRS:
        before=ROWS[p['base']]['source'].encode()
        after=ROWS[p['broken']]['source'].encode()
        p['edits']=[dict(start=i,end=j,old=before[i:j].decode(),new=after[k:l].decode())
                    for op,i,j,k,l in difflib.SequenceMatcher(None,before,after,autojunk=False).get_opcodes() if op!='equal']
        reconstructed=before
        for e in reversed(p['edits']):
            reconstructed=reconstructed[:e['start']]+e['new'].encode()+reconstructed[e['end']:]
        assert reconstructed==after and before!=after
    if '--draft' in sys.argv:
        (ROOT/'.tmp/quality-draft.json').write_text(json.dumps(ROWS,indent=2)+'\n')
        print(f'{len(ROWS)} draft inputs, no fixture population changed')
        return
    # Materialize all independent calculations first, sources second. No executable oracle.
    names=sorted(n for n,r in ROWS.items() if not r['deferred'])
    (HERE/'quality_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (HERE/'quality_names.rs').write_text('[\n'+''.join(f'    "{n}",\n' for n in names)+']\n')
    (HERE/'quality_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    deferred={n:r for n,r in ROWS.items() if r['deferred']}
    (HERE/'quality_deferred.json').write_text(json.dumps(deferred,indent=2)+'\n')
    inventory={}
    for name in names:
        row=ROWS[name]; d=ROOT/'fixtures'/name; d.mkdir(exist_ok=True)
        (d/'calc.md').write_text('# Independent hand calculation (ADR 0045)\n\n'+row['calc']+'\n\nNumbers above are rational counts/probabilities, time is minutes. This is authored acceptance, not implementation verification.\n')
        (d/'expected.json').write_text(json.dumps(row['expected'],indent=2)+'\n')
        (d/'model.relca').write_text(row['source'])
        if row['invocation']: (d/'invocation.json').write_text(json.dumps(row['invocation'],indent=2)+'\n')
        inventory[name]={k:v for k,v in row.items() if k not in ('source','expected','calc','deferred')}
    (HERE/'quality_inventory.json').write_text(json.dumps(inventory,indent=2)+'\n')
    print(f'{len(names)} installed inputs; {len(deferred)} deferred; {len(PAIRS)} edit witnesses')


if __name__=='__main__':
    main()
