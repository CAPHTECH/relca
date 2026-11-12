"""ADR0042 hand-oracle materializer. No Relca binary or output is read.

Run with --write to materialize the closed scn-* population, or --check to
verify it. All substitutions assert that the edit actually happened. The
arithmetic below is FIFO interval arithmetic, not the production scheduler.
"""
import argparse
from fractions import Fraction
import json
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
SUPPORT = Path(__file__).resolve().parent
SIM = 'simulation replications 4 seed 1 confidence 0.5'
QUERY = 'verify probability reached hit == true within 4min >= 1'
REACH = 'verify reachable end.done'
POLICY = 'policy fifo tie_by case_id'
METRICS = '''metrics {
  window 30min
  currency "JPY"
  rate A per_minute 1
  rate A.run per_start 0
}'''
BODY = '''type T = enum { ok }
observed case { hit: bool = false }
actor A {
  capacity 1
  capability run() -> T {
    duration fixed 3min
    result nondet { ok }
    writes hit
    finish { hit = true }
  }
}
process Flow { r = A.run(); end done }
cut C { include [A] }
'''


def edit(s, old, new, count=1):
    assert s.count(old) == count, (old, s.count(old), count)
    t = s.replace(old, new)
    assert t != s, old
    return t


def scenario(name='Light', arrival='arrivals fixed_every 4min during [0min, 12min)',
             cases=3, clock=30, horizon=None, extra=''):
    h = '' if horizon is None else f'  horizon {horizon}min\n'
    arrival_line = f'  {arrival}\n' if arrival else ''
    return (f'scenario {name} {{\n{arrival_line}{h}'
            f'  bounds {{ cases {cases}; live_tasks 1; clock {clock}min }}\n{extra}\n}}\n')


LIGHT = scenario()
BUSY = scenario('Busy', 'arrivals fixed_every 1min during [0min, 3min)')
S = 'model M version "0.2" {\n' + BODY + POLICY + '\n' + LIGHT + BUSY + QUERY + '\n}\n'
ONE = edit(S, BUSY, '')
H = ('model M version "0.2" {\n' + edit(BODY, 'fixed 3min', 'fixed 5min') + POLICY + '\n'
     + scenario('Day', 'arrivals fixed_every 2min during [0min, 6min)', horizon=12)
     + METRICS + '\n' + REACH + '\n}\n')


def rg(x):
    return {'min': x, 'max': x}


def scalar(x, sampled=False, reason=None):
    d = {'min': None if x is None else str(x), 'max': None if x is None else str(x),
         'reason': reason}
    if sampled:
        d.update(mean=None if x is None else str(x), samples=0 if x is None else 4)
    return d


def fold(vs):
    return next((v for v in ['violated', 'inconclusive'] if v in vs), 'satisfied')


