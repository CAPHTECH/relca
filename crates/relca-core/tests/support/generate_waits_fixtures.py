"""ADR0048 hand arithmetic materializer. Does not execute/import Relca.

Every number below is an interval length, integral, or rational quotient fixed
before implementation. Regeneration only writes the closed wait-* population.
"""
import copy
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
ROWS = {}
PAIRS = []
CAUSES = ('actor_capacity', 'resource', 'executor', 'combined', 'unattributed')


def replace(s, old, new):
    t = s.replace(old, new)
    assert t != s, (old, new)
    return t


def base(n):
    return (ROOT / 'fixtures' / n / 'model.relca').read_text()


def add(s, text):
    i = s.rfind('}')
    return s[:i] + text + '\n' + s[i:]


def enabled(s, window=20):
    if 'metrics {' not in s:
        return add(s, f'metrics {{ window {window}min; currency "JPY"; breakdown waits }}')
    out = re.sub(r'(metrics\s*\{[^}]*)(})', r'\1; breakdown waits \2', s)
    assert out != s
    return out


def scalar(x=0, hi=None, reason=None, sim=False):
    if reason:
        out = dict(min=None, max=None, reason=reason)
    else:
        out = dict(min=str(x), max=str(x if hi is None else hi), reason=None)
    if sim:
        assert hi is None, 'sample constants only in these deterministic fixtures'
        out.update(mean=None if reason else str(x), samples=0 if reason else 4)
    return out


def breakdown(reason=None, **values):
    return {**{k: scalar(values.get(k, 0), reason=reason) for k in
               ('external', 'in_service', 'contest_route', 'intentional', 'unattributed')},
            'queue': {k: scalar(values.get(k, 0), reason=reason) for k in CAUSES[:-1]}}


def causes(reason=None, **values):
    return {k: scalar(values.get(k, 0), reason=reason) for k in CAUSES}


def task(binding, predecessor=0, reason=None, **values):
    return dict(binding=binding, predecessor_wait=scalar(predecessor),
                wait_causes=causes(reason, **values))


def terminal(end='done', reason=None, **values):
    return dict(end=end, breakdown=breakdown(reason, **values))


def case(i, terminals, tasks, joins=None, dwell=None):
    c = dict(case_id=i, terminals=terminals, tasks=tasks, joins=joins or [])
    if dwell is not None:
        c['dwell'] = dict(until=scalar(3), breakdown=dwell, bound='lower')
    return c


def actor(name, total=0, maximum=0, average=0, lower=False):
    def observed(x):
        q = dict(stats=scalar(x))
        if lower:
            q['bound'] = 'lower'
        return q
    return dict(actor=name, queue=dict(wait_total=observed(total), length_max=scalar(maximum),
                                      length_time_average=observed(average)))


def join(fork='f', mode='all', a=0, b=0):
    return [dict(fork=fork, mode=mode, branches=[
        dict(binding=k, join_wait=scalar(x, reason='join_any' if mode == 'any' else None))
        for k, x in [('a', a), ('b', b)]])]


def oracle(cases, actors, overall=0):
    return dict(cases=cases, actors=actors, overall=dict(queue_wait_observed=scalar(overall)))


def put(name, source, hand, calc, verdict='satisfied', invocation=None, static=None):
    assert name not in ROWS
    assert name.startswith('wait-') and '/' not in name
    source = '\n'.join(line.rstrip() for line in source.splitlines()) + '\n'
    ROWS[name] = dict(source=source, hand=hand, calc=calc, verdict=verdict,
                      invocation=invocation or dict(command='check'), static=static)


def pair(name, fault, source, other, hand, other_hand, calc, **options):
    assert source != other
    put(name, source, hand, calc, **options)
    broken = name + '.broken-' + fault
    put(broken, other, other_hand, calc, **options)
    PAIRS.append(dict(base=name, broken=broken))
    return broken


