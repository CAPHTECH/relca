"""ADR0043 independent hand tables. No production executable is consulted.

Materialize only the closed agy2 population. A replacement is witnessed before
writing either member. Durations are minutes; counts are per-case static sinks.
"""
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
OUT = ROOT / 'fixtures'
ROWS = []
SIGNAL = '判定・拒否・実行の担い手と生成根の集中の兆候'
MECHANISM = '同じ担い手又は生成根への依存が構成上重なる'
CHECK = 'モデル外の統制・原本参照・ログで確認する'


def replace(s, a, b):
    t = s.replace(a, b)
    assert t != s, a
    return t


W = '''model AgencySecondHalf version "0.2" {
  type Entry = enum { entry }
  type V = enum { pass, fail }
  type Ack = enum { committed }
  people [c1, a1]
  actor Doc { source document }
  actor Clerk { executors [c1]; capability enter() -> Entry { reads [Doc]; duration fixed 1min; result nondet { entry } } }
  actor Approver { executors [a1]; capability approve(e: Entry) -> V { duration fixed 1min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post(v: V) -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  process Flow { e = Clerk.enter(); v = Approver.approve(e); if v == pass {} else { end returned }; k = Ledger.post(v); end posted }
  cut Scope { include [Doc, Clerk, Approver, Ledger] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
GUARD = 'if v == pass {} else { end returned }; '


def carrier(actor, people=(), permitted=None):
    return dict(actors=[actor], people=list(people),
                unattributed_actors=[] if people else [actor],
                permitted=list(people if permitted is None else permitted))


def judgement(actor='Approver', people=('a1',), stops='all_paths', bypasses=0,
              call='v', capability='approve', permitted=None):
    return dict(call=call, capability=f'{actor}.{capability}',
                **carrier(actor, people, permitted), stops=stops, bypasses=bypasses)


def effect(actor='Ledger', people=(), js=None, single_actor=False,
           single_person='unknown', roots=None, state=None, call='k', unknown=False):
    js = [judgement()] if js is None else js
    roots = [dict(call='e', capability='Clerk.enter')] if roots is None else roots
    concentration = dict(single_actor=single_actor, single_person=single_person,
                         single_generation_root=len(roots) == 1,
                         generation_roots=roots, signal=SIGNAL, mechanism=MECHANISM, check=CHECK)
    r = dict(effect=f'{actor}.post', call=call, status='derived', e=carrier(actor, people),
             j=js, u=[j for j in js if j['stops'] != 'none'],
             state_refusers=state or [], process_judgements='derived', concentration=concentration)
    if unknown:
        del r['j'], r['u']
        r['process_judgements'] = dict(status='not_derived', reason='state_guards')
        for key in ('single_actor', 'single_person', 'single_generation_root'):
            concentration[key] = 'unknown'
        concentration['generation_roots'] = dict(status='not_derived', reason='state_guards')
    return r


def effects(rows, state_write=False):
    return dict(status='derived', rows=rows, unchecked=['authority', 'fallback_path'] +
                (['state_write_effects'] if state_write else []))


def lrow(cap, lo, hi, cancel=False, fallback='unchecked', reason=None):
    r = dict(capability=cap, status='derived', normal=dict(min=lo, max=hi),
             contest_route=False, basis='unknown', fallback=fallback,
             join_any_cancellation=cancel)
    if reason:
        del r['normal']
        r.update(status='not_derived', reason=reason)
    return r


def frow(reachable, readers=(), selectors=(), reason=None):
    if reason:
        return dict(end='done', status='not_derived', reason=reason)
    return dict(end='done', status='derived', reachable=reachable,
                selectors=list(selectors), read_only=list(readers))


def expected(es=None, verdict='satisfied', counts=None, framing=None):
    vp = dict(rules='agency-viewpoints-p1-2')
    if es is not None:
        vp['effects'] = effects(es)
    if counts is not None:
        vp['retention'] = dict(status='derived', capabilities=counts)
    if framing is not None:
        vp['framing'] = dict(status='derived', ends=[framing])
    return dict(verdict=verdict, agency=dict(viewpoints=vp))


def refused(verdict, code):
    return dict(verdict=verdict, agency=None, diagnostics=[dict(code=code)])


def add(name, source, oracle, diag, calc, invocation=None, extra=None):
    assert not ('//' in source or '#' in source), name
    row = dict(name='agy2-' + name, source=source, expected=oracle,
               codes=diag, calc=calc, invocation=invocation, probes=extra or {})
    ROWS.append(row)


def pair(name, source, before, after, oracle, changed, diag, changed_diag, calc,
         fault='changed', invocation=None, extra=None, changed_extra=None):
    changed_source = replace(source, before, after)
    add(name, source, oracle, diag, calc + '\nControl member.\n', invocation, extra)
    add(name + '.broken-' + fault, changed_source, changed, changed_diag,
        calc + f'\nSingle edit: {before!r} -> {after!r}.\n', invocation, changed_extra)


def main():
    any_src = (OUT / 'rsrc-queued-cancel/model.relca').read_text()
    a, b = 'A.a', 'A.b'
    any_ok = expected([], counts=[lrow(a, 1, 1, True), lrow(b, 0, 0)],
                      framing=frow(True, [dict(call='a', capability=a)]))
    any_bad = expected([], 'violated', [lrow(a, 1, 1), lrow(b, 0, 0)], frow(False))
    any_codes = ['W-UNREACH-0']
    dead_codes = ['W-DEADLOCK-0', 'W-UNREACH-0', 'W-UNREACH-2']
    any_calc = ('T01/T18. R capacity 0 < demand 1: b remains queued, starts=0. '
                'a runs [0,2], work=2. cancel_then_drain cancels queued b iff a=pass; '
                'done/result_at=2. a=fail cannot take if_none because b never finishes. '
                'drain never completes even when a matches. Blocked sinks count a=1,b=0; '
                'only satisfiable cancellation paths set a flag (not actual cancellations).')
    pair('rsrc-any', any_src, 'result nondet { pass }', 'result nondet { fail }',
         any_ok, any_bad, any_codes, dead_codes, any_calc, fault='no-feasible-match')
    pair('rsrc-drain', any_src, 'remaining cancel_then_drain', 'remaining drain',
         any_ok, any_bad, any_codes, dead_codes, any_calc, fault='drain')
    all_src = (OUT / 'rsrc-atomic.broken-excess-demand/model.relca').read_text()
    pair('rsrc-join-all', all_src, 'uses R: 2', 'uses R: 1',
         expected([], 'violated', [lrow('A.x', 0, 0), lrow('A.y', 1, 1)], frow(False)),
         expected([], counts=[lrow('A.x', 1, 1), lrow('A.y', 1, 1)], framing=frow(True)),
         dead_codes, [], 'T03. x cannot acquire either resource partially. y runs [0,2]; '
         'R and S each occupancy-time=2, peak=1. join all blocks. When x fits, x3+y2 '
         'serialize on R/S in either order: done=5, work=5, occupancy-time each=5.', fault='fits')
    seq = '''model Seq version "0.2" {
  type V = enum { pass,fail }
  resource R capacity 1
  actor A { capacity 1;
    capability z() -> V { duration fixed 1min; result nondet { pass,fail } }
    capability x() -> V { uses R: 2; duration fixed 1min; result nondet { pass } }
    capability y() -> V { duration fixed 1min; result nondet { pass } }
  }
  process P { w = A.z(); if w == pass { y = A.y() } else { x = A.x(); y2 = A.y() }; end done }
  cut K { include [A] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    pair('rsrc-seq', seq, 'uses R: 2', 'uses R: 1',
         expected([], 'violated', [lrow('A.z', 1, 1), lrow('A.x', 0, 0), lrow('A.y', 0, 1)],
                  frow(True, selectors=[dict(call='w', capability='A.z', selects='all_contexts', blocked=True)])),
         expected([], counts=[lrow('A.z', 1, 1), lrow('A.x', 0, 1), lrow('A.y', 1, 1)],
                  framing=frow(True, [dict(call='w', capability='A.z')])),
         ['W-DEADLOCK-0', 'W-UNREACH-0'], [],
         'T02. z(pass),y reaches done at2. z(fail) blocks before x, hence y=0 on that sink. '
         'The alternative value blocks, so w selects done/all_contexts/blocked=true. '
         'After demand 2->1, fail runs z,x,y and ends at3; both values reach done, w read_only.', fault='feasible')
    ordered = '''model Ordered version "0.2" {
  type V = enum { pass,fail }
  resource R capacity 1
  actor A { capacity 2;
    capability big() -> V { uses R: 2; duration fixed 1min; result nondet { pass }; cancel cooperative ack_within 1min }
    capability small() -> V { uses R: 1; duration fixed 1min; result nondet { pass }; cancel cooperative ack_within 1min }
  }
  process P { fork f { a = A.big(); b = A.small() }; join any f where result == pass tie by [a,b] remaining cancel_then_drain if none -> done; end done }
  cut K { include [A] }
  arrivals fixed [0min]
  policy fifo tie_by case_id then call_order
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    ordered_e = expected([], 'violated', [lrow('A.big', 0, 0, reason='resource_infeasible'),
                                        lrow('A.small', 0, 0, reason='resource_infeasible')],
                         frow(False, reason='resource_infeasible'))
    loose_e = expected([], counts=[lrow('A.big', 0, 0), lrow('A.small', 1, 1, True)],
                       framing=frow(True, [dict(call='b', capability='A.small')]))
    pair('rsrc-callorder', ordered, ' then call_order', '', ordered_e, loose_e,
         ['W-DEADLOCK-0', 'W-UNREACH-0', 'W-UNREACH-0', 'W-UNREACH-2'], ['W-UNREACH-0'],
         'T04. call_order makes big the resource/Actor head; small cannot pass. F/L remain '
         'not_derived/resource_infeasible. FIFO alone ties both keys (queued_at=0,case=1), '
         'so small starts at0, matches at1; big starts0 times, small1 time, work1.', fault='no-call-order')
    pair('scenario-callorder-substrate', ordered, ' then call_order', '', ordered_e, loose_e,
         ['W-DEADLOCK-0','W-UNREACH-0','W-UNREACH-0','W-UNREACH-2'], ['W-UNREACH-0'],
         'T17 non-scenario controls. Same hand paths as T04; '
         'only call_order changes. This pair fixes the selected IR Agency contract before '
         'scenario selection APIs exist. The actual Ordered/Loose scenario crossing was '
         'deferred to integration tests with explicit inputs, expectations and targets.',fault='loose')
    pair('jue', W, GUARD, '', expected([effect()]),
         expected([effect(js=[judgement(stops='none', bypasses=1)])]),
         ['W-RESP-0'] * 2, ['W-AGY-0', 'W-RESP-0'],
         'T05. e(entry) then v(pass|fail). pass reaches posted at3, fail returned at2; '
         'post count0..1. One context, only pass permits k: v all_paths/0. Without the '
         'guard both values reach posted at3: none/1, J retained and U empty. '
         'O=c1 is excluded from single_person; anonymous E makes unknown. Root=e, not Doc.', fault='no-guard')
    concentration = '''model Concentration version "0.2" {
  type Entry = enum { entry }
  type V = enum { pass,fail }
  type Ack = enum { committed }
  people [p1,p2,p3]
  actor Doc { source document }
  actor A { executors [p1];
    capability enter() -> Entry { reads [Doc]; duration fixed 1min; result nondet { entry } }
    capability approve(e: Entry) -> V { duration fixed 1min; result nondet { pass,fail } }
    capability post(v: V) -> Ack { commits; duration fixed 1min; result nondet { committed } }
  }
  process Flow { e = A.enter(); v = A.approve(e); if v == pass {} else { end returned }; k = A.post(v); end posted }
  cut Scope { include [Doc,A] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    # A single structural relocation is recorded as the exact three necessary
    # textual changes (declaration, call owner and Cut), not three semantic faults.
    def split_ledger(s, person):
        s = replace(s, '    capability post(v: V) -> Ack { commits; duration fixed 1min; result nondet { committed } }\n', '')
        s = replace(s, '  process Flow', f'  actor Ledger {{ executors [{person}]; capability post(v: V) -> Ack {{ commits; duration fixed 1min; result nondet {{ committed }} }} }}\n  process Flow')
        s = replace(s, 'k = A.post(v)', 'k = Ledger.post(v)')
        return replace(s, 'include [Doc,A]', 'include [Doc,A,Ledger]')
    def ce(actor='A', ep=('p1',), jp=('p1',), sa=True, sp=True):
        return effect(actor, ep, [judgement('A', jp)], sa, sp,
                      roots=[dict(call='e', capability='A.enter')])
    cc = ('T06/T14/T15. Sequential durations: returned2, posted3. Generation root=e/A.enter. '
          'Source Doc is not a carrier. J/U=A; E=A or Ledger. single_actor requires singleton '
          'O/J/U/E with nonempty U. single_person unions J/U/E only, deduplicating shared IDs. '
          'W-AGY-2 is OR, once per effect. Relocating post also retargets its call and Cut.')
    for name, s, er, person in [
        ('concentration', concentration, ce(), 'p2'),
        ('single-actor-many-people', replace(concentration, 'executors [p1]', 'executors [p1,p2]'),
         ce(ep=('p1','p2'), jp=('p1','p2'), sp=False), 'p3'),
    ]:
        add(name, s, expected([er]), ['W-AGY-2', 'W-RESP-0'], cc)
        jp = tuple(er['j'][0]['people'])
        add(name+'.broken-separate-ledger', split_ledger(s, person),
            expected([ce('Ledger', (person,), jp, False, False)]), ['W-RESP-0']*2, cc)
    shared = split_ledger(concentration, 'p1')
    pair('single-person-many-actors', shared, 'actor Ledger { executors [p1]', 'actor Ledger { executors [p2]',
         expected([ce('Ledger', ('p1',), ('p1',), False, True)]),
         expected([ce('Ledger', ('p2',), ('p1',), False, False)]),
         ['W-AGY-2', 'W-RESP-0', 'W-RESP-0'], ['W-RESP-0']*2, cc, fault='different-person')
    two = replace(replace(W, 'people [c1, a1]', 'people [c1, a1, a2]'), 'executors [a1]', 'executors [a1,a2]')
    pair('known-two-plus-anonymous', two, 'executors [a1,a2]', 'executors [a1]',
         expected([effect(js=[judgement(people=('a1','a2'))], single_person=False)]), expected([effect()]),
         ['W-RESP-0']*2, ['W-RESP-0']*2 + ['W-UNREACH-3'],
         'T16. Known J/U people a1,a2 already establish at least two, even with anonymous '
         'Ledger. Removing a2 from the pool leaves a1 plus anonymous -> unknown; unused '
         'declared people not in any executor pool do not cause W-UNREACH-3.', fault='one-known')
    # Only executor-pool people are diagnosed as never assigned; a2 leaves the pool.
    ROWS[-1]['codes'] = ['W-RESP-0']*2
    permit = replace(two, '  process Flow', '  permit a1 to Approver.approve\n  process Flow')
    pair('permission-not-filter', permit, 'permit a1', 'permit a2',
         expected([effect(js=[judgement(people=('a1','a2'), permitted=('a1',))], single_person=False)]),
         expected([effect(js=[judgement(people=('a1','a2'), permitted=('a2',))], single_person=False)]),
         ['W-RESP-0']*2, ['W-RESP-0']*2,
         'T21. No permitted property: assignments still include a1,a2. Permission is a '
         'normative set, not a reachability filter. Only permitted changes; people remains '
         '[a1,a2] for both J and U, single_person=false.', fault='other-permit')
    atomic = replace(W, 'type Ack = enum { committed }', 'type Ack = enum { committed, rejected_duplicate }')
    atomic = replace(atomic, 'actor Ledger { capacity 1; capability post(v: V) -> Ack { commits; duration fixed 1min; result nondet { committed } } }',
                     'actor Ledger { capacity 1; state keys: set<CaseId>; capability post(key: CaseId) -> Ack { commits; writes keys; duration fixed 1min; atomic { if key in keys { result rejected_duplicate } else { keys += key; result committed } }; cancel forbidden_after_start } }')
    atomic = replace(atomic, 'Ledger.post(v)', 'Ledger.post(case.id)')
    state = [dict(actor='Ledger', capability='Ledger.post', reads=['Ledger.keys'], on_normal_path=False)]
    ae = effect(state=state)
    unknown_state = copy.deepcopy(state)
    unknown_state[0]['on_normal_path'] = 'unknown'
    unknown = effect(state=unknown_state, unknown=True)
    acalc = ('T07/T19. keys initially empty. All insertions are the current case.id. '
             'First post without start/earlier insertion cannot reject: state_refuser Ledger '
             'on_normal_path=false and v all_paths/0. Legacy refusal remains [] with state_guards. '
             'A second post sees prior insertion, or its own start inserts before its test: '
             'unknown, process_judgements not_derived/state_guards, no j/u keys. '
             'pass first commit at3, second normal rejection at4; fail returned2. '
             'Counts first post0..1, repeated post0..2; self-start commits0 but starts0..1.')
    pair('atomic-u', atomic, 'k = Ledger.post(case.id);', 'k = Ledger.post(case.id); k2 = Ledger.post(case.id);',
         expected([ae], counts=[lrow('Clerk.enter',1,1),lrow('Approver.approve',1,1),lrow('Ledger.post',0,1)]),
         expected([ae, dict(unknown, call='k2')], counts=[lrow('Clerk.enter',1,1),lrow('Approver.approve',1,1),lrow('Ledger.post',0,2)]), ['W-RESP-0'], ['W-RESP-0'], acalc,
         fault='second-post', extra={'/agency/refusal': []}, changed_extra={'/agency/refusal': []})
    pair('atomic-self-start', atomic, 'commits; writes keys;', 'commits; writes keys; start { keys += key };',
         expected([ae]), expected([unknown]), ['W-RESP-0'], ['W-RESP-0'], acalc, fault='start-inserts',
         extra={'/agency/refusal': []}, changed_extra={'/agency/refusal': []})
    # Two configurations, identical declaration universe and binding keys.
    configured = replace(W, '  process Flow', '  actor Extractor { capacity 1; capability extract() -> Entry { reads [Doc]; duration fixed 1min; result nondet { entry } } }\n  process Flow')
    configured = replace(configured, 'include [Doc, Clerk, Approver, Ledger]', 'include [Doc, Clerk, Extractor, Approver, Ledger]')
    flow = 'e = Clerk.enter(); v = Approver.approve(e); '+GUARD+'k = Ledger.post(v); end posted'
    to_flow = replace(flow, 'Clerk.enter()', 'Extractor.extract()')
    configured = replace(configured, '  cut Scope', '  process ToFlow { '+to_flow+' }\n  configuration From { cut Scope; process Flow }\n  configuration To { cut Scope; process ToFlow; intent { retire Clerk.enter from normal_path keep fallback; add Extractor.extract reason "replace entry" } }\n  cut Scope')
    fallback_l = [lrow('Clerk.enter', 0, 0, fallback='kept_by_intent'), lrow('Approver.approve', 1, 1),
                  lrow('Ledger.post', 0, 1), lrow('Extractor.extract', 1, 1)]
    to_e = effect(roots=[dict(call='e', capability='Extractor.extract')])
    cfgdiag = ['W-AGY-3'] + ['W-RESP-0']*2 + ['W-UNREACH-0', 'W-UNREACH-3']
    pair('keep-fallback', configured, 'retire Clerk.enter from normal_path keep fallback', 'retire Clerk.enter reason "retired"',
         expected([to_e], counts=fallback_l),
         expected([to_e], counts=[dict(r, fallback='unchecked') for r in fallback_l]),
         cfgdiag, [x for x in cfgdiag if x != 'W-AGY-3'],
         'T08. From calls Clerk1, Extractor0; To calls Clerk0, Extractor1. Both return2/post3. '
         'L Clerk=0..0 in selected To and kept_by_intent only for new retire. W-AGY-3 once; '
         'basis remains unknown, fallback_dependency unchecked, no deletion advice. '
         'Diff relation removed carries keep_fallback and suppresses only W-DIFF-2; plain '
         'retire loses no recorded capability and therefore retains W-DIFF-2.', fault='plain-retire',
         invocation=dict(command='check', configuration='To'))
    reads = replace(W, '  actor Doc', '  observed case { entry: Entry = entry; po: V = pass }\n  latent case { truth: bool = false }\n  actor Doc')
    reads = replace(reads,'actor Doc { source document }','actor Doc { source document }\n  actor PO { source record }')
    reads = replace(reads, 'reads [Doc];', 'reads [Doc]; writes entry;')
    reads = replace(reads, 'result nondet { entry } } }', 'result nondet { entry } } capability match(e: Entry) -> V { reads [e,PO]; writes po; duration fixed 1min; result nondet { pass,fail } } }')
    reads = replace(reads, 'approve(e: Entry) -> V {', 'approve() -> V { reads [entry,po];')
    reads = replace(reads, 'v = Approver.approve(e)', 'm = Clerk.match(e); v = Approver.approve()')
    reads = replace(reads, 'include [Doc, Clerk, Approver, Ledger]', 'include [Doc, PO, Clerk, Approver, Ledger]')
    re = effect(js=[judgement('Clerk', ('c1',), 'none', 1, 'm', 'match'), judgement()], single_person=False)
    ro = expected([re])
    ro['agency']['rules'] = 'agency-p1-reads-1'
    rc = ('T09/T22. enter writes entry from Doc. m reads e and PO, writes po; v reads '
          'entry,po and has parents e,m. m has parent e, so generation roots=[e/Clerk.enter], '
          'not [Doc,PO]. v subject_sources=[Doc,PO], independent=[]; m subject=[Doc], '
          'independent=[PO]. m result does not constrain nondet v: m none/1, v all_paths/0. '
          'J=m,v and U=v. J people c1,a1 establish two before anonymous Ledger. '
          'returned at3, posted at4; enter/m/v each1, post0..1. Latent read is invalid E-NAME-0.')
    pair('reads-observed', reads, 'reads [entry,po]', 'reads [entry,po,truth]', ro, refused('invalid','E-NAME-0'),
         ['W-AGY-0','W-RESP-0','W-RESP-0'], ['E-NAME-0'], rc, fault='latent-read')
    diff_inv = dict(command='diff', **{'from':'From', 'to':'To'})
    differences = []
    def d(viewpoint, item, key, old, new):
        differences.append(dict(viewpoint=viewpoint, item=item, **key, status='changed', **{'from':old,'to':new}))
    d('observation','normal.actors',dict(actor='Doc'), ['Clerk'], ['Extractor'])
    d('observation','normal.people',dict(actor='Doc'), ['c1'], [])
    d('observation','normal.unattributed_actors',dict(actor='Doc'), [], ['Extractor'])
    d('observation','normal.direct',dict(person='c1'), ['Doc'], [])
    d('origin','direct',dict(actor='Clerk'), ['Doc'], [])
    d('origin','direct',dict(actor='Extractor'), [], ['Doc'])
    d('retention','normal',dict(capability='Clerk.enter'), dict(min=1,max=1), dict(min=0,max=0))
    d('retention','normal',dict(capability='Extractor.extract'), dict(min=0,max=0), dict(min=1,max=1))
    d('retention','fallback',dict(capability='Clerk.enter'), 'unchecked','kept_by_intent')
    d('effects','concentration.generation_roots',dict(effect='Ledger.post',call='k'),
      [dict(call='e',capability='Clerk.enter')],[dict(call='e',capability='Extractor.extract')])
    diff_e = dict(verdict='satisfied', agency=dict(differences=dict(rules='agency-diff-p2-1',status='derived',rows=differences,
                  unchecked=['fallback_path','workload_transfer','via_model_paths'])))
    # W-UNREACH-1 is a side diagnostic; root codes are checked separately in core.
    diff_src = replace(configured, '  actor Doc', '  observed case { flag: bool = false }\n  actor Doc')
    diff_bad = copy.deepcopy(diff_e)
    def unknown_diff(item, **key):
        return dict(viewpoint='framing',item=item,**key,status='not_comparable',
                    **{'from':dict(status='derived',reason=None),'to':dict(status='not_derived',reason='state_guards')})
    diff_bad['agency']['differences']['rows'] = differences[:6] + [unknown_diff(i,end='posted') for i in ['reachable','selectors','read_only']] + [unknown_diff('actors')] + differences[6:]
    pair('diff-items', diff_src, 'process ToFlow { '+to_flow,
         'process ToFlow { '+replace(to_flow,'end posted','if flag == true {} else {}; end posted'),
         diff_e, diff_bad, None, None,
         'T10. Exactly ten changed meaning rows: Doc actors/people/anonymous, c1 direct, '
         'Clerk/Extractor direct, Clerk normal/fallback, Extractor normal, roots. '
         'No same-valued framing/refusal/group/concentration flags. A constant false '
         'Order is viewpoint, then item, then declaration key: retention normal Clerk, '
         'normal Extractor, fallback Clerk (the adopted order corrects the initial plan). '
         'empty state guard before To posted adds four not_comparable rows: posted '
         'reachable/selectors/read_only and global framing actors (complete=false). '
         'returned ends before this guard; L stays unchanged. Root structural diff '
         'diagnostics W-DIFF-1/3 are separate from side diagnostics.', fault='state-guard', invocation=diff_inv)
    pair('resp0-undeclared', W, '  process Flow', '  responsible a1 for Approver.approve\n  process Flow',
         expected([effect()]), expected([effect()]), ['W-RESP-0']*2, ['W-RESP-1','W-RESP-0'],
         'T11. No metadata still requires warnings for U Approver and E Ledger, not O Clerk. '
         'Adding duty for a1 covers Approver via executors, leaving Ledger warning. '
         'Approver direct=[] and indirect=[Doc], so W-RESP-1 once; indirect is not direct.', fault='declared')
    # T12: each metadata dimension is an independent pair on both structures.
    for suffix, original, oracle, inv, diag in [('effect',concentration,expected([ce()]),None,['W-AGY-2','W-RESP-0']),
                                              ('diff',configured,diff_e,diff_inv,None)]:
        for label, old, new in [('name','Clerk' if suffix=='diff' else 'A', 'Renamed'),
                                ('grain','actor '+('Clerk' if suffix=='diff' else 'A')+' {', 'actor '+('Clerk' if suffix=='diff' else 'A')+' { grain model;'),
                                ('boundary','actor '+('Clerk' if suffix=='diff' else 'A')+' {', 'actor '+('Clerk' if suffix=='diff' else 'A')+' { boundary "opaque unit";'),
                                ('via','actor '+('Clerk' if suffix=='diff' else 'A')+' {', 'actor '+('Clerk' if suffix=='diff' else 'A')+' { via model "Parser@v1";')]:
            if label == 'name':
                # Identifier-only replacement: do not rewrite Ack/Approver/model title.
                import re
                changed_s = re.sub(r'\b'+old+r'\b',new,original)
                assert changed_s != original
                changed_o = json.loads(re.sub(r'\b'+old+r'\b',new,json.dumps(oracle,ensure_ascii=False)))
                add(f'label-{suffix}-{label}',original,oracle,diag,'T12. Name-only bijection; normalize names, effects/differences unchanged.',inv)
                add(f'label-{suffix}-{label}.broken-label',changed_s,changed_o,diag,'T12. Name-only bijection; no semantic information in labels.',inv)
            else:
                pair(f'label-{suffix}-{label}',original,old,new,oracle,oracle,diag,diag,
                     'T12. Change only '+label+'. O/J/U/E and generation roots depend on reads/calls/people, not metadata. '
                     'via_model_paths, spans, input hash and actor metadata are intentionally outside this equality.',fault='label',invocation=inv)
    # T13 grammar/name matrix. Each starts from a valid independent control.
    for suffix, old, new, verdict, code in [
        ('actor','retire Clerk.enter','retire Clerk','unsupported','U-SYN-0'),
        ('path','normal_path','contest_path','unsupported','U-SYN-0'),
        ('action','retire Clerk.enter','add Clerk.enter','invalid','E-SYN-0'),
        ('missing-fallback','keep fallback','keep','invalid','E-SYN-0'),
        ('still-called','process ToFlow { e = Extractor.extract()','process ToFlow { e = Clerk.enter()','invalid','E-DIFF-0'),
    ]:
        fault_e = refused(verdict,code)
        fault_codes = [code]
        if suffix != 'still-called':
            fault_codes += ['W-CUT-0']
            fault_e['diagnostics'].append(dict(code='W-CUT-0'))
        pair('static-'+suffix,configured,old,new,expected([to_e]),fault_e,cfgdiag,fault_codes,
             'T13. Control To is legal keep-fallback. One '+suffix+' change only; '
             'Actor/path unsupported, action/missing token invalid, lexical still-called E-DIFF-0. '
             'No Agency or numeric values after static failure; W-AGY-3 must not appear. '
             'Parser failures retain existing W-CUT-0 because no AST exists; '
             'lower-only still-called failure has no such warning.',fault='rejected',
             invocation=dict(command='check',configuration='To'))
    state_reads = replace(W,'actor Ledger { capacity 1;', 'actor Ledger { capacity 1; state keys: set<CaseId>;')
    pair('static-actor-state', state_reads, 'reads [Doc]', 'reads [Ledger.keys]', expected([effect()]),
         refused('unsupported','U-SYN-0'), ['W-RESP-0']*2,['U-SYN-0'],
         'T13c. Parse full qualified read name; lower refuses Actor state reads with U-SYN-0 '
         '(not E-SYN-0, and do not truncate Ledger.keys to Ledger).', fault='qualified')
    pair('static-param',W,'approve(e: Entry) -> V {','approve(e: Entry) -> V { reads [e];',
         expected([effect()]),expected([effect()]),['W-RESP-0']*2,['W-RESP-0']*2,
         'T13h. Param reads are accepted and checked but add no new edge and do not remove '
         'other actual-argument edges. Legacy lineage/origin/refusal/rules exactly equal.',fault='explicit-param')
    writer = '''model Writer version "0.2" {
  type V = enum { pass,fail }
  observed case { x: V = pass }
  actor W { capacity 1; capability write() -> V { writes x; duration fixed 1min; result nondet { pass } } }
  actor R { capacity 1; capability read() -> V { reads [x]; duration fixed 1min; result nondet { pass } } }
  process P { w = W.write(); r = R.read(); end done }
  cut K { include [W,R] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    for suffix, calls in [('writer-first','w = W.write(); r = R.read()'),('reader-first','r = R.read(); w = W.write()')]:
        pair('static-fork-'+suffix, writer, 'w = W.write(); r = R.read();', 'fork f { '+calls+' }; join all f;',
             expected([]),refused('unsupported','U-SEM-0'),[],['U-SEM-0'],
             'T13g. Sequential writer then reader: one edge, no commits, legal. Fork siblings '
             'have ambiguous start snapshot in either lexical order. Reader does not write x; '
             'this must fail read variable has a concurrent writer, not write/write conflict.',fault='concurrent')
    later = replace(reads, '  actor Doc', '  actor Extra { source record }\n  actor Later { capacity 1; capability write() -> Entry { reads [Extra]; writes entry; duration fixed 1min; result nondet { entry } } }\n  actor Doc')
    later = replace(later,'include [Doc, PO, Clerk, Approver, Ledger]','include [Doc, PO, Extra, Later, Clerk, Approver, Ledger]')
    pair('later-writer',later,'k = Ledger.post(v)','late = Later.write(); k = Ledger.post(v)',ro,ro,
         ['W-AGY-0','W-RESP-0','W-RESP-0','W-UNREACH-0'],['W-AGY-0','W-RESP-0','W-RESP-0'],
         rc+' T22. Late writer occurs after v snapshot and guard; Extra is not v ancestry. '
         'Later count0..0 ->0..1, returned3 unchanged, posted4->5.',fault='later-call')
    initial = replace(W,'  actor Doc','  observed case { x: V = pass }\n  actor S { source record }\n  actor Writer { capacity 1; capability write() -> V { reads [S]; writes x; duration fixed 1min; result nondet { pass } } }\n  actor Doc')
    initial = replace(initial,'approve(e: Entry) -> V {','approve() -> V { reads [x];')
    initial = replace(initial,'v = Approver.approve(e)','v = Approver.approve()')
    initial = replace(initial,'include [Doc, Clerk, Approver, Ledger]','include [Doc, S, Writer, Clerk, Approver, Ledger]')
    pair('initial-read',initial,'v = Approver.approve()','w = Writer.write(); v = Approver.approve()',
         expected([effect(js=[],roots=[])]),expected([effect(roots=[dict(call='w',capability='Writer.write')])]),
         ['W-RESP-0','W-UNREACH-0'],['W-RESP-0','W-RESP-0'],
         'T23. Initial x has no writer edge, so v is not J and roots empty, old agency.rules. '
         'e remains unrelated. Adding w before v creates only parent w, source S, J/U=v '
         'all_paths, root=w, reads rules. Writer count0->1; posted3->4, returned2->3.',fault='preceding-writer')
    only_atomic = '''model OnlyAtomic version "0.2" {
  type Ack = enum { committed,rejected_duplicate }
  people [p]
  actor Ledger { capacity 1; state keys: set<CaseId>; capability post(key: CaseId) -> Ack { commits; writes keys; duration fixed 1min; atomic { if key in keys { result rejected_duplicate } else { keys += key; result committed } }; cancel forbidden_after_start } }
  process P { k = Ledger.post(case.id); end done }
  cut K { include [Ledger] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    only_e = effect(js=[],roots=[],state=state,single_actor=True)
    pair('no-people',only_atomic,'  people [p]\n','',expected([only_e], counts=[lrow('Ledger.post',1,1)]),expected([only_e], counts=[lrow('Ledger.post',1,1)]),
         ['W-AGY-2','W-RESP-0'],['W-AGY-2'],
         'T24. Single first atomic starts/commits at1, count1..1. Owner Ledger is E and '
         'state J/U so single_actor=true, people unknown. Removing people does not change '
         'this warning, but removes W-RESP-0 and adds responsibility_people_undeclared once.',fault='undeclared')
    state_only = '''model StateOnly version "0.2" {
  type Ack = enum { committed }
  actor A { capacity 1; state n: 0..1 = 0; capability post() -> Ack { writes n; finish { n += 1 }; duration fixed 1min; result nondet { committed } } }
  process P { k = A.post(); end done }
  cut K { include [A] }
  bounds { cases 1; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    se = expected([])
    se['agency']['viewpoints']['effects'] = effects([],True)
    pair('state-write-only',state_only,'writes n;', 'commits; writes n;',se,
         expected([effect('A',js=[],roots=[])]),[],[],
         'T25. A writes observed Actor n=0+1=1 at finish1, starts1. Noncommits write is '
         'not an E row but state_write_effects unchecked. Adding commits creates E row '
         'and removes that unchecked, not single_actor (no U), anonymous single_person unknown.',fault='commits')
    version = '''model VersionAgency version "0.2" {
  type Ack = enum { ok,denied }
  people [p]
  actor Ledger { capacity 2; state n: 0..2 = 0;
    capability post() -> Ack { commits; duration fixed 3min; result nondet { ok };
      writes n; finish { n += 1 }; version_check [n] else denied }
  }
  process P { k = Ledger.post(); end done }
  cut K { include [Ledger] }
  arrivals fixed [0min,0min]
  bounds { cases 2; live_tasks 3; clock 20min }
  verify safety no_resource_overcommit
}
'''
    vr=[dict(actor='Ledger',capability='Ledger.post',reads=['Ledger.n'],on_normal_path='unknown')]
    pair('version-check',version,'; version_check [n] else denied','',
         expected([effect(js=[],roots=[],state=vr,unknown=True)]),expected([effect(js=[],roots=[])]),
         ['W-RESP-0'],['W-RESP-0'],
         'T20. Both cases start at0, finish at3, work3 each. First finish n=1; version '
         'mark makes second denied/no write, final n=1. Removing only version_check leaves '
         'two commutative +1 writes, final n=2, state_in_range and safety satisfied. '
         'Static count per case1..1. With version_check state_refuser Ledger.n unknown; '
         'no process j/u keys and legacy refusal omitted. Without it ordinary E with '
         'empty J/U, no state_refusers. A5 is a prerequisite; C5 owns the Agency assertions.',fault='no-check')
    resp_diff = replace(configured, 'capability approve(e: Entry)', 'capability inspect() -> Entry { reads [Doc]; duration fixed 1min; result nondet { entry } } capability approve(e: Entry)')
    resp_diff = replace(resp_diff, '  process Flow', '  responsible a1 for Approver.approve\n  process Flow')
    resp_diff = replace(resp_diff, 'process ToFlow { e = Extractor.extract()', 'process ToFlow { e = Clerk.enter(); i = Approver.inspect()')
    resp_diff = replace(resp_diff, 'intent { retire Clerk.enter from normal_path keep fallback; add Extractor.extract reason "replace entry" }','intent { }')
    # T26 only inspects responsibility rows; exact O payloads are independently fixed.
    pair('responsibility-diff',resp_diff,'; i = Approver.inspect()','',dict(verdict='satisfied'),dict(verdict='satisfied'),None,None,
         'T26. Duty key (a1, capability, Approver.approve) is identical on both sides. '
         'From observation={status:missing,reason:null,required_sources:[Doc],direct:[], '
         'indirect:[Doc],missing_sources:[Doc]}; To direct=[Doc],status=present,missing_sources=[]. '
         'No within: both time_frame unchecked, so not_comparable even when identical. '
         'Removing only To inspect removes changed observation but not unknown time_frame. '
         'Extra unused Extractor warning exists on both sides and is not a new C5 warning.',fault='no-inspect',invocation=diff_inv)
    # The exact population and all hand probes are committed; runtime never imports this generator.
    names = []
    for row in ROWS:
        name = row['name']
        names.append(name)
        d = OUT / name
        d.mkdir(exist_ok=True)
        (d/'calc.md').write_text('# ADR 0043 hand derivation\n\n'+row['calc']+'\n\n'
            'The expected values above are algebraic/static hand oracles, not captured output. '
            'Stage C RED is not implemented semantics, calibrated mutation detection, '
            'real-world legitimacy, skill retention, or all-schedule safety.\n')
        (d/'expected.json').write_text(json.dumps(row['expected'],ensure_ascii=False,indent=2)+'\n')
        (d/'model.relca').write_text(row['source'])
        if row['invocation']:
            (d/'invocation.json').write_text(json.dumps(row['invocation'],indent=2)+'\n')
        # W-RESP-0 identities follow the hand E/U/state-owner table, not a CLI.
        import re
        targets=set()
        es=row['expected'].get('agency') or {}
        es=es.get('viewpoints',{}).get('effects',{}).get('rows',[])
        if re.search(r'\bpeople\s*\[',row['source']):
            duties=set(re.findall(r'\bresponsible\s+(\w+)\s+for\b',row['source']))
            for e in es:
                if e['state_refusers']:
                    targets.update(s['actor'] for s in e['state_refusers'])
                    targets.update(e['e']['actors'])
                else:
                    targets.update(e['e']['actors'])
                    for j in e.get('u',[]):targets.update(j['actors'])
            for e in es:
                for carrier in [e['e']]+e.get('j',[]):
                    if set(carrier['people']).intersection(duties):targets.difference_update(carrier['actors'])
        row['responsibility_targets']=[a for a in re.findall(r'\bactor\s+(\w+)\s*\{',row['source']) if a in targets]
    assert len(names)==len(set(names))
    here = Path(__file__).parent
    (here/'agency_second_half_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (here/'agency_second_half_names.rs').write_text('#[allow(dead_code)]\npub const AGENCY_SECOND_HALF_FIXTURES: &[&str] = &[\n' + ''.join('    '+json.dumps(n)+',\n' for n in names) + '];\n')
    (here/'agency_second_half_population.json').write_text(json.dumps([
        {k:v for k,v in r.items() if k not in ('source','expected','calc')}
        for r in ROWS],ensure_ascii=False,indent=2)+'\n')
    print(len(names),'hand-authored inputs')


if __name__ == '__main__':
    main()