def oracle(arrivals=(0, 4, 8), service=3, horizon: int | None=None, clock=30,
           query='within', within=4, response: int | None=None, sampled=False,
           selected: str | None='Light', form='fixed_every', every=4, until=12,
           metrics=False, price=True, legacy=False):
    """Single-server FIFO, no cancellations. Inclusive horizon, old clock cut.

    start_i=max(a_i, finish_(i-1)); finish_i=start_i+s. Work at D is
    max(0,min(finish_i,D)-start_i). A clock cut does NOT advance the state.
    """
    starts, finishes = [], []
    for a in arrivals:
        starts.append(max(a, finishes[-1] if finishes else 0))
        finishes.append(starts[-1] + service)
    stopped = finishes[-1] > min(clock, horizon if horizon is not None else clock)
    cut = stopped and horizon is not None and horizon <= clock
    truncated = stopped and not cut
    limit = horizon if horizon is not None and cut else clock if truncated else finishes[-1]
    actual = min(finishes[-1], limit)
    kinds = ['end' if f <= limit else 'cut_off' if cut else None for f in finishes]
    work = [service if k == 'end' else max(0, limit - st) if k == 'cut_off' or sampled and truncated else 0
            for k, st in zip(kinds, starts)]
    states, probs = [], []
    for a, f, k in zip(arrivals, finishes, kinds):
        if response is not None:
            state = ('satisfied' if f <= limit and f <= a + response else
                     'violated' if a + response <= limit else 'inconclusive')
            prob = None
        elif query == 'within':
            p = '1' if f <= min(limit, a + within) else '0' if a + within <= limit or k == 'end' else None
            state = 'inconclusive' if p is None else 'satisfied' if p == '1' else 'violated'
            prob = {'min': p or '0', 'max': p or '1'}
        else:
            state = 'satisfied' if k == 'end' else 'inconclusive'
            prob = rg('1') if k == 'end' else {'min': '0', 'max': '1'}
        states.append(state)
        probs.append(prob)
    verdict = fold(states)
    prop_id = ('contract K #1 response' if response is not None else
               'reachable end.done' if query == 'reachable' else 'probability reached #1')
    prop = {'id': prop_id, 'verdict': verdict, 'cases': []}
    outcomes = []
    for i, (a, f, k, w, state, p) in enumerate(zip(arrivals, finishes, kinds, work, states, probs), 1):
        pr = {'case_id': i, 'arrival': a, 'verdict': state}
        if query != 'reachable' and response is None:
            if sampled:
                pr.update(basis='statistical', hits=4 if state == 'satisfied' else 0,
                          unknown=4 if state == 'inconclusive' else 0, trials=4)
                if state == 'inconclusive':
                    pr['interval'] = {'lower': '0', 'upper': '1'}
            else:
                pr['probability'] = p
        prop['cases'].append(pr)
        terminals = []
        if sampled:
            if k == 'end':
                terminals.append({'kind': 'end', 'end': 'done', 'count': 4})
            row = {'case_id': i, 'arrival': a, 'terminals': terminals,
                   'unfinished_count': 4 if k is None else 0, 'deadlocked_count': 0}
            if horizon is not None:
                row['cut_off_count'] = 4 if k == 'cut_off' else 0
        else:
            if k:
                t = {'kind': k, 'end': 'done' if k == 'end' else None,
                     'completed_at': rg(f) if k == 'end' else None,
                     'elapsed': rg(f-a) if k == 'end' else None,
                     'work': rg(w), 'result_at': None}
                if response is None and query != 'reachable' or k == 'cut_off':
                    t['probability'] = rg('1')
                terminals.append(t)
            row = {'case_id': i, 'arrival': a, 'terminals': terminals}
        outcomes.append(row)
    e = {'verdict': verdict, 'mode': 'simulation' if sampled else 'exhaustive',
         'bounds': {'cases': len(arrivals), 'live_tasks': 1, 'clock': clock, 'truncated': truncated},
         'properties': [prop], 'terminals': [], 'case_outcomes': outcomes,
         'absent_diagnostics': ['W-DEADLOCK-0', 'W-BUDGET-0']}
    # Boundary review (ADR 0042): an anonymous `response` of a multi-case model
    # has no obligation ledger under the old rule (Model::tracks_obligations);
    # a scenario shell does not add one. The per-case verdicts carry P04/P24.
    if selected is not None:
        end = until if form == 'fixed_every' else arrivals[-1]
        e['scenario'] = {'name': selected,
                         'arrivals': {'form': form, 'every': every if form == 'fixed_every' else None,
                                      'from': arrivals[0], 'until': end, 'count': len(arrivals)},
                         'horizon': horizon,
                         'drain': {'from': arrivals[-1], 'until': horizon if horizon is not None else 'all_terminal_or_clock'},
                         'aggregation': {'from': 0, 'until': 'horizon_or_all_terminal_or_clock' if horizon is not None else 'all_terminal_or_clock',
                                         'observed_until': scalar(None if truncated and not sampled else actual, sampled,
                                                                  'truncated' if truncated and not sampled else None)},
                         'warmup': 0}
    if sampled:
        e['simulation'] = {'status': 'completed', 'replications': 4, 'seed': 1, 'confidence': '1/2',
                           'attempted_replications': 4, 'completed_replications': 4,
                           'truncated_replications': 4 if truncated else 0,
                           'multiplicity': {'rows': 3, 'alpha_before': '1/4', 'alpha_after': '1/12'}}
        if horizon is not None:
            e['simulation']['cut_off_replications'] = 4 if cut else 0
    if metrics:
        bad = truncated and not sampled
        value = lambda x, missing=False: scalar(None if bad or missing else x, sampled,
                                                 'truncated' if bad else 'not_configured' if missing else None)
        count = lambda x: {'stats': value(x), 'observed_total': x*4 if sampled else None}
        completed, cutoff = kinds.count('end'), kinds.count('cut_off')
        counts = {k: count(n) for k, n in [('offered', len(arrivals)), ('received', len(arrivals)),
                  ('completed', completed), ('unfinished', kinds.count(None)), ('deadlocked', 0), ('rejected', 0), ('not_arrived', 0)]}
        if horizon is not None:
            counts['cut_off'] = count(cutoff)
        total = sum(work)
        # Boundary review M1: any computational cut (truncated_replications>0)
        # makes the sampled status truncated, with or without a scenario.
        e['metrics'] = {'status': 'truncated' if truncated else 'complete',
                        'observation_scope': 'until_horizon_or_execution_stop' if horizon is not None else 'until_execution_stop_no_extra_drain',
                        'overall': {'counts': counts, 'observed_until': value(actual), 'work': value(total),
                                    'occupancy_cost': value(total, not price), 'start_cost': value(0),
                                    'observed_cost_total': value(total, not price),
                                    'observed_cost_per_arrival': value(Fraction(total, len(arrivals)), not price)},
                        'actors': [{'actor': 'A', 'work': value(total),
                                    'utilization': value(Fraction(total, 30)), 'occupancy_cost': value(total, not price)}]}
        e['metrics']['capabilities']=[{'capability':'A.run','starts':value(sum(st<=limit for st in starts)),'start_cost':value(0)}]
        e['metrics']['terminals']=[{'end':'done','count':count(completed),
            'cost_per_case':value(service,not price) if completed or bad else scalar(None,sampled,'no_samples' if sampled else 'no_terminal')}]
        case_metrics=[]
        for i,(a,f,k,w) in enumerate(zip(arrivals,finishes,kinds,work),1):
            cc={'unfinished':count(int(k is None))}
            if horizon is not None: cc['cut_off']=count(int(k=='cut_off'))
            lead=value(f-a) if k=='end' or bad else scalar(None,sampled,'no_samples' if sampled else 'no_terminal')
            conditional_cost=value(service,not price) if k=='end' or bad else scalar(None,sampled,'no_samples' if sampled else 'no_terminal')
            ts=[{'kind':'end','end':'done','lead_time':lead,'cost_per_case':conditional_cost},{'kind':'rejected','end':None}]
            if horizon is not None:
                for t in ts: t['excluded']={'cut_off':count(int(k=='cut_off'))}
            case_metrics.append({'case_id':i,'arrival':a,'counts':cc,'terminals':ts,'work':value(w)})
        e['metrics']['cases']=case_metrics
    if legacy:
        e.pop('case_outcomes')
        e.pop('scenario', None)
        e['properties'] = [{'id': prop_id, 'verdict': verdict, **({'probability': probs[0]} if query == 'within' else {})}]
        e['terminals'] = [{'end': 'done', 'elapsed': rg(service), 'work': rg(service), 'result_at': None,
                           **({'probability': rg('1')} if query == 'within' else {})}]
    return e


def failure(code):
    return {'verdict': 'unsupported' if code.startswith('U-') else 'invalid',
            'bounds': {'truncated': None}, 'properties': [], 'terminals': [],
            'diagnostics': [{'code': code, 'severity': 'error'}]}


ROWS = []
FILES = {}