def fifo(wide=False, resource=False, removed=False):
    cs = []
    for i in range(3):
        ac = (0, 0, 1)[i] if removed else 2*i
        bc = (0, 4, 7)[i] if removed else 0 if wide else 2*i
        vals = dict(in_service=8, actor_capacity=(ac+bc if not resource or removed else bc))
        if resource and not removed:
            vals['resource'] = ac
        cs.append(case(i+1, [terminal(**vals)], [
            task('a', **{('resource' if resource and not removed else 'actor_capacity'): ac}),
            task('b', predecessor=(3, 3, 4)[i] if removed else 3+2*i, actor_capacity=bc)]))
    aa = actor('A', 1, 1, '1/18') if removed else actor('A', 6, 2, '3/7' if wide else '1/3')
    bb = actor('B', 11, 2, '11/18') if removed else actor('B', 0 if wide else 6, 0 if wide else 1, 0 if wide else '1/3')
    return oracle(cs, [aa, bb], 6 if wide else 12)


FIFO = enabled(base('met-fifo'))
pair('wait-fifo', 'wide-b', FIFO, replace(FIFO, 'actor B { capacity 1', 'actor B { capacity 2'),
     fifo(), fifo(True), 'T01: arrivals 0,1,2. A [0,3),[3,6),[6,9). B [3,8),[8,13),[13,18). '
     'Lead 8,12,16 = service8 + queue0,4,8. Each Actor integral6; T18 => 1/3. '
     'Wide B starts3,6,9 and ends8,11,14: A integral6/T14=3/7, B0. Predecessor b=3,5,7.')
PERSON = enabled(base('met-person'))
person_hand = oracle([case(1, [terminal(in_service=5)], [task('a'), task('b')], join(a=2))], [actor('A'), actor('B')])
shared = oracle([case(1, [terminal(in_service=8)], [task('a'), task('b')], join())], [actor('A'), actor('B')])
shared['cases'][0]['tasks'][0]['wait_causes']['executor'] = scalar(0, 5)
shared['cases'][0]['tasks'][1]['wait_causes']['executor'] = scalar(0, 3)
for idx, hi in enumerate((5, 3)):
    shared['cases'][0]['joins'][0]['branches'][idx]['join_wait'] = scalar(0, hi)
    shared['actors'][idx]['queue'] = dict(wait_total=dict(stats=scalar(0, hi)), length_max=scalar(0, 1),
                                         length_time_average=dict(stats=scalar(0, f'{hi}/8')))
shared['overall']['queue_wait_observed'] = scalar(3, 5)
pair('wait-person', 'shared-person', PERSON, replace(PERSON, 'executors [q]', 'executors [p]'),
     person_hand, shared, 'T02: independent [0,3),[0,5), End5 work8 => service5 and joins2,0. '
     'Shared p gives a0..3/b3..8 or b0..5/a5..8. Service8 always; Task executor extrema a0..5,b0..3. '
     'Same-leaf total is3 or5, not marginal sum0..8. Average extrema a0..5/8,b0..3/8. '
     'Inline join controls: an observed-false outer branch keeps lexical f/a/b but never creates either Task, '
     'so End0 and both join_wait null/never_created. Horizon4 instead observes a.stop3 and b still running; '
     'both join waits null/unfinished, dwell service[0,4)=4, never a forecast of long.stop5 or join2.')
EXTERNAL = enabled(base('lim-concurrent-wait'))
external_hand = oracle([case(i+1, [terminal(external=2*i, in_service=2)], [task('r')]) for i in range(3)], [actor('A')])
external_short = oracle([
    case(1, [terminal(in_service=2)], [task('r')]),
    case(2, [terminal(reason='no_terminal')], [task('r')], dwell=breakdown(external=2, in_service=1)),
    case(3, [terminal(reason='no_terminal')], [dict(binding='r', predecessor_wait=scalar(reason='never_created'),
         wait_causes=causes('never_created'))], dwell=breakdown(external=3))], [actor('A', lower=True)])
external_day = replace(EXTERNAL, 'arrivals fixed [0min, 0min, 0min]', '')
external_day = replace(external_day, 'bounds { cases 3; live_tasks 1; clock 20min }',
                       'scenario Day { arrivals fixed [0min, 0min, 0min]; horizon 3min; bounds { cases 3; live_tasks 1; clock 20min } }')
