"""Hand-only ADR0049 materializer. Never runs or imports Relca arithmetic.

Run with python3 -B after copying into crates/relca-core/tests/support.
The numerical rows below are left inverses of the explicitly stated CDFs.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
rows = {}
pairs = []
deferred = {}


def edit(s, old, new):
    assert old in s, old
    out = s.replace(old, new, 1)
    assert out != s
    return out


def append(s, lines):
    return edit(s, '\n}\n', '\n' + lines.rstrip() + '\n}\n')


def metric(name='Median', p='0.5', prop='Done', given=False):
    return f'  metric quantile.{name} = quantile({p}, case.{prop}' + (' given reached' if given else '') + ')\n'


def range_row(name, p, lo=None, hi=None, basis='exact', reason=None, lb=None,
              prop='Done', given=False):
    return dict(name=name, p=p, property=prop,
                form='reached_only' if given else 'all_arrivals',
                value=dict(lower=None if lo is None else str(lo),
                           upper=None if hi is None else str(hi),
                           basis=basis if lo is not None else None, reason=reason),
                lower_bound=None if lb is None else str(lb))


def sample_row(name, p, point, n, c=0, reason=None, lb=None, prop='Done', given=False):
    return dict(name=name, p=p, property=prop,
                form='reached_only' if given else 'all_arrivals',
                value=dict(point=None if point is None else str(point), observations=n,
                           censored=c, reason=reason), lower_bound=None if lb is None else str(lb))


def two(t, p='1/2', basis='exact', prop='Done'):
    lo, hi = t if isinstance(t, tuple) else (t, t)
    return [range_row('Median', p, lo, hi, basis, prop=prop),
            range_row('MedianDone', p, lo, hi, basis, prop=prop, given=True)]


def missing(reason, lb=None, given_reason=None, given_lb=None):
    return [range_row('Median','1/2',reason=reason,lb=lb),
            range_row('MedianDone','1/2',reason=given_reason or reason,lb=given_lb,given=True)]


def add(name, source, q, calc, verdict='satisfied', invocation=None, code=None,
        message=None, defer=None):
    assert '//' not in source and '#' not in source, name
    expected: dict = {'verdict': verdict}
    if q is not None:
        expected['quality'] = {'quantiles': q}
    if code:
        expected['diagnostics'] = [{'code': code, 'severity':'error'}]
        if message:
            expected['diagnostics'][0]['message'] = message
    row = dict(source=source, expected=expected, calc=calc, invocation=invocation,
               code=code, message=message)
    if defer:
        row['dependency'] = defer
        deferred[name] = row
    else:
        rows[name] = row
    return source


def broken(base, fault, old, new, q, calc, **kwargs):
    table = rows if base in rows else deferred
    s = table[base]['source']
    changed = edit(s, old, new)
    name = base + '.broken-' + fault
    pairs.append(dict(base=base, broken=name, old=old, new=new))
    defaults = {k:table[base].get(k) for k in ('invocation','defer')}
    defaults.update(kwargs)
    add(name, changed, q, calc, **defaults)
    return changed


def basic(duration=3, cases=1, arrivals=None, policy=True, verify='reachable end.done'):
    a = '' if arrivals is None else f'  arrivals fixed [{arrivals}]\n'
    return f'''model Qtl version "0.2" {{
  type T = enum {{ ok }}
  actor A {{ capacity 1; capability run() -> T {{ duration fixed {duration}min; result nondet {{ ok }} }} }}
  process P {{ job = A.run(); end done }}
  cut C {{ include [A] }}
{a}''' + ('  policy fifo tie_by case_id\n' if policy else '') + f'''  bounds {{ cases {cases}; live_tasks 1; clock 10min }}
  verify {verify}
  property case.Done = reached(end.done)
''' + metric() + metric('MedianDone', given=True) + '}\n'


def materialize():
    samp = (ROOT/'fixtures/met-sample/model.relca').read_text()
    samp = append(samp, '  property case.Done = reached(end.done)\n' + metric() + metric('P95','0.95'))
    sample_q = [range_row('Median','1/2',2,2),range_row('P95','19/20',4,4)]
    add('qtl-sample',samp,sample_q,
        'T01. T=2 with mass1/2 and T=4 with mass1/2. F(2)=p=1/2, so the left inverse is2; p95=4. No interpolation. Check S/0.')
    broken('qtl-sample','shifted','2min:0.5, 4min:0.5','2min:0.4, 4min:0.6',
           [range_row('Median','1/2',4,4),range_row('P95','19/20',4,4)],
           'T01 fault. F(2)=2/5<1/2, F(4)=1. Both left inverses4. Check S/0.')
    add('qtl-sample-sim',samp,[sample_row('Median','1/2',2,4),sample_row('P95','19/20',4,4)],
        'T01/T07. The frozen seed41020 supports [2,2,4,4] as a multiset (old mean3,min2,max4,n4). Nearest ranks2 and4 yield2/4. Safety sampling is not proof: I/2.',
        verdict='inconclusive', invocation={'command':'simulate'})
    broken('qtl-sample-sim','short-clock','clock 10min','clock 3min',
           [sample_row('Median','1/2',2,4,2),sample_row('P95','19/20',None,4,2,'truncated',4)],
           'T07. Pool=[2,2,>3,>3]. At2 the rank2 is known. Rank4 may be infinite; optimistic completion at4 gives LB4, not last event2+1. I/2.',verdict='inconclusive')
    identified=edit(samp,'  verify safety no_resource_overcommit\n','')
    add('qtl-identifiable',identified,sample_q,
        'T07 exhaustive identification supplement. F(1)=0,F(2)=1/2,F(4)=1; med2,p954,exact. Reachability only S/0.')
    broken('qtl-identifiable','clock','clock 10min','clock 3min',
           [range_row('Median','1/2',2,2),range_row('P95','19/20',reason='truncated',lb=4)],
           'T07 exhaustive supplement. Known F(1)=0,F(2)=1/2 in all four endpoints fixes BOTH median endpoints at2 despite unknown late mass. Tail p95 is unknown,optimistic earliest4 gives LB4. Existential reachable already has a witness,so S/0; do not make quantile null change verdict.')

    q = (ROOT/'fixtures/qual-base/model.relca').read_text()
    q = append(q, '  property case.Posted = reached(end.posted)\n' + metric('P90','0.9','Posted') + metric('P95','0.95','Posted') + metric('P95Done','0.95','Posted',True))
    add('qtl-censor',q,[range_row('P90','9/10',4,4,prop='Posted'),
        range_row('P95','19/20',reason='beyond_reached_mass',prop='Posted'),
        range_row('P95Done','19/20',4,4,prop='Posted',given=True)],
        'T02. Each arrival posts after3+1=4min with mass9/10+1/50=23/25; miss2/25. F(4)=23/25>=9/10 but <19/20. Conditional F_c(4)=1. Reachable-only root S/0.')
    broken('qtl-censor','clock','clock 30min','clock 12min',
           [range_row(n,p,reason='truncated',lb=4,prop='Posted',given=g)
            for n,p,g in [('P90','9/10',False),('P95','19/20',False),('P95Done','19/20',True)]],
           'T02. Case1 known23/25,case2 unknown. Optimistic F(3)=1/2,F(4)=24/25. Conditional upper at3=25/48,at4=1; lower remains23/48. All3 values unknown,LB4. I/2.',verdict='inconclusive')

    c = basic(cases=2,arrivals='0min, 0min')
    add('qtl-contention',c,two(3),'T03. FIFO T=[3,6], F(3)=1/2. Both forms median3 exact. S/0.')
    broken('qtl-contention','no-policy','  policy fifo tie_by case_id\n','',two((3,6),basis='per_case_relaxation'),
           'T03. At3 each case A=[0,1],B=[0,1],D=1. Sums A/B=[0,2],conditional CDF=[0,1]. At6 both CDFs1. Interval[3,6] is NOT the simultaneous median3. S/0.')

    m = '''model QtlMaybe version "0.2" {
  type V = enum { pass, fail }
  actor A { capacity 1; capability check() -> V { duration fixed 2min; result nondet { pass, fail } } }
  process P { v = A.check(); if v == pass { end done } else { end returned } }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  property case.Done = reached(end.done)
''' + metric() + metric('MedianDone',given=True) + '}\n'
    add('qtl-maybe',m,missing('beyond_reached_mass',2,'denominator_may_be_zero'),
        'T04. Completely solved nondeterminism Dlo0,Dhi1. All-arrival optimistic inverse2 but no finite upper; given denominator may be zero and has NO lower_bound. S/0.')
    passonly = broken('qtl-maybe','pass-only','result nondet { pass, fail }','result nondet { pass }',two(2),
           'T04. Only pass: D=1,T=2. Both quantiles2 exact. Else/returned unreachable warnings remain. S/0.')
    broken('qtl-maybe','fail-only','result nondet { pass, fail }','result nondet { fail }',
           missing('beyond_reached_mass',given_reason='zero_denominator'),
           'T05. Dhi=0. All-arrival p cannot be met,conditional denominator0. No lower bound. Reachable done fails independently: V/1.',verdict='violated')
    one = edit(edit(passonly,'quantile(0.5, case.Done)','quantile(1, case.Done)'),
               'quantile(0.5, case.Done given reached)','quantile(1, case.Done given reached)')
    add('qtl-one',one,two(2,p='1'),'T06. p=1 is valid: full finite mass occurs at2. Both forms2. S/0.')
    for label, phrase in [('all','quantile(1, case.Done)'),('given','quantile(1, case.Done given reached)')]:
        broken('qtl-one','zero-'+label,phrase,phrase.replace('(1,','(0,'),None,
               'T06. Exactly one p is zero; zero is invalid,not an initial-time quantile. X/3.',
               verdict='invalid',code='E-PROB-0',message='quantile level must be positive')

    label = add('qtl-label',c,two(3),'T11. FIFO median3; preserve declaration order,not name sort. S/0.')
    renamed = label
    for a,b in [('MedianDone','A0'),('Median','Z1'),('Done','Q9'),('actor A {','actor Desk { grain team;'),('A.run','Desk.run'),('include [A]','include [Desk]')]:
        before = renamed
        renamed = edit(renamed,a,b) if renamed.count(a)==1 else renamed.replace(a,b)
        assert renamed != before
    assert renamed != label
    rq = two(3,prop='Q9');rq[0]['name']='Z1';rq[1]['name']='A0'
    add('qtl-label.broken-renamed',renamed,rq,
        'T11. A bijective relabel,plus meaningless grain team. T=[3,6],median3 exact; output order[Z1,A0]. Model hash/spans/explicit metadata differ; numeric/reasons/verdict do not. S/0.')
    pairs.append(dict(base='qtl-label',broken='qtl-label.broken-renamed',edits='bijection'))

    sim = append(c,'  simulation replications 4 seed 1 confidence 0.5')
    sq=[sample_row('Median','1/2',3,8),sample_row('MedianDone','1/2',3,8,given=True)]
    add('qtl-nondeterminism',sim,sq,'T13. Four repetitions of[3,6]: poolN8,rank4 is3. Reachability witnessed: S/0.',invocation={'command':'simulate'})
    broken('qtl-nondeterminism','no-policy','  policy fifo tie_by case_id\n','',
           [sample_row('Median','1/2',None,0,reason='aborted'),sample_row('MedianDone','1/2',None,0,reason='aborted',given=True)],
           'T13. Unresolved first-start scheduler choice must abort,not randomize. Retract all observations and inventory: N0,C0. U/4.',
           verdict='unsupported',code='U-SEM-0')

    mutable = '''model QtlState version "0.2" {
  type T = enum { ok }
  observed case { hit: bool = false }
  actor A { capacity 1;
    capability on() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = true } }
    capability off() -> T { duration fixed 2min; result nondet { ok }; writes hit; finish { hit = false } }
  }
  process P { a = A.on(); b = A.off(); end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  simulation replications 2 seed 1 confidence 0.5
  verify reachable end.done
  property case.Hit = reached(hit == true)
''' + metric('First','1','Hit') + metric('FirstDone','1','Hit',True) + '}\n'
    for suffix, sampled in [('',False),('-sim',True)]:
        qr=[sample_row(n,'1',2,2,prop='Hit',given=g) if sampled else range_row(n,'1',2,2,prop='Hit',given=g)
            for n,g in [('First',False),('FirstDone',True)]]
        add('qtl-first-hit'+suffix,mutable,qr,'T15. hit false@0,true@2,false@4. FIRST hit2 survives overwrite,not terminal4. S/0.',invocation={'command':'simulate'} if sampled else None)
        qmiss=[sample_row(n,'1',None,2,reason=r,prop='Hit',given=g) if sampled else range_row(n,'1',reason=r,prop='Hit',given=g)
            for n,g,r in [('First',False,'beyond_reached_mass'),('FirstDone',True,'zero_denominator')]]
        broken('qtl-first-hit'+suffix,'no-hit','finish { hit = true }','finish { hit = false }',qmiss,
               'T15. No true state; all known miss,not censored. No lower bounds. End still reachable: S/0.')

    within=append(edit(samp,'property case.Done = reached(end.done)','property case.Done = reached(end.done) within 4min'),metric('P95Done','0.95',given=True))
    add('qtl-within',within,sample_q+[range_row('P95Done','19/20',4,4,given=True)],'T16. Inclusive within4 retains full support{2,4}; D=1. S/0.')
    broken('qtl-within','short-within','within 4min','within 3min',
           [range_row('Median','1/2',2,2),range_row('P95','19/20',reason='beyond_reached_mass'),range_row('P95Done','19/20',2,2,given=True)],
           'T16. Within3 leaves only mass1/2 at2; F(2)=1/2 reaches median; p95 beyond mass,conditional p95=2. Deadline must not be ignored. S/0.')

    zero = edit(edit(basic(), '  actor A {','  observed case { hit: bool = true }\n  actor A {'),
                'reached(end.done)','reached(hit == true) within 0min')
    add('qtl-initial',zero,two(0),'T16 initial true at arrival and within0 inclusive: both medians0. S/0.')
    broken('qtl-initial','false','hit: bool = true','hit: bool = false',missing('beyond_reached_mass',given_reason='zero_denominator'),
           'T16. Initial false with no writer,deadline0 closes known miss. S/0.')

    start=edit(edit(basic(duration=1),'  actor A {','  observed case { hit: bool = false }\n  actor A {'),
               'duration fixed 1min','start { hit = true }; duration fixed 1min')
    start=edit(start,'reached(end.done)','reached(hit == true)')
    add('qtl-same-tick',start,two(0),'T17. Start effect hits at arrival tick0 before completion1. At engine budget0 this same source is unknown and LB0 (see core test). S/0.')
    broken('qtl-same-tick','no-hit','start { hit = true }','start { hit = false }',missing('beyond_reached_mass',given_reason='zero_denominator'),
           'T17 control. No hit after start false; complete miss. S/0.')

    clock=edit(basic(duration=2),'clock 10min','clock 2min')
    add('qtl-denominator',clock,two(2),'T18. Hit exactly at inclusive clock2; D=1,quantiles2. S/0.')
    broken('qtl-denominator','clock','clock 2min','clock 1min',missing('truncated',2,'truncated'),
           'T18. Unknown D=[0,1] caused by clock,not solved scheduler width. All optimistic LB2; conditional LBnull because Dlo0. I/2.',verdict='inconclusive')

    direct='''model QtlDirect version "0.2" {
  type C = enum { low, high }
  type V = enum { pass, fail }
  kernel half { * -> { 0.5: pass; 0.5: fail } }
  actor A { capacity 1;
    capability choose() -> C { duration fixed 1min; result nondet { low, high } }
    capability low() -> V { duration {1min:0.4, 3min:0.6}; outcomes kernel half }
    capability high() -> V { duration {1min:0.6, 3min:0.4}; result nondet { pass } }
  }
  process P { c = A.choose(); if c == low { lval = A.low(); if lval == pass { end done } else { end returned } } else { rval = A.high(); if rval == pass { end done } else { end returned } } }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  property case.Done = reached(end.done)
''' + metric('Conditional','0.8',given=True) + '}\n'
    add('qtl-direct-b',direct,[range_row('Conditional','4/5',4,4,'per_case_relaxation',given=True)],
        'T19. Choose1 then low durations1/3 and independent pass1/2 => T2:1/5,T4:3/10,miss1/2; high T2:3/5,T4:2/5. At2 A[1/5,3/5],B[3/10,2/5],Fc[1/3,2/3]; .8 inverse[4,4] still per_case_relaxation. S/0.')
    broken('qtl-direct-b','redistributed','1min:0.4, 3min:0.6','1min:0.8, 3min:0.2',
           [range_row('Conditional','4/5',2,4,'per_case_relaxation',given=True)],
           'T19. Low T2:2/5,T4:1/10,miss1/2. At2 A[2/5,3/5],B[1/10,2/5],Fc[1/2,6/7]; .8 inverse[2,4]. D unchanged; B is NOT Dlo-Ahi. S/0.')

    crossing='''model QtlCrossing version "0.2" {
  type C = enum { left, right }
  type T = enum { ok }
  actor A { capacity 1;
    capability choose() -> C { duration fixed 1min; result nondet { left, right } }
    capability left() -> T { duration {1min:0.6, 5min:0.4}; result nondet { ok } }
    capability right() -> T { duration fixed 3min; result nondet { ok } }
  }
  process P { c = A.choose(); if c == left { lwork = A.left() } else { rwork = A.right() }; end done }
  cut C { include [A] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify reachable end.done
  property case.Done = reached(end.done)
''' + metric() + metric('P90','0.9') + '}\n'
    add('qtl-crossing',crossing,[range_row('Median','1/2',2,4,'per_case_relaxation'),range_row('P90','9/10',4,6,'per_case_relaxation')],
        'T20. L:T2=.6,T6=.4,R:T4=1. CDF maximum switches fromL at2 toR at4. q.5=[2,4],q.9=[4,6],both relaxed. S/0.')
    broken('qtl-crossing','later-right','duration fixed 3min','duration fixed 6min',
           [range_row('Median','1/2',2,7,'per_case_relaxation'),range_row('P90','9/10',6,7,'per_case_relaxation')],
           'T20. Only R shifts4->7. L unchanged. q.5=[2,7],q.9=[6,7]. S/0.')

    offset=append(edit(c,'quantile(0.5,','quantile(1,'),'  concurrent cases 1 on_overflow wait_external fifo')
    # Change the other metric as well; the pair changes arrivals only.
    offset=edit(offset,'quantile(0.5,','quantile(1,')
    add('qtl-offset',offset,two(6,p='1'),'T22. External wait preserves arrival clock: absolute ends3/6,arrivals0/0,elapsed3/6,q1=6. S/0.')
    broken('qtl-offset','later-arrival','[0min, 0min]','[0min, 1min]',two(5,p='1'),
           'T22. Absolute ends still3/6; elapsed3/(6-1)=5. Admission of case2 at3 is not the origin. q1=5. S/0.')

    union=edit(m,'property case.Done = reached(end.done)','property case.Done = reached(end.done || end.returned)')
    add('qtl-union',union,two(2),'T23. One union predicate true on every terminal branch,T2,D1. Do not add separately minimized terminal probabilities. S/0.')
    broken('qtl-union','single-end','end.done || end.returned','end.done',missing('beyond_reached_mass',2,'denominator_may_be_zero'),
           'T23. Removing returned from target recovers M: all LB2 without value,given denominator may zero. S/0.')

    reject=append(edit(c,'verify reachable end.done','verify safety no_resource_overcommit'),'  queue capacity 1 on_overflow reject')
    reject=edit(reject,metric('MedianDone',given=True).rstrip(),metric('P95','0.95').rstrip()+'\n'+metric('P95Done','0.95',given=True).rstrip())
    add('qtl-rejected',reject,[range_row('Median','1/2',3,3),range_row('P95','19/20',reason='beyond_reached_mass'),range_row('P95Done','19/20',3,3,given=True)],
        'T26. One hit3,one admission reject; denominator2. F(3)=1/2. Conditional mass1 gives p95=3. S/0.')
    broken('qtl-rejected','reject-all','queue capacity 1','queue capacity 0',
           [range_row('Median','1/2',reason='beyond_reached_mass'),range_row('P95','19/20',reason='beyond_reached_mass'),range_row('P95Done','19/20',reason='zero_denominator',given=True)],
           'T26. Both rejected,N remains2,D0. All-arrival beyond mass,given zero denominator,no LB. Safety S/0.')
    add('qtl-deadlock',basic(),two(3),'T27. Single positive-capacity call completes3. S/0.')
    broken('qtl-deadlock','zero-capacity','capacity 1','capacity 0',missing('beyond_reached_mass',given_reason='zero_denominator'),
           'T27. No enabled work,complete deadlock is known miss,not censor. W-DEADLOCK-0 and unreachable done V/1.',verdict='violated')

    static=basic()
    add('qtl-static',static,two(3),'T12. Positive baseline; each broken directory changes exactly one syntax/name/level component. S/0.')
    for fault,old,new,verdict,code,msg in [
        ('given', 'given reached','given case.Done','unsupported','U-SYN-0','quantile given must be reached'),
        ('given-missing','given reached','given','unsupported','U-SYN-0','quantile given must be reached'),
        ('unknown','quantile(0.5, case.Done)','quantile(0.5, case.Absent)','invalid','E-NAME-0','unknown quality property'),
        ('zero','quantile(0.5,','quantile(0,','invalid','E-PROB-0','quantile level must be positive'),
        ('too-large','quantile(0.5,','quantile(1.5,','invalid','E-PROB-0','probability outside 0..1'),
        ('negative','quantile(0.5,','quantile(-0.1,','invalid','E-PROB-0','probability outside 0..1'),
        ('fraction','quantile(0.5,','quantile(1/2,','unsupported','U-SYN-0','unsupported syntax: exponent or fraction probability'),
        ('exponent','quantile(0.5,','quantile(5e-1,','unsupported','U-SYN-0','unsupported syntax: exponent or fraction probability'),
        ('duplicate','quantile.MedianDone','quantile.Median','unsupported','U-SEM-0','duplicate quantile metric name'),
        ('equal','quantile.Median =','quantile.Median','invalid','E-SYN-0',None),
        ('open','quantile(0.5,','quantile 0.5,','invalid','E-SYN-0',None),
        ('comma','quantile(0.5,','quantile(0.5','invalid','E-SYN-0',None),
        ('close','quantile(0.5, case.Done)','quantile(0.5, case.Done','invalid','E-SYN-0',None),
        ('property-name','quantile(0.5, case.Done)','quantile(0.5, case.)','invalid','E-SYN-0',None),
    ]:
        broken('qtl-static',fault,old,new,None,'T12. Single static fault '+fault+'; no partial quality. '+verdict+'/'+{'invalid':'3','unsupported':'4'}[verdict]+'.',verdict=verdict,code=code,message=msg)
    for fault,text in [('quantiles','quantiles [0.5]'),('p95','p95 lead_time'),('median','median lead_time')]:
        broken('qtl-static',fault,'  verify reachable end.done',f'  metrics {{ {text} }}\n  verify reachable end.done',None,
               'T12. Old metrics syntax remains unsupported with its original diagnostic,not newly accepted quantile syntax.',
               verdict='unsupported',code='U-SYN-0',message='unsupported syntax: '+fault)
    for where,old,new in [
        ('process','process P { job = A.run(); end done }','process P { job = A.run();\n'+metric('Nested')+' end done }'),
        ('configuration','  cut C { include [A] }','  cut C { include [A] }\n  configuration Plan { cut C; process P;\n'+metric('Nested')+' }'),
        ('scenario','  bounds { cases 1; live_tasks 1; clock 10min }','  bounds { cases 1; live_tasks 1; clock 10min }\n  scenario Day {\n'+metric('Nested')+' }'),
    ]:
        broken('qtl-static','nested-'+where,old,new,None,
               'T12. Nested quantile is unsupported; model-direct declarations alone are accepted.',verdict='unsupported',code='U-SYN-0')
    for order in ['cohort-first','quantile-first']:
        lines='  metric cohort.Median = expected(count(case.Done)) / arrivals\n'
        old=metric()
        new=lines+old if order=='cohort-first' else old+lines
        broken('qtl-static',order,old,new,None,
               'T12. One shared metric namespace. Later declaration owns duplicate-name span; no partial quality.',
               verdict='unsupported',code='U-SEM-0',message='duplicate quantile metric name')

    # Sources that rely on the pending B6 horizon integration are committed as
    # hand data, not quietly excluded runtime fixtures.
    horizon=edit(c,'  arrivals fixed [0min, 0min]\n','')
    horizon=edit(horizon,'  bounds { cases 2; live_tasks 1; clock 10min }',
                 '  scenario Day { arrivals fixed [0min, 0min]; horizon 6min; bounds { cases 2; live_tasks 1; clock 10min }; simulation replications 4 seed 1 confidence 0.5 }')
    horizon=append(horizon,metric('P75','0.75')+metric('P75Done','0.75',given=True))
    hq=sq+[sample_row('P75','3/4',6,8),sample_row('P75Done','3/4',6,8,given=True)]
    add('qtl-pooling',horizon,hq,'T14. Four replications[3,6],N8,ranks4/6=>3/6. S/0.',invocation={'command':'simulate','scenario':'Day'},defer='B6 horizon integration')
    broken('qtl-pooling','horizon','horizon 6min','horizon 4min',
           [sample_row('Median','1/2',3,8,4),sample_row('MedianDone','1/2',3,8,4,given=True),
            sample_row('P75','3/4',None,8,4,'undetermined',5),sample_row('P75Done','3/4',None,8,4,'undetermined',3,given=True)],
           'T14. Pool[3,>4]x4. Conditional optimistic drops late unknown =>LB3; all optimistic places at5=>LB5. Both medians3 identifiable. I/2.',verdict='inconclusive',defer='B6 horizon integration')
    dh=edit(clock,'  bounds { cases 1; live_tasks 1; clock 2min }',
            '  scenario Day { arrivals fixed [0min]; horizon 2min; bounds { cases 1; live_tasks 1; clock 10min } }')
    add('qtl-denominator-horizon',dh,two(2),'T18. Inclusive horizon2 reaches2. S/0.',invocation={'command':'check','scenario':'Day'},defer='B6 horizon integration')
    broken('qtl-denominator-horizon','horizon','horizon 2min','horizon 1min',missing('undetermined',2,'undetermined'),
           'T18. Horizon only: unknown denominator has reason undetermined,not denominator_may_be_zero; conditional LBnull. I/2.',verdict='inconclusive',defer='B6 horizon integration')
    latest=edit(basic(duration=3,cases=2,arrivals='0min, 2min'),'actor A { capacity 1','actor A { capacity 2')
    latest=edit(latest,'  arrivals fixed [0min, 2min]\n','')
    latest=edit(latest,'  bounds { cases 2; live_tasks 1; clock 10min }',
                '  scenario Day { arrivals fixed [0min, 2min]; horizon 3min; bounds { cases 2; live_tasks 2; clock 10min }; simulation replications 1 seed 1 confidence 0.5 }')
    latest=edit(latest,'reached(end.done)','reached(end.done) within 3min')
    before=latest
    latest=latest.replace('quantile(0.5,','quantile(1,')
    assert latest!=before
    add('qtl-censor-within',latest,[sample_row('Median','1',None,2,1,'undetermined',3),sample_row('MedianDone','1',3,2,1,given=True)],
        'T25. Hit3 and Censored earliest2/latest3. Given all finite hits<=3 so point3. All may miss mass and stays null,LB3. I/2.',verdict='inconclusive',invocation={'command':'simulate','scenario':'Day'},defer='B6 horizon integration')
    broken('qtl-censor-within','no-within',' within 3min','',
           [sample_row('Median','1',None,2,1,'undetermined',3),sample_row('MedianDone','1',None,2,1,'undetermined',3,given=True)],
           'T25. Without latest3 the unknown can arrive after3,so conditional p1 loses identification. I/2.',verdict='inconclusive',defer='B6 horizon integration')

    # B7 objective wire is not fabricated before that implementation lands.
    # Store complete sources and finite semantic oracles for the integration
    # owner; these are a separate population, not skipped installed fixtures.
    objective='''model QtlObjective version "0.2" {
  type T = enum { ok }
  actor A { capacity 1; capability run() -> T { duration fixed 3min; result nondet { ok } } }
  actor B { capacity 2; capability run() -> T { duration fixed 4min; result nondet { ok } } }
  process P { job = A.run(); end done }
  process Q { job = B.run(); end done }
  cut C { include [A,B] }
  configuration One { cut C; process P }
  configuration Two { cut C; process Q }
  arrivals fixed [0min, 0min]
  policy fifo tie_by case_id
  bounds { cases 2; live_tasks 2; clock 10min }
  verify reachable end.done
  property case.Done = reached(end.done)
  property case.OnTime = reached(end.done) within 4min
  metric cohort.OnTimePerArrival = expected(count(case.OnTime)) / arrivals
''' + metric() + '''  compare [One, Two] cut C hard { cohort.OnTimePerArrival >= 0.5 } objectives { minimize quantile.Median }
}
'''
    add('qtl-objective',objective,None,'T08. One FIFO T3/6 median3,hard1/2;Two T4/4 median4,hard1. Both admissible,non_dominated[One],S/0.',
        invocation={'command':'compare','plans':['One','Two'],'mode':'exhaustive'},defer='ADR0047/B7')
    deferred['qtl-objective']['semantic_oracle']={'quantiles':{'One':two(3)[:1],'Two':two(4)[:1]},'among':['One','Two'],'non_dominated':['One'],'not_evaluated':[]}
    broken('qtl-objective','no-policy','  policy fifo tie_by case_id\n','',None,
           'T08. One hard[0,1] inconclusive but objective[3,6] retained. One undetermined/not_evaluated;among/non_dominated[Two],I/2.',verdict='inconclusive',defer='ADR0047/B7')
    deferred['qtl-objective.broken-no-policy']['semantic_oracle']={'quantiles':{'One':two((3,6),basis='per_case_relaxation')[:1],'Two':two(4)[:1]},'among':['Two'],'non_dominated':['Two'],'not_evaluated':['One']}

    pareto=(ROOT/'fixtures/met-compare/model.relca').read_text()
    pareto=edit(pareto,'    capability long() -> T { duration fixed 5min; result nondet { pass } }',
                '    capability long() -> T { duration fixed 5min; result nondet { pass } }\n    capability fastlong() -> T { duration fixed 5min; result nondet { pass } }')
    pareto=edit(pareto,'process Parallel { fork f { a = Office.short(); b = Office.long() }','process Parallel { fork f { a = Office.short(); b = Office.fastlong() }')
    pareto=edit(pareto,'rate Office.long per_start 3','rate Office.long per_start 3; rate Office.fastlong per_start 3')
    pareto=append(pareto,'  property case.Done = reached(end.done)\n'+metric('Lead','1')+
                  '  compare [AsIs, ToBeP] cut C objectives { minimize quantile.Lead; minimize metrics.overall.observed_cost_total }')
    add('qtl-pareto',pareto,None,'T09. Serial time3+5=8,parallel max(3,5)=5. Both cost(3+5)*100+2+3=805. non_dominated[ToBeP],S/0.',
        invocation={'command':'compare','plans':['AsIs','ToBeP'],'mode':'exhaustive'},defer='ADR0047/B7')
    deferred['qtl-pareto']['semantic_oracle']={'lead':{'AsIs':'8','ToBeP':'5'},'cost':{'AsIs':'805','ToBeP':'805'},'non_dominated':['ToBeP'],'undetermined_pairs':[]}
    broken('qtl-pareto','pricier-parallel','rate Office.fastlong per_start 3','rate Office.fastlong per_start 300',None,
           'T09. Only fastlong start fee3->300. Serial8/805,parallel5/(800+2+300)=1102. Neither dominates;non_dominated[AsIs,ToBeP],no undetermined pair,S/0.',defer='ADR0047/B7')
    deferred['qtl-pareto.broken-pricier-parallel']['semantic_oracle']={'lead':{'AsIs':'8','ToBeP':'5'},'cost':{'AsIs':'805','ToBeP':'1102'},'non_dominated':['AsIs','ToBeP'],'undetermined_pairs':[]}
    missing_rate=append((ROOT/'fixtures/met-compare/model.relca').read_text(),
                        '  property case.Done = reached(end.done)\n'+metric('Lead','1')+
                        '  compare [AsIs, ToBeP] cut C objectives { minimize quantile.Lead; minimize metrics.overall.observed_cost_total }')
    add('qtl-missing-rate',missing_rate,None,'T10. Both plans use long. Normal prices805 each,time8/5;non_dominated[ToBeP],S/0.',
        invocation={'command':'compare','plans':['AsIs','ToBeP'],'mode':'exhaustive'},defer='ADR0047/B7')
    broken('qtl-missing-rate','missing-rate','; rate Office.long per_start 3','',None,
           'T10. Fee used by BOTH plans absent,not0. Both cost null/not_configured,lead8/5. Undetermined pair(AsIs,ToBeP),both non_dominated,S/0.',defer='ADR0047/B7')
    deferred['qtl-missing-rate.broken-missing-rate']['semantic_oracle']={'lead':{'AsIs':'8','ToBeP':'5'},'cost_reason':{'AsIs':'not_configured','ToBeP':'not_configured'},'non_dominated':['AsIs','ToBeP'],'undetermined_pairs':[['AsIs','ToBeP']]}

    nullobj='''model QtlNullObjective version "0.2" {
  type V = enum { pass, fail }
  actor A { capacity 1; capability run() -> V { duration fixed 2min; result nondet { pass, fail } } }
  actor B { capacity 1; capability run() -> V { duration fixed 4min; result nondet { pass } } }
  process P { v = A.run(); if v == pass { end done } else { end returned } }
  process Q { v = B.run(); if v == pass { end done } else { end returned } }
  cut C { include [A,B] }
  configuration Maybe { cut C; process P }
  configuration Sure { cut C; process Q }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
  property case.Done = reached(end.done)
''' + metric() + '''  compare [Maybe, Sure] cut C objectives { minimize quantile.Median }
}
'''
    add('qtl-null-objective',nullobj,None,'T21. Safety S for both;no hard. Maybe value null/beyond_reached_mass LB2,Sure4exact. Both admissible/non_dominated,undetermined pair. LB2 is NOT an objective value. S/0.',
        invocation={'command':'compare','plans':['Maybe','Sure'],'mode':'exhaustive'},defer='ADR0047/B7')
    deferred['qtl-null-objective']['semantic_oracle']={'quantiles':{'Maybe':missing('beyond_reached_mass',2)[:1],'Sure':two(4)[:1]},'non_dominated':['Maybe','Sure'],'undetermined_pairs':[['Maybe','Sure']]}
    broken('qtl-null-objective','pass-only','result nondet { pass, fail }','result nondet { pass }',None,
           'T21. Maybe now2exact,Sure4exact. Maybe dominates;non_dominated[Maybe],undetermined_pairs empty,S/0.',defer='ADR0047/B7')
    deferred['qtl-null-objective.broken-pass-only']['semantic_oracle']={'quantiles':{'Maybe':two(2)[:1],'Sure':two(4)[:1]},'non_dominated':['Maybe'],'undetermined_pairs':[]}
    interval='''model QtlIntervalObjective version "0.2" {
  type V = enum { pass, fail }
  actor A { capacity 1; capability run() -> V { duration fixed 3min; result nondet { pass } } }
  actor B { capacity 2; capability run() -> V { duration fixed 4min; result nondet { pass } } }
  process P { v = A.run(); if v == pass { end done } else { end returned } }
  process Q { v = B.run(); if v == pass { end done } else { end returned } }
  cut C { include [A,B] }
  configuration Maybe { cut C; process P }
  configuration Sure { cut C; process Q }
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 2; clock 10min }
  verify safety no_resource_overcommit
  property case.Done = reached(end.done)
''' + metric() + '''  compare [Maybe, Sure] cut C objectives { minimize quantile.Median }
}
'''
    add('qtl-interval-objective',interval,None,
        'T21 interval supplement. Maybe no FIFO has per-case median[3,6] relaxed;Sure both arrivals finish4,median4exact. Intervals overlap,so neither certainly dominates. Both admissible/non_dominated,S/0; do not compare only lower bounds.',
        invocation={'command':'compare','plans':['Maybe','Sure'],'mode':'exhaustive'},defer='ADR0047/B7')
    deferred['qtl-interval-objective']['semantic_oracle']={'quantiles':{'Maybe':two((3,6),basis='per_case_relaxation')[:1],'Sure':two(4)[:1]},'non_dominated':['Maybe','Sure']}
    broken('qtl-interval-objective','fifo','  arrivals fixed [0min, 0min]','  arrivals fixed [0min, 0min]\n  policy fifo tie_by case_id',None,
        'T21 interval supplement. Only FIFO changes:Maybe becomes3exact,Sure4exact;Maybe certainly dominates. non_dominated[Maybe],S/0.',defer='ADR0047/B7')
    deferred['qtl-interval-objective.broken-fifo']['semantic_oracle']={'quantiles':{'Maybe':two(3)[:1],'Sure':two(4)[:1]},'non_dominated':['Maybe']}
    broken('qtl-objective','quantile-hard','hard { cohort.OnTimePerArrival >= 0.5 }','hard { quantile.Median <= 0.5 }',None,
        'T12. Quantile hard remains outside B7 scope. U-SYN-0 hard constraints apply to cohort ratios only,U/4.',
        verdict='unsupported',code='U-SYN-0',message='hard constraints apply to cohort ratios only',
        invocation={'command':'compare','plans':['One','Two'],'mode':'exhaustive'},defer='ADR0047/B7')

    for name,row in rows.items():
        directory=ROOT/'fixtures'/name
        directory.mkdir(exist_ok=True)
        (directory/'calc.md').write_text('# ADR0049 hand calculation\n\n'+row['calc']+'\n\nTimes are arrival-relative integer minutes. Numerical expectations were authored before implementation, not captured. Stage C RED is not verification.\n')
        (directory/'expected.json').write_text(json.dumps(row['expected'],indent=2)+'\n')
        (directory/'model.relca').write_text(row['source'])
        if row['invocation']:
            (directory/'invocation.json').write_text(json.dumps(row['invocation'],indent=2)+'\n')
    names=sorted(rows)
    (HERE/'quantile_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (HERE/'quantile_names.rs').write_text('[\n'+''.join('    '+json.dumps(n)+',\n' for n in names)+']\n')
    (HERE/'quantile_inventory.json').write_text(json.dumps({n:{k:v for k,v in rows[n].items() if k!='source'} for n in names},indent=2)+'\n')
    (HERE/'quantile_pairs.json').write_text(json.dumps(pairs,indent=2)+'\n')
    (HERE/'quantile_deferred.json').write_text(json.dumps(deferred,indent=2)+'\n')
    print(f'{len(names)} installed hand fixtures; {len(deferred)} deferred; {len(pairs)} edit witnesses')


if __name__=='__main__':
    materialize()