def add(name, source, expected, invocation=None, *, pair, calc, kind='runtime', extra=None):
    assert name.startswith('scn-') and name not in {r['name'] for r in ROWS}
    assert '//' not in source and '#' not in source
    invocation = {'command': 'check', 'scenario': 'Light'} if invocation is None else invocation
    record = dict(name=name, pair=pair, kind=kind, invocation=invocation)
    if extra:
        record.update(extra)
    ROWS.append(record)
    base = 'fixtures/' + name + '/'
    FILES[base+'model.relca'] = source
    FILES[base+'expected.json'] = json.dumps(expected, ensure_ascii=False, indent=2)+'\n'
    FILES[base+'invocation.json'] = json.dumps(invocation, ensure_ascii=False, indent=2)+'\n'
    selector_note = ('\nSelector-error span follows the archived PRE-B5 configuration-required check/simulate diagnostics for '
                     'crates/relca-cli/tests/support/scenario_legacy.relca '
                     '(whole model [0,425)); see tests/support/scenario-selector/calc.md. The original executable is recorded separately.\n'
                     if pair=='P13/U7' or name.startswith('scn-unknown') else '')
    ledger_note = ('\nNo obligation ledger: an anonymous response of a multi-case model has none under the old '
                   'rule, and the scenario shell does not add one (boundary review). By9 case3 has deadline13>D12 '
                   'and stays inconclusive; by7/by8 deadlines already due at D are expiry violations.\n'
                   if pair in ['P04','P24'] else '')
    FILES[base+'calc.md'] = (f'# {pair}: {name}\n\n'
        'Hand oracle, fixed before implementation (ADR0042; plan section 3). Time unit: min.\n\n'
        + calc + selector_note + ledger_note + '\n\nFIFO: start_i=max(arrival_i, finish_(i-1)), finish_i=start_i+service. '
        'At a business horizon D, work_i=max(0,min(D,finish_i)-start_i). '
        'A cutoff is neither End nor rejected nor unfinished nor a computational truncation.\n'
        'For four deterministic replications, three query rows and confidence 1/2, '
        'one-sided alpha=(1-1/2)/(2*3)=1/12. G(4;1/2)=F(0;1/2)=1/16<1/12; '
        'four hits satisfy >=1/2, four misses violate it, four unknowns give [0,1].\n'
        'No result was copied from a new Relca implementation. Stage C RED is not semantic verification.\n')


def pair(name, p, a, ea, b, eb, calc, ia=None, ib=None, kind='runtime', fault='change', fault_kind=None):
    add(name, a, ea, ia, pair=p, calc=calc, kind=kind)
    add(name+'.broken-'+fault, b, eb, ib if ib is not None else ia,
        pair=p, calc=calc, kind=fault_kind or ('static' if eb.get('diagnostics') else kind))