pair('wait-external', 'horizon', EXTERNAL, external_day, external_hand, external_short,
     'T03: receipts0, admissions0,2,4; run0..2/2..4/4..6. External0,2,4; predecessor0. '
     'Horizon3 completes case1; case2 dwell external2+service1, case3 external3. Queue0 lower; '
     'terminal remains no_terminal. Response deadline6 exceeds3; no computational cut.')
ROWS['wait-external.broken-horizon'].update(verdict='inconclusive', invocation=dict(command='check', scenario='Day'))
RESOURCE = replace(FIFO, 'actor A { capacity 1', 'resource Desk capacity 1;\n actor A { capacity 2')
RESOURCE = replace(RESOURCE, 'capability prep() -> T {', 'capability prep() -> T { uses Desk:1;')
pair('wait-resource', 'no-resource', RESOURCE, replace(RESOURCE, 'uses Desk:1;', ''), fifo(resource=True), fifo(resource=True, removed=True),
     'T04: Desk serializes A despite capacity2: same intervals as T01, a causes resource0,2,4; '
     'b causes actor0,2,4. Removing uses only: A0..3,1..4,3..6 (not2..5: two slots full), '
     'B3..8,8..13,13..18. a actor0,0,1; b actor0,4,7; predecessor b3,3,4. '
     'Integrals A1/B11; T18 =>1/18,11/18. Both leads8,12,16 and overall12 unchanged.')
COMBINED = '''model WaitCombined version "0.2" {
type T = enum { pass }
resource Desk capacity 1;
actor A { capacity 1; capability prep() -> T { duration fixed 3min; result nondet { pass } } }
actor B { capacity 2; capability finish() -> T { uses Desk:1; duration fixed 5min; result nondet { pass } } }
process P { fork f { a = A.prep(); b = B.finish() }; join all f; end done }
cut C { include [A, B] }
arrivals fixed [0min, 0min]
policy fifo tie_by case_id
bounds { cases 2; live_tasks 2; clock 20min }
metrics { window 20min; currency "JPY"; breakdown waits }
verify reachable end.done
verify safety no_resource_overcommit
}
'''
def combined(same=False):
    return oracle([case(1, [terminal(in_service=5)], [task('a'), task('b')], join(a=2)),
                   case(2, [terminal(in_service=7, **{'actor_capacity' if same else 'combined': 3})],
                        [task('a', actor_capacity=3), task('b', **{'actor_capacity' if same else 'resource': 5})], join(a=4))],
                  [actor('A', 3, 1, '3/10'), actor('B', 5, 1, '1/2')], 8)
same = replace(COMBINED, 'capacity 2;', 'capacity 1;')
same = replace(same, 'uses Desk:1;', '')
same = replace(same, 'resource Desk capacity 1;', '')
pair('wait-combined', 'same-cause', COMBINED, same, combined(), combined(True),
     'T05: case1 a0..3,b0..5 End5; case2 a3..6,b5..10 End10. '
     'Case2 [0,3) has two different causes => combined3, service[3,10)=7. '
     'Task waits a3/b5; actor integrals3,5 /T10 =>3/10,1/2. '
     'Control replaces resource serialization with Actor serialization, same intervals, case actor3.')
INTENTIONAL = replace(FIFO, 'arrivals fixed [0min, 1min, 2min]', '')
INTENTIONAL = replace(INTENTIONAL, 'cases 3;', 'cases 1;')
INTENTIONAL = replace(INTENTIONAL, 'b = B.finish()', 'wait 2min; b = B.finish()')
def intentional(wait=True):
    return oracle([case(1, [terminal(in_service=8, intentional=2 if wait else 0)],
                        [task('a'), task('b', predecessor=5 if wait else 3)])], [actor('A'), actor('B')])
pair('wait-intentional', 'no-wait', INTENTIONAL, replace(INTENTIONAL, 'wait 2min;', ''), intentional(), intentional(False),
     'T06: a0..3, explicit wait3..5, b5..10. Lead10=service8+intentional2; predecessor b5. '
     'Remove wait: b3..8, lead8, intentional0, predecessor3. No Actor queue in either input.')
ANY = enabled(base('any-drain'), 5)
ALL = re.sub(r'join any checks where result == pass\s+tie by \[a, b\]\s+remaining cancel_then_drain\s+if none -> no_match', 'join all checks', ANY)
assert ALL != ANY
def any_hand(all_mode=False):
    return oracle([case(1, [terminal(in_service=5)], [task('a'), task('b')],
                        join('checks', 'all' if all_mode else 'any', a=2))], [actor('Office')])
pair('wait-any', 'all', ANY, ALL, any_hand(), any_hand(True),
     'T07: short0..3,long0..5; forbidden_after_start drains to5. End5, result_at3, work8, '
     'service5. Any branches both null/join_any, not a fabricated status. All branches2,0.')


def invalidated(h, reason='truncated'):
    out = copy.deepcopy(h)
    def walk(v):
        if isinstance(v, dict):
            v.pop('bound', None)
            v.pop('dwell', None)
            if set(('min', 'max', 'reason')) <= v.keys():
                v.update(min=None, max=None, reason=reason)
            else:
                for x in v.values():
                    walk(x)
        elif isinstance(v, list):
            for x in v:
                walk(x)
    walk(out)
    return out


put('wait-fifo.broken-short-clock', replace(FIFO, 'clock 20min', 'clock 12min'), invalidated(fifo()),
    'T08: clock12 truncates while B case2 and queued case3 remain. All NEW scalars null/truncated, '
    'including the completed case1. No dwell or bound; window20 is unchanged.', verdict='inconclusive')
PAIRS.append(dict(base='wait-fifo', broken='wait-fifo.broken-short-clock'))
put('wait-person.broken-short-clock', replace(PERSON, 'clock 10min', 'clock 4min'), invalidated(person_hand),
    'T08 fork control: clock4 < long.stop5; both join_wait scalars null/truncated, not just joins=[].', verdict='inconclusive')
PAIRS.append(dict(base='wait-person', broken='wait-person.broken-short-clock'))
CONTEST = enabled(base('sov-sleep'))
no_contest = re.sub(r'contract Controls \{ contest[^\n]*\}', '', CONTEST)
assert no_contest != CONTEST
def contest_hand(route=True):
    t = terminal('posted', in_service=3, intentional=5)
    if route:
        t['breakdown']['intentional'] = scalar(4, 5)
        t['breakdown']['contest_route'] = scalar(0, 1)
    return oracle([case(1, [terminal('returned', in_service=2), t], [task('approval'), task('ack', predecessor=2)])],
                  [actor('Approver'), actor('Ledger')] + ([actor('Clerk')] if route else []))
pair('wait-contest', 'no-contest', CONTEST, no_contest, contest_hand(), contest_hand(False),
     'T09: approve0..2; fail returns2. Pass posts2..3 then waits3..8, posted8. '
     'Filed recheck3..4 subtracts1 from intentional, not service: service3, route0..1, intentional4..5. '
     'Each leaf sums8; marginal maxima sum9 is not a valid sum assertion. No contest: route0,intentional5; W-CONTEST-0.')
label = RESOURCE
for old, new in [('A', 'Zeta'), ('B', 'Alpha'), ('Desk', 'Q9'), ('prep', 'z1'), ('finish', 'a0')]:
    other = re.sub(r'\b' + old + r'\b', new, label)
    assert other != label
    label = other
label_hand = fifo(resource=True)
label_hand['actors'][0]['actor'] = 'Zeta'
label_hand['actors'][1]['actor'] = 'Alpha'
pair('wait-label', 'original-labels', label, RESOURCE, label_hand, fifo(resource=True),
     'T10: bijective relabeling A/Zeta,B/Alpha,Desk/Q9,prep/z1,finish/a0. Intervals exactly T04. '
     'Actor declaration order Zeta,Alpha must not be sorted. Source hashes/spans and named leaves may differ, numbers may not.')

for fault, replacement in [('queue-quantiles','breakdown queue_quantiles'), ('missing','breakdown'),
                           ('duplicate','breakdown waits; breakdown waits'), ('waits-by-cause','waits by_cause'),
                           ('quantiles','quantiles'), ('p95','p95')]:
    s = replace(FIFO, 'breakdown waits', replacement)
    put('wait-fifo.broken-' + fault, s, None,
        'T11: only metrics item changed. Unknown/missing words U-SYN-0; duplicate U-SEM-0 at second item. '
        'No execution or metrics is allowed. Existing waits/quantiles/p95 error messages remain byte-identical.',
        verdict='unsupported', static='U-SEM-0' if fault == 'duplicate' else 'U-SYN-0')
    PAIRS.append(dict(base='wait-fifo', broken='wait-fifo.broken-' + fault))