def build():
    pair('scn-fixed-every', 'P01', ONE, oracle(), edit(ONE, '12min)', '13min)'), failure('E-ARR-0'),
         'n=1+(12-0-1)//4=3: [0,4,8], finishes [3,7,11]. Changing b to13 yields [0,4,8,12], four arrivals for cases3.', fault='end')
    pair('scn-select', 'P02', S, oracle(), S, oracle(arrivals=(0,1,2), every=1, until=3, selected='Busy'),
         'Only the selector changes. Light elapsed [3,3,3]; Busy starts [0,3,6], finishes [3,6,9], elapsed [3,5,7], within4 probabilities [1,0,0].',
         ib={'command':'check','scenario':'Busy'}, fault='busy')
    hi = {'command':'check','scenario':'Day'}
    ho: dict[str, Any] = dict(arrivals=(0,2,4), service=5, horizon=12, every=2, until=6, query='reachable', metrics=True, selected='Day')
    eq = edit(H, 'clock 30min', 'clock 12min')
    pair('scn-horizon', 'P03', H, oracle(**ho), edit(H, 'horizon 12min', 'horizon 11min'),
         oracle(**dict(ho,horizon=11)), 'H: [0,5], [5,10], [10,15]. D12 gives End5/10 and cutoff work2. Total12, price12, utilization12/30=2/5. D11 gives cutoff work1.', hi, fault='earlier')
    pair('scn-horizon-equal-clock', 'P03/P14', eq, oracle(**dict(ho,clock=12)),
         edit(eq, '  horizon 12min\n', ''), oracle(**dict(ho,horizon=None,clock=12)),
         'Only the horizon line is removed. D=clock=12 is business cutoff, not computational cut. Without D, case3 has no terminal and metrics are truncated/null.', hi, fault='no-horizon')
    contract = 'contract K { response case.received -> one_of(end.done) by 9min after case.received otherwise unresolved }'
    obligation = edit(H, REACH, contract)
    pair('scn-obligation', 'P04', obligation, oracle(**dict(ho,response=9)), edit(obligation,'by 9min','by 7min'),
         oracle(**dict(ho,response=7)), 'by9 deadlines [9,11,13]: cases1/2 resolved, case3 unknown. by7 deadlines [7,9,11]: cases2/3 violated, never excused by cutoff.', hi, fault='short')
    pair('scn-drain', 'P05', edit(H,'horizon 12min','horizon 20min'), oracle(**dict(ho,horizon=20)),
         edit(H,'horizon 12min','horizon 14min'), oracle(**dict(ho,horizon=14)),
         'Last actual arrival4 (not exclusive right edge6). Declared drain [4,20], observed end15, totalwork15, perarrival5, utilization1/2. D14 observes14, third work4, utilization7/15.',hi,fault='short-horizon')
    sim = edit(edit(edit(H,'fixed 5min','fixed 3min'),'horizon 12min','horizon 8min'),REACH,
               'verify probability reached hit == true >= 0.5')
    sim = edit(edit(sim,'clock 30min','clock 8min'),'scenario Day {','scenario Day {\n  '+SIM)
    so: dict[str, Any] = dict(ho,service=3,horizon=8,clock=8,sampled=True,query='probability')
    pair('scn-sim', 'P06',sim,oracle(**so),edit(sim,'  horizon 8min\n',''),oracle(**dict(so,horizon=None)),
         'Four identical paths: finish3,6,9; D8 cuts third at work2. hits [4,4,0], unknown [0,0,4]. cutoff reps4 versus clock truncated reps4. Totalwork8 each, observed total32. '
         'Without D the clock8 cut makes truncated_replications4, so metrics.status is truncated as for a model without the scenario shell; '
         'a sampled metric with no sample has reason no_samples (ADR 0019), with or without a scenario.',
         {'command':'simulate','scenario':'Day'},fault='no-horizon')
    original_sim=edit(sim,'clock 8min','clock 30min')
    pair('scn-sim-clock30','P06',original_sim,oracle(**dict(so,clock=30)),
         edit(original_sim,'horizon 8min','horizon 7min'),oracle(**dict(so,clock=30,horizon=7)),
         'Retain the original ADR positive: D8/clock30, finishes3/6/9, third cutoff work2, totalwork8 and unknown4. The equal-clock pair is separate. Change only D8 to7: third work1, totalwork7, still unknown4. Old sampled clock cuts also observe partial occupancy through clock, so the no-horizon clock8 control has metric work8, not6.',
         {'command':'simulate','scenario':'Day'},fault='earlier')
    fast = '''  capability fast() -> T { duration fixed 1min; result nondet { ok }; writes hit; finish { hit = true } }
'''
    comp = edit(S,'process Flow', 'process Q { q = A.fast(); end done }\nprocess Flow')
    comp = edit(comp,'}\nprocess Q', fast+'}\nprocess Q')
    comp = edit(comp,QUERY,'configuration Slow { cut C; process Flow }\nconfiguration Fast { cut C; process Q }\n'+QUERY)
    def comparison(selected, diff=False):
        a = (0,4,8) if selected=='Light' else (0,1,2)
        kw: dict[str, Any] = dict(arrivals=a, every=4 if selected=='Light' else 1, until=12 if selected=='Light' else 3, selected=selected)
        slow, quick = oracle(**kw),oracle(**dict(kw,service=1))
        v=fold([slow['verdict'],quick['verdict']])
        if diff:
            return {'verdict':v,'from':{'report':slow},'to':{'report':quick}}
        return {'verdict':v,'scenario':selected,'analysis':'exhaustive','mismatch':None,
                'plans':[{'configuration':'Slow','report':slow},{'configuration':'Fast','report':quick}]}
    ci = {'command':'compare','plans':['Slow','Fast'],'mode':'exhaustive','scenario':'Light'}
    pair('scn-compare','P07',comp,comparison('Light'),comp,comparison('Busy'),
         'Same scenario for both plans. Light Slow[3,7,11], Fast[1,5,9]. Busy Slow[3,6,9] violates within4 for2/3; Fast[1,2,3] satisfies all.',ci,dict(ci,scenario='Busy'),fault='busy')
    hc=edit(H,'}\nprocess Flow',fast+'}\nprocess Q { q = A.fast(); end done }\nprocess Flow')
    hc=edit(hc,REACH,'configuration Slow { cut C; process Flow }\nconfiguration Fast { cut C; process Q }\n'+REACH)
    hc=edit(hc,'  rate A.run per_start 0','  rate A.run per_start 0\n  rate A.fast per_start 0')
    def cut_comparison(d, diff=False):
        slow=oracle(**dict(ho,horizon=d))
        quick=oracle(**dict(ho,horizon=d,service=1))
        quick['metrics']['capabilities'][0]['capability']='A.fast'
        verdict=slow['verdict']
        if diff: return {'verdict':verdict,'from':{'report':slow},'to':{'report':quick}}
        return {'verdict':verdict,'scenario':'Day','analysis':'exhaustive','mismatch':None,
                'plans':[{'configuration':'Slow','report':slow},{'configuration':'Fast','report':quick}]}
    for cmd in ['compare','diff']:
        inv: dict[str, Any]=dict(command=cmd,scenario='Day')
        if cmd=='compare': inv.update(plans=['Slow','Fast'],mode='exhaustive')
        else: inv.update(**{'from':'Slow','to':'Fast'})
        pair('scn-'+cmd+'-cutoff','H-PROJECTION',hc,cut_comparison(12,cmd=='diff'),edit(hc,'horizon 12min','horizon 20min'),cut_comparison(20,cmd=='diff'),
             'Slow service5: End5,10 then cutoff work2 at12; Fast service1: End1,3,5. Both sides use Day. At D20 Slow ends15 with no cutoff. Raw cutoff work2/probability1 must survive terminal projection, not become zero/not_reported.',inv,fault='full-drain')
    pair('scn-shared','P08',S,oracle(arrivals=(0,1,2),every=1,until=3,selected='Busy'),
         edit(S,'scenario Busy {','scenario Busy {\n'+POLICY),failure('U-SEM-0'),
         'Inherited FIFO fixes finishes [3,6,9]. Duplicate model/scenario policy is unsupported, not an override.',
         {'command':'check','scenario':'Busy'},fault='both')
    spare = scenario('Spare')
    unselected = edit(S,QUERY,spare+QUERY)
    pair('scn-unselected','P09',unselected,oracle(),edit(unselected,spare,edit(spare,'12min)','13min)')),failure('E-ARR-0'),
         'Spare has four arrivals but cases3 after a single right-edge edit. Light must not execute. Every scenario receives full static validation.',fault='spare')
    label = edit(edit(edit(S,'Light','BusyDay'), 'Busy {','SmallExact {'),'actor A {','actor A {\n  grain team')
    label = edit(edit(edit(label,'actor A {','actor Desk {'),'A.run','Desk.run'),'include [A]','include [Desk]')
    assert label != S and 'actor Desk' in label
    pair('scn-label','P10',S,oracle(),label,oracle(selected='BusyDay'),
         'Only scenario/Actor names and grain metadata change. Same arrivals, times, probabilities, verdict and exit. A closed source-derived metadata projection is tested separately.',
         ib={'command':'check','scenario':'BusyDay'},fault='renamed')
    fixed = edit(ONE,'arrivals fixed_every 4min during [0min, 12min)','arrivals fixed [0min, 4min, 8min]')
    pair('scn-equivalent','P11',fixed,oracle(form='fixed'),edit(fixed,'[0min, 4min, 8min]','[1min, 4min, 8min]'),
         oracle(arrivals=(1,4,8),form='fixed'),'Explicit fixed[0,4,8] equals P01. Change only first arrival to1: finishes [4,7,11], elapsed still3.',fault='first')
    one = edit(edit(ONE,LIGHT,scenario('One','',cases=1,clock=10)),POLICY+'\n','')
    route = {'command':'check','scenario':'One'}
    old = edit(one,scenario('One','',cases=1,clock=10),'bounds { cases 1; live_tasks 1; clock 10min }\n')
    pair('scn-route','P12',one,oracle(arrivals=(0,),clock=10,selected='One',form='fixed',legacy=False),
         old,oracle(arrivals=(0,),clock=10,selected=None,legacy=True),
         'No policy, queue, concurrent or arrivals in either input. Scenario One forces new route with case_outcomes; moving bounds out restores genuine legacy route.',route,{'command':'check'},fault='no-scenario')
    pair('scn-clock-priority','P14',eq,oracle(**dict(ho,clock=12)),edit(eq,'clock 12min','clock 11min'),oracle(**dict(ho,clock=11)),
         'Equal D/clock12 prioritizes horizon. Change clock alone to11: truncated true, case3 no cutoff row, metrics null despite declared horizon12.',hi,fault='short-clock')
    pair('scn-inclusive','P15',edit(H,'horizon 12min','horizon 10min'),oracle(**dict(ho,horizon=10)),
         edit(H,'horizon 12min','horizon 9min'),oracle(**dict(ho,horizon=9)),
         'At D10, finish2 and start3 happen before cutoff: completed2/cutoff1, starts3, work10. D9: completed1/cutoff2, starts2, work9.',hi,fault='earlier')
    future = edit(edit(H,'fixed 5min','fixed 1min'),'arrivals fixed_every 2min during [0min, 6min)','arrivals fixed [0min, 4min, 12min]')
    pair('scn-future-arrival','P16',future,oracle(**dict(ho,arrivals=(0,4,12),service=1,form='fixed')),
         edit(future,'[0min, 4min, 12min]','[0min, 4min, 13min]'),failure('E-ARR-0'),
         'D12 includes arrival/start3 at12, received3 and cutoff work0. Arrival13>D is invalid, not Future/not_arrived.',hi,fault='after')
    w6 = edit(H,REACH,'verify probability reached hit == true within 6min >= 1')
    pair('scn-within','P17',w6,oracle(**dict(ho,query='within',within=6)),edit(w6,'within 6min','within 9min'),oracle(**dict(ho,query='within',within=9)),
         'W6 deadlines[6,8,10]<=D12: [P1,P0,P0], per-case [S,V,V]. W9 deadlines[9,11,13]: [P1,P1,P[0,1]], [S,S,I].',hi,fault='long-window')
    di={'command':'diff','from':'Slow','to':'Fast','scenario':'Light'}
    pair('scn-diff','P18',comp,comparison('Light',True),comp,comparison('Busy',True),
         'Same arithmetic as P07, but both diff sides and comparison table must retain the selector.',di,dict(di,scenario='Busy'),fault='busy')
    cross = edit(unselected,QUERY,METRICS+'\n'+QUERY)
    pair('scn-unselected-window','P19',cross,oracle(metrics=True),edit(cross,spare,edit(spare,'clock 30min','clock 31min')),failure('E-CON-0'),
         'Window30 covers every clock30. Only Spare clock31 makes whole model invalid even when Light is selected.',fault='spare-clock')
    direct = edit(ONE,LIGHT,'arrivals fixed_every 4min during [0min, 12min)\nbounds { cases 3; live_tasks 1; clock 30min }\n')
    pair('scn-direct-every','P21',direct,oracle(selected=None),edit(direct,'12min)','13min)'),failure('E-ARR-0'),
         'Direct model fixed_every expands identically [0,4,8]. No scenario key. b13 generates four arrivals for cases3.',{'command':'check'},fault='end')
    cut = edit(H,'cut C { include [A] }','cut C { include [A]; horizon received..done within 1min }')
    pair('scn-cut-record','P22',cut,oracle(**ho),edit(cut,'within 1min','within 2min'),oracle(**ho),
         'Cut.horizon is documentation only. Its 1->2 edit does not change D12, totalwork12, cutoff1, observed_until12.',hi,fault='record')
    pair('scn-missing-price','P23',H,oracle(**ho),edit(H,'  rate A per_minute 1\n',''),oracle(**dict(ho,price=False)),
         'Removing actor rate leaves work12/cutoff1. Occupancy and total/perarrival costs become null/not_configured, not zero.',hi,fault='unset')
    by8=edit(obligation,'by 9min','by 8min')
    explain_bad=oracle(**dict(ho,response=8))
    explain_bad['explanation']={'status':'complete','shortest_scope':'explored_graph','counterexamples':[
        {'id':'contract K #1 response','case_id':3,'status':'verified','reason':None,
         'replay':{'status':'verified','reason':None}}]}
    explain_pending=oracle(**dict(ho,response=9))
    explain_pending['explanation']={'status':'complete','shortest_scope':'explored_graph','counterexamples':[]}
    pair('scn-explain','P24',by8,explain_bad,
         obligation,explain_pending,
         'by8 deadlines[8,10,12], finish2=deadline10; unresolved3 violates exactly12 after advance. by9 leaves3 pending at13, no violation counterexample.',
         {'command':'explain','scenario':'Day'},fault='later-deadline')
    pair('scn-mutate','P25',ONE,{'verdict':'unsupported','original':oracle()},ONE,
         {'verdict':'invalid','original':failure('E-CONF-0')},
         'No reads/guards to mutate; unsupported default mutation kinds keep root exit4, original S. Removing selector yields original E-CONF-0/root invalid, zero executed mutants.',
         {'command':'mutate','scenario':'Light'},{'command':'mutate'},kind='mutation',fault='no-selector')
    nd = edit(S,QUERY,edit(QUERY,'>= 1','>= 0.5')+'\n'+SIM)
    pair('scn-simulation-fifo','P26',nd,oracle(arrivals=(0,1,2),every=1,until=3,selected='Busy',sampled=True),
         edit(nd,POLICY+'\n',''),{'verdict':'unsupported','diagnostics':[{'code':'U-SEM-0'}],
         'simulation':{'status':'unsupported','attempted_replications':1,'completed_replications':0}},
         'FIFO: hits[4,0,0], unknown0. Removing only policy exposes two eligible waiting cases at3: reject nondeterminism on replication1 and withdraw all partial statistics.',
         {'command':'simulate','scenario':'Busy'},fault='no-policy',fault_kind='simulation_abort')
    keys = (ROOT/'fixtures/sim-keys/model.relca').read_text()
    keys=edit(keys,'  bounds { cases 1; live_tasks 1; clock 2min }',
              '  scenario X { bounds { cases 1; live_tasks 1; clock 2min } }\n  scenario Y { bounds { cases 1; live_tasks 1; clock 2min } }')
    # This reuses the existing checked seed=0 oracle; selection forces arrivals route.
    ke=json.loads((ROOT/'fixtures/sim-keys/expected.json').read_text())
    ke['scenario']={'name':'X'}
    ky=dict(ke,scenario={'name':'Y'})
    pair('scn-random-name','P27',keys,ke,keys,ky,
         'Reuse sim-keys seed0 SHA256 counter oracle. X and Y have identical conditions: all draws, keys, query truths and terminals must coincide in both interpreters.',
         {'command':'simulate','scenario':'X'},{'command':'simulate','scenario':'Y'},kind='simulation',fault='renamed')
    aset='''model M version "0.2" {
type T = enum { ok }
actor L { state keys: set<CaseId> }
actor A { capacity 1; capability c() -> T { duration fixed 1min; result nondet { ok } } }
process P { r = A.c(); end done }
cut C { include [L,A] }
'''
    aset+=scenario('Small','arrivals fixed [0min, 0min]',cases=2)+scenario('Large','arrivals fixed ['+', '.join(['0min']*64)+']',cases=64)+REACH+'\n}\n'
    large=scenario('Large','arrivals fixed ['+', '.join(['0min']*64)+']',cases=64)
    too=scenario('Large','arrivals fixed ['+', '.join(['0min']*65)+']',cases=65)
    pair('scn-set-limit','P28',aset,{'verdict':'satisfied'},edit(aset,large,too),failure('U-SEM-0'),
         'Small2 is selected; unused Large64 is legal for set<CaseId>. Change the single Large workload to a matching65 arrivals/cases: reject set capacity, not arrival mismatch.',
         {'command':'check','scenario':'Small'},fault='large65')
    static_pairs(comp)
    branches = edit(H,'duration fixed 5min','duration {1min:0.5,3min:0.5}')
    branches = edit(branches,scenario('Day','arrivals fixed_every 2min during [0min, 6min)',horizon=12),
                    scenario('Day','',cases=1,clock=5,horizon=2))
    branches = edit(branches,REACH,'verify probability reached hit == true >= 1')
    bh = {'verdict':'inconclusive','bounds':{'truncated':False},
          'scenario':{'horizon':2,'aggregation':{'observed_until':{'min':'1','max':'2','reason':None}}},
          'properties':[{'id':'probability reached #1','verdict':'inconclusive','cases':[
              {'case_id':1,'arrival':0,'verdict':'inconclusive','probability':{'min':'1/2','max':'1'}}]}],
          'case_outcomes':[{'case_id':1,'arrival':0,'terminals':[
              {'kind':'end','end':'done','completed_at':rg(1),'elapsed':rg(1),'work':rg(1),'result_at':None,'probability':rg('1/2')},
              {'kind':'cut_off','end':None,'completed_at':None,'elapsed':None,'work':rg(2),'result_at':None,'probability':rg('1/2')}]}]}
    full={'verdict':'satisfied','bounds':{'truncated':False},'scenario':{'horizon':3},
          'properties':[{'id':'probability reached #1','verdict':'satisfied','cases':[
              {'case_id':1,'arrival':0,'verdict':'satisfied','probability':rg('1')}]}],
          'case_outcomes':[{'case_id':1,'arrival':0,'terminals':[
              {'kind':'end','end':'done','completed_at':{'min':1,'max':3},'elapsed':{'min':1,'max':3},'work':{'min':1,'max':3},'result_at':None,'probability':rg('1')}]}]}
    pair('scn-probability-mass','H34',branches,bh,edit(branches,'horizon 2min','horizon 3min'),full,
         'Two positive mass branches: finish1 has End mass1/2; duration3 survives toD2 with cutoff mass1/2 and work2. Query interval[1/2,1]. D3 processes finish at boundary and End mass is1. Never renormalize remaining End mass.',hi,fault='full-window')
    rejected=edit(edit(H,METRICS+'\n',''),REACH,'queue capacity 0 on_overflow reject\nverify probability reached hit == true >= 1')
    re=oracle(**dict(ho,metrics=False,query='probability'))
    re['verdict']='violated'
    re['scenario']['aggregation']['observed_until']=scalar(4)
    re['admission']={'rejected_count':rg(3)}
    re['case_outcomes']=[{'case_id':i,'arrival':a,'terminals':[{'kind':'rejected','end':None,
        'completed_at':None,'elapsed':None,'work':None,'result_at':None,'probability':rg('1')}]} for i,a in enumerate([0,2,4],1)]
    re['properties'][0].update(verdict='violated',cases=[{'case_id':i,'arrival':a,'verdict':'violated','probability':rg('0')} for i,a in enumerate([0,2,4],1)])
    pair('scn-rejected-not-cutoff','H-REJECT',rejected,re,edit(rejected,'capacity 0 on_overflow','capacity 3 on_overflow'),oracle(**dict(ho,metrics=False,query='probability')),
         'Queue0 rejects every arrival[0,2,4]. All three rejected terminal masses remain1 and query probabilities0 at D12; actual all-terminal stop4, cutoff0. Queue3 admits all: End5,10 and cutoff work2, third query unknown.',hi,fault='admit')
    # Lower-only boundary: do not accidentally require exhaustive exploration of10000 cases.
    FILES['crates/relca-core/tests/support/scenario_limit_10000.relca']=edit(edit(ONE,'12min)','10000min)'),'fixed_every 4min','fixed_every 1min')
    FILES['crates/relca-core/tests/support/scenario_limit_10000.relca']=edit(FILES['crates/relca-core/tests/support/scenario_limit_10000.relca'],'cases 3','cases 10000')
    FILES['crates/relca-core/tests/support/scenario_busyday.relca']='''model BusyLoad version "0.2" {
type T = enum { ok }
observed case { hit: bool = false }
actor A { capacity 2; capability run() -> T {
  duration { 6min: 0.5, 8min: 0.5 }
  result nondet { ok }
  writes hit
  finish { hit = true }
} }
process P { r = A.run(); end done }
cut C { include [A] }
policy fifo tie_by case_id then call_order
scenario SmallExact {
  arrivals fixed [0min, 0min]
  bounds { cases 2; live_tasks 1; clock 30min }
}
scenario BusyDay {
  arrivals fixed_every 4min during [0min, 480min)
  horizon 960min
  bounds { cases 120; live_tasks 1; clock 960min }
  simulation replications 1000 seed 41020 confidence 0.95
}
verify probability reached hit == true >= 0.9
}
'''