outside = replace(FIFO, '; breakdown waits', '')
outside = add(outside, 'breakdown waits')
put('wait-fifo.broken-outside', outside, None, 'T11: breakdown outside metrics is U-SYN-0, not a model declaration.',
    verdict='unsupported', static='U-SYN-0')
PAIRS.append(dict(base='wait-fifo', broken='wait-fifo.broken-outside'))

HQ = '''model HQ version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A { capacity 1; capability run() -> T { duration fixed 5min; result nondet { ok }; writes hit; finish { hit = true } } }
process P { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id
scenario Day { arrivals fixed [0min, 0min]; horizon 3min; bounds { cases 2; live_tasks 1; clock 20min } }
metrics { window 20min; currency "JPY"; breakdown waits }
verify reachable end.done
}
'''
hq_hand = oracle([case(1, [terminal(reason='no_terminal')], [task('r')], dwell=breakdown(in_service=3)),
                  case(2, [terminal(reason='no_terminal')], [task('r', reason='unfinished')], dwell=breakdown(actor_capacity=3))],
                 [actor('A', 3, 1, 1, True)], 3)
hqc = replace(replace(HQ, 'horizon 3min', 'horizon 30min'), 'clock 20min', 'clock 3min')
day = dict(command='check', scenario='Day')
pair('wait-horizon-queue', 'clock', HQ, hqc, hq_hand, invalidated(hq_hand),
     'T12: case1 runs[0,3) observed prefix, case2 queues[0,3). Integral3, max1, avg3/3=1 lower. '
     'Both terminal breakdown no_terminal; dwell service3 or actor3. Queued Task wait/causes unfinished, predecessor0. '
     'Clock control stops at same3 but computationally: ALL new scalars truncated, dwell/bound absent.',
     verdict='inconclusive', invocation=day)


def sampled(h):
    out = copy.deepcopy(h)
    def walk(v):
        if isinstance(v, dict):
            if set(('min', 'max', 'reason')) <= v.keys():
                assert v['min'] == v['max']
                v.update(mean=v['min'], samples=0 if v['min'] is None else 4)
                if v['reason'] == 'no_terminal':
                    v['reason'] = 'no_samples'
            else:
                for x in v.values(): walk(x)
        elif isinstance(v, list):
            for x in v: walk(x)
    walk(out)
    return out


SAMPLE = replace(HQ, 'horizon 3min', 'horizon 12min; simulation replications 4 seed 1 confidence 0.5')
SAMPLE = replace(SAMPLE, 'verify reachable end.done', 'verify probability reached hit == true >= 0.5')
sample_hand = oracle([case(1, [terminal(in_service=5)], [task('r')]),
                      case(2, [terminal(in_service=5, actor_capacity=5)], [task('r', actor_capacity=5)])], [actor('A',5,1,'1/2')],5)
pair('wait-sample', 'short-horizon', SAMPLE, replace(SAMPLE, 'horizon 12min', 'horizon 3min'),
     sampled(sample_hand), sampled(hq_hand),
     'T13: four deterministic replications End5/10. Case2 queue5, Actor integral5/T10=1/2. '
     'm2, alpha=(1-1/2)/(2*2)=1/8; hits4 tail1/16 <=1/8 => satisfied. '
     'D3: all four replications cut_off, unknown4 not failure0, inconclusive. Dwell until3 samples4; '
     'Actor3/max1/avg1 lower. Terminal samples0/no_samples, unstarted causes unfinished.',
     invocation=dict(command='simulate', scenario='Day'))
ROWS['wait-sample.broken-short-horizon']['verdict'] = 'inconclusive'
ONE = replace(HQ, 'scenario Day { arrivals fixed [0min, 0min]; horizon 3min; bounds { cases 2; live_tasks 1; clock 20min } }',
              'arrivals fixed [0min, 0min]; bounds { cases 2; live_tasks 1; clock 20min }')
ONE = replace(ONE, 'actor A {', 'resource Desk capacity 1;\nactor A {')
ONE = replace(ONE, 'capability run() -> T {', 'capability run() -> T { uses Desk:1;')
def one_hand(wide=False):
    cause = 'resource' if wide else 'combined'
    h = copy.deepcopy(sample_hand)
    h['cases'][1]['terminals'][0] = terminal(in_service=5, **{cause:5})
    h['cases'][1]['tasks'][0] = task('r', **{cause:5})
    return h
pair('wait-one-task-combined', 'wide-actor', ONE, replace(ONE, 'actor A { capacity 1', 'actor A { capacity 2'),
     one_hand(), one_hand(True), 'T14: run0..5/5..10. Case2 waits5 with Actor AND Desk occupied => combined5. '
     'Wide Actor2 removes only Actor shortage: resource5 instead, same lead10 and queue integral5/T10=1/2.')
REJECT = replace(ONE, 'resource Desk capacity 1;', '')
REJECT = replace(REJECT, 'uses Desk:1;', '')
REJECT = replace(REJECT, 'duration fixed 5min', 'duration fixed 2min')
REJECT = replace(REJECT, 'verify reachable end.done', 'verify safety no_resource_overcommit')
REJECT = add(REJECT, 'queue capacity 1 on_overflow reject')
rejected = oracle([case(1,[terminal(in_service=2)],[task('r')]),
                   case(2,[terminal(reason='no_terminal')],[dict(binding='r',wait_causes=causes('never_created'),predecessor_wait=scalar(reason='never_created'))])], [actor('A')])
overflow = copy.deepcopy(rejected)
overflow['cases'][1]['terminals'].append(terminal('spill'))
pair('wait-reject', 'overflow', REJECT, replace(REJECT, 'on_overflow reject', 'on_overflow end spill'),
     rejected, overflow, 'T15: capacity1 counts admitted case1 at0; case2 is rejected before Task creation. '
     'Case1 run0..2. Rejected breakdown null/rejected; all done rows for case2 no_terminal. '
     'Overflow control records spill at receipt0 with lead0 and all breakdown0, no fake task or dwell.')
ZERO = '''model ZeroObservation version "0.2" {
type T = enum { ok }
observed case { flag: bool = false }
actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }
process P { if flag == true { r = A.run(); end unused } else { end done } }
cut C { include [A] }
bounds { cases 1; live_tasks 1; clock 10min }
metrics { window 10min; currency "JPY"; breakdown waits }
verify reachable end.done
}
'''
def zero_hand(wait=False):
    h = oracle([case(1,[terminal('unused',reason='no_terminal'),terminal(intentional=1 if wait else 0)],
        [dict(binding='r',wait_causes=causes('never_created'),predecessor_wait=scalar(reason='never_created'))])], [actor('A')])
    if not wait:
        h['actors'][0]['queue']['length_time_average']['stats'] = scalar(reason='no_samples')
    return h
pair('wait-zero-window-observation', 'wait', ZERO, replace(ZERO, 'else { end done }', 'else { wait 1min; end done }'),
     zero_hand(),zero_hand(True),'T16: flag false has immediate End0; lexical unused Task keeps Actor A inventory. '
     'T0 integral0,max0, average null/no_samples (not window10). Add wait1 => intentional1, T1, average0. '
     'Dead branch remains no_terminal and never_created.')
CONFIG = replace(FIFO, 'process P { a = A.prep(); b = B.finish(); end done }',
                 'process P { a = A.prep(); b = B.finish(); end done }\nprocess Q { a = A.prep(); b = B.finish(); end done }')
CONFIG = add(CONFIG, 'configuration Left { cut C; process P }\nconfiguration Right { cut C; process Q }')
pair('wait-compare-diff', 'no-breakdown', CONFIG, replace(CONFIG,'breakdown waits','               '),
     fifo(),None,'T17: Left and Right are identical P/Q copies of T01. Embedded reports carry T01 values. '
     'Compare/diff metric tables MUST equal opt-out tables; no new metric rows. Check both configurations; '
     'resource variant separately ensures checked vocabulary validation.', invocation=dict(command='compare',plans=['Left','Right'],mode='exhaustive'))