def static_pairs(comp):
    extras = {
        'case-mix':'case_mix { routine:0.8; difficult:0.2 }',
        'resources':'resources { A:2 }', 'analysis':'analysis finite_model_check',
        'input':'input finite_cases 2', 'stop':'stop_arrivals_at 12min',
        'warmup':'warmup 4min', 'calendar':'calendar weekdays',
        'queue-old':'queue capacity 2 on_overflow reject_and_count',
        'metrics':'metrics { window 30min; currency "JPY" }',
        'process':'process Other { end done }', 'actor':'actor Other {}',
        'configuration':'configuration Other { cut C; process Flow }',
        'cut':'cut Other { include [A] }', 'verify':'verify reachable end.done',
        'contract':'contract K { invariant true }',
    }
    edits = [(k, ONE,edit(ONE,'scenario Light {','scenario Light {\n  '+v),'U-SYN-0') for k,v in extras.items()]
    edits += [
        ('right-closed',ONE,edit(ONE,'12min)','12min]'),'U-SYN-0'),
        ('left-open',ONE,edit(ONE,'[0min, 12min)','(0min, 12min)'),'U-SYN-0'),
        ('direct-horizon',ONE,edit(ONE,QUERY,'horizon 12min\n'+QUERY),'U-SYN-0'),
        ('poisson',ONE,edit(ONE,'fixed_every 4min during [0min, 12min)','poisson 1'),'U-SYN-0'),
        ('bounds-queue',ONE,edit(ONE,'cases 3;','cases 3; queue 2;'),'U-SYN-0'),
        ('bounds-live-old',ONE,edit(ONE,'live_tasks 1','live_tasks_per_case 1'),'U-SYN-0'),
        ('config-scenario',comp,edit(comp,'configuration Slow {','configuration Slow { scenario Inner {}'),'U-SYN-0'),
        ('duplicate-name',ONE,edit(ONE,QUERY,LIGHT+QUERY),'U-SEM-0'),
        ('duplicate-bounds',ONE,edit(ONE,'scenario Light {','scenario Light { bounds { cases 3; live_tasks 1; clock 30min }'),'U-SEM-0'),
        ('duplicate-simulation',ONE,edit(ONE,'scenario Light {','scenario Light { '+SIM+'; '+SIM),'U-SEM-0'),
        ('duplicate-arrivals',ONE,edit(ONE,'scenario Light {','scenario Light { arrivals fixed [0min, 4min, 8min]'),'U-SEM-0'),
        ('step-zero',ONE,edit(ONE,'fixed_every 4min','fixed_every 0min'),'E-ARR-0'),
        ('equal-interval',ONE,edit(ONE,'[0min, 12min)','[4min, 4min)'),'E-ARR-0'),
        ('reverse-interval',ONE,edit(ONE,'[0min, 12min)','[5min, 4min)'),'E-ARR-0'),
        ('horizon-zero',ONE,edit(ONE,'scenario Light {','scenario Light { horizon 0min'),'E-ARR-0'),
        ('missing-name',ONE,edit(ONE,'scenario Light {','scenario {'),'E-SYN-0'),
        ('missing-duration',ONE,edit(ONE,'fixed_every 4min','fixed_every'),'E-SYN-0'),
        ('missing-during',ONE,edit(ONE,'4min during','4min'),'E-SYN-0'),
        ('missing-brace',ONE,edit(ONE,'scenario Light {','scenario Light'),'E-SYN-0'),
    ]
    for name, good, bad, code in edits:
        conf=name=='config-scenario'
        e={'verdict':'satisfied'} if conf else oracle()
        inv={'command':'check','configuration':'Slow','scenario':'Light'} if conf else None
        pair('scn-static-'+name,'P13',good,e,bad,failure(code),
             f'Single edit {name}: closed syntax/semantic contract requires {code}; static failure has no runtime bounds or properties.',inv,fault=name)
    cap=edit(edit(ONE,'4min during [0min, 12min)','1min during [0min, 10001min)'),'cases 3','cases 10001')
    pair('scn-static-expansion','P13',ONE,oracle(),cap,failure('U-SEM-0'),
         '10001 arrivals match cases10001, so only expansion cap is violated. Exact10000 successful lower is a separate lower-only input.',fault='10001')
    for field in ['arrivals','bounds','simulation','queue']:
        item={'arrivals':'arrivals fixed [0min, 4min, 8min]','bounds':'bounds { cases 3; live_tasks 1; clock 30min }',
              'simulation':SIM,'queue':'queue capacity 3 on_overflow reject'}[field]
        good=ONE
        inside=item
        expectation=oracle()
        if field=='arrivals':
            inside='arrivals fixed_every 4min during [0min, 12min)'
            good=edit(good,'  '+inside+'\n','')
            good=edit(good,QUERY,item+'\n'+QUERY)
            expectation=oracle(form='fixed')
        elif field=='bounds':
            good=edit(good,'  '+item+'\n','')
            good=edit(good,QUERY,item+'\n'+QUERY)
        else:
            good=edit(ONE,QUERY,item+'\n'+QUERY)
        bad=edit(good,'scenario Light {','scenario Light {\n'+inside)
        pair('scn-duplicate-common-'+field,'P13/U6',good,expectation,bad,failure('U-SEM-0'),
             f'Common {field} is inherited. Same whole item inside Light is unsupported; no field merge or silent override.',fault='both')
    for command in ['check','simulate','compare','diff','explain','mutate']:
        entry_args: dict[str, Any]={'command':command,'scenario':'Light'}
        good=comp if command in ['compare','diff'] else ONE
        if command=='compare': entry_args.update(plans=['Slow','Fast'],mode='exhaustive')
        if command=='diff': entry_args.update(**{'from':'Slow','to':'Fast'})
        if command=='simulate': good=edit(good,QUERY,edit(QUERY,'>= 1','>= 0.5')+'\n'+SIM)
        base={'verdict':'unsupported','original':oracle()} if command=='mutate' else {'verdict':'satisfied'}
        if command=='explain': base=oracle()
        refusal: dict[str, Any]={'verdict':'invalid'}
        if command in ['check','simulate']: refusal=failure('E-CONF-0')
        if command=='explain': refusal=failure('E-CONF-0')
        if command=='mutate': refusal['original']=failure('E-CONF-0')
        if command=='compare': refusal['plans']=[{'configuration':n,'report':failure('E-CONF-0')} for n in ['Slow','Fast']]
        if command=='diff': refusal.update(**{'from':{'report':failure('E-CONF-0')},'to':{'report':failure('E-CONF-0')}})
        pair('scn-select-required-'+command,'P13/U7',good,base,good,refusal,
             f'Only invocation selector is removed. {command} must require a selector even with just one scenario; simulate may not return E-SIM-0 first.',
             entry_args,{k:v for k,v in entry_args.items() if k!='scenario'},kind='invocation',fault='missing')
    pair('scn-unknown','P13',ONE,oracle(),ONE,failure('E-NAME-0'),
         'Declared Light exists; selecting Nope is E-NAME-0, not declaration absence.',ib={'command':'check','scenario':'Nope'},fault='name')
    # No simulation in selected Light, although an unselected scenario supplies it.
    n=edit(edit(S,'scenario Busy {','scenario Busy { '+SIM),'>= 1','>= 0.5')
    pair('scn-no-simulation','P13',edit(n,'scenario Light {','scenario Light { '+SIM),{'verdict':'satisfied'},n,failure('E-SIM-0'),
         'Only selected Light simulation is removed. Busy simulation cannot be borrowed.',{'command':'simulate','scenario':'Light'},fault='missing')
    c=dict(command='compare',plans=['Slow','Fast'],mode='exhaustive',scenario='Light')
    pair('scn-compare-many','P13/U14',comp,{'verdict':'satisfied'},comp,
         {'verdict':'unsupported','diagnostics':[{'code':'U-SEM-0'}]},
         'Single --scenario Light,Busy is a semantic multiple-scenario refusal; duplicate flags are syntax errors tested separately.',c,dict(c,scenario='Light,Busy'),fault='many')