pair('wait-no-policy-sample', 'no-policy', SAMPLE, replace(SAMPLE,'policy fifo tie_by case_id',''),
     sampled(sample_hand),None,'T18: FIFO is deterministic and T13 applies. Without FIFO, simultaneous eligible starts '
     'are nondeterministic and simulation must refuse U-SEM-0/unsupported, not supply uniform choices or zero waits.',
     invocation=dict(command='simulate',scenario='Day'))
ROWS['wait-no-policy-sample.broken-no-policy'].update(verdict='unsupported', static='U-SEM-0')
DURATION = replace(REJECT,'queue capacity 1 on_overflow reject','')
DURATION = replace(DURATION,'arrivals fixed [0min, 0min]','')
DURATION = replace(DURATION,'cases 2;','cases 1;')
DURATION = replace(DURATION,'duration fixed 2min','duration { 2min: 0.5, 4min: 0.5 }')
DURATION = replace(DURATION,'verify safety no_resource_overcommit','verify probability reached hit == true within 4min >= 1')
duration_hand = oracle([case(1,[terminal(in_service=2)],[task('r')])],[actor('A')])
duration_hand['cases'][0]['terminals'][0]['breakdown']['in_service'] = scalar(2,4)
duration_zero = copy.deepcopy(duration_hand)
duration_zero['cases'][0]['terminals'][0]['breakdown']['in_service'] = scalar(2)
pair('wait-duration-support','zero-mass',DURATION,replace(DURATION,'2min: 0.5, 4min: 0.5','2min: 1, 4min: 0'),
     duration_hand,duration_zero,'T19: positive duration support2,4 gives service/lead[2,4], not expectation3. '
     'Moving all mass to2 excludes zero-probability4 entirely =>[2,2]. No queue, wait or join.')


def main():
    import sys
    names = sorted(ROWS)
    if sys.argv[1:] == ['--names-only']:
        (HERE/'waits_names.json').write_text(json.dumps(names,indent=2)+'\n')
        (HERE/'waits_names.rs').write_text('&[\n'+''.join('    '+json.dumps(n)+',\n' for n in names)+']\n')
        return
    assert not sys.argv[1:]
    for name, r in ROWS.items():
        d = ROOT / 'fixtures' / name
        d.mkdir(exist_ok=True)
        (d/'calc.md').write_text('# ' + name + '\n\nADR 0048; design v0.2 §22.3. Units: minutes.\n\n'
                                 + r['calc'] + '\n\n[R] Hand interval arithmetic, not captured collector output. '
                                 'Stage C acceptance stops RED; production fault calibration not performed.\n')
        (d/'model.relca').write_text(r['source'])
        expected = dict(verdict=r['verdict'])
        if r['static']:
            expected['diagnostics'] = [dict(code=r['static'])]
        elif r['invocation']['command'] != 'compare':
            # Ordinary clock cuts carry no W-BUDGET-0 (the pre-A7 binary gives no diagnostics on the masked clock substrates).
            expected['absent_diagnostics'] = ['W-DEADLOCK-0', 'W-BUDGET-0']
            codes = []
            if name.startswith(('wait-external','wait-any')):
                codes.append('W-CUT-0')
            if name == 'wait-contest.broken-no-contest':
                codes.append('W-CONTEST-0')
            if codes:
                expected['diagnostics'] = [dict(code=c) for c in codes]
        if r['hand'] and r['invocation']['command'] != 'compare':
            expected['metrics'] = dict(overall=r['hand']['overall'])
        (d/'expected.json').write_text(json.dumps(expected,indent=2)+'\n')
        if r['invocation'] != dict(command='check'):
            (d/'invocation.json').write_text(json.dumps(r['invocation'],indent=2)+'\n')
    (HERE/'waits_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (HERE/'waits_names.rs').write_text('&[\n'+''.join('    '+json.dumps(n)+',\n' for n in names)+']\n')
    (HERE/'waits_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    data = {n: {k:r[k] for k in ('hand','verdict','invocation','static')} for n,r in sorted(ROWS.items())}
    (HERE/'waits_oracles.json').write_text(json.dumps(data,indent=2)+'\n')
    print(f'{len(names)} closed wait inputs, {len(PAIRS)} transformations; no binary used')


if __name__ == '__main__':
    main()