def substrate(source, selected):
    """Independent lexical erasure for old-grammar validation, not selection API.

    Runs only on intended-valid fixture models. It deliberately cannot check
    all-scenario validation and is never used by the new semantic acceptance.
    """
    import re
    ts=list(re.finditer(r'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[^\s]',source))
    spans=[]
    chosen=''
    i=0
    while i<len(ts):
        if ts[i].group()=='scenario':
            name=ts[i+1].group()
            assert ts[i+2].group()=='{'
            depth=1
            j=i+3
            while depth:
                if ts[j].group()=='{': depth+=1
                if ts[j].group()=='}': depth-=1
                j+=1
            if name==selected:
                chosen=source[ts[i+2].end():ts[j-1].start()]
            spans.append((ts[i].start(),ts[j-1].end()))
            i=j
        else: i+=1
    if spans:
        assert chosen, selected
    for a,b in reversed(spans): source=source[:a]+source[b:]
    if spans:
        end=source.rfind('}')
        source=source[:end]+chosen+'\n'+source[end:]
    source=re.sub(r'(?m)^\s*horizon\s+[0-9]+min\s*\n','\n',source)
    def expand(m):
        step,a,b=map(int,m.groups())
        assert step>0 and a<b
        return 'arrivals fixed ['+', '.join(f'{x}min' for x in range(a,b,step))+']'
    return re.sub(r'arrivals fixed_every ([0-9]+)min during \[([0-9]+)min, ([0-9]+)min\)',expand,source)


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--write',action='store_true')
    p.add_argument('--write-support',action='store_true')
    p.add_argument('--check',action='store_true')
    o=p.parse_args()
    build()
    names=sorted(r['name'] for r in ROWS)
    assert all('.broken-' in n or any(x.startswith(n+'.broken-') for x in names) for n in names)
    FILES['crates/relca-core/tests/support/scenario_names.json']=json.dumps(names,indent=2)+'\n'
    FILES['crates/relca-core/tests/support/scenario_names.rs']='&'+json.dumps(names,indent=2)+'\n'
    FILES['crates/relca-core/tests/support/scenario_cases.json']=json.dumps(sorted(ROWS,key=lambda r:r['name']),indent=2)+'\n'
    substrates=[]
    for row in ROWS:
        expected=json.loads(FILES['fixtures/'+row['name']+'/expected.json'])
        if row['kind']=='static' or expected['verdict'] in ['invalid','unsupported']:
            continue
        inv=row['invocation']
        if not inv.get('scenario') and 'scenario ' in FILES['fixtures/'+row['name']+'/model.relca']:
            continue
        src=substrate(FILES['fixtures/'+row['name']+'/model.relca'],inv.get('scenario'))
        substrates.append(dict(name=row['name'],source=src,configuration=inv.get('configuration',inv.get('plans',[inv.get('from')])[0])))
    FILES['crates/relca-core/tests/support/scenario_substrates.json']=json.dumps(substrates,indent=2)+'\n'
    for name,text in FILES.items():
        path=ROOT/name
        if o.write or o.write_support and not name.startswith('fixtures/'):
            assert path.is_relative_to(ROOT)
            path.parent.mkdir(parents=True,exist_ok=True)
            if not path.exists() or path.read_text()!=text:
                path.write_text(text)
        elif o.check:
            assert path.read_text()==text, name
    print(f'{len(names)} closed fixtures; {len(FILES)} materialized files; hand arithmetic only')


if __name__=='__main__':
    main()
