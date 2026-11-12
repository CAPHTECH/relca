"""ADR0040 hand-authored models and arithmetic; this generator never runs Relca.

Only sov-* fixtures and this population's two JSON inventories are generated.
P14 (atomic) was materialized after ADR 0035 was integrated (atomic()); P27 and
the B4 joins are tests in crates/relca-core/tests/effect_standing_deferred.rs.
"""
import copy
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
FIXTURES = ROOT/'fixtures'
PAIRS = []
ORACLES = {}
GRANT = '  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }\n'
CLAUSE = 'contest Vendor on Ledger.post.commit route Clerk.recheck reversible_by [a1] respond_by 5min'
POST = 'ack = Ledger.post(); end posted'
BASE = '''model StandingEffects version "0.2" {
  type Verdict = enum { pass, fail }
  type Ack = enum { committed }
  people [c1, a1, cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 2min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } } }
  actor Clerk { executors [c1]; capability recheck() -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Vendor { control party; standing over [Ledger.post.commit] basis "posting claim" }
  permit a1 to Approver.approve
''' + GRANT + '''  process Flow { approval = Approver.approve(); if approval == fail { end returned } else { }; ack = Ledger.post(); end posted }
  cut Scope { include [Approver, Ledger, Clerk, Vendor] }
  bounds { cases 1; live_tasks 2; clock 20min }
  verify safety no_resource_overcommit
  contract Controls { ''' + CLAUSE + ''' }
}
'''


def edit(src, old, new):
    assert src.count(old) == 1, (old, src.count(old))
    changed = src.replace(old, new, 1)
    assert changed != src
    return changed


def r(lo, hi=None):
    return {'min': lo, 'max': lo if hi is None else hi}


def scalar(lo, hi=None, reason=None):
    return dict(min=None if lo is None else str(lo),
                max=None if lo is None else str(lo if hi is None else hi), reason=reason)


def terminal(name, elapsed, work):
    return dict(end=name, elapsed=r(*elapsed) if isinstance(elapsed, tuple) else r(elapsed),
                work=r(work), result_at=None)


def alt(at, deadline=None, response=None, result=None, duration=1, reason=None):
    filed = deadline is not None
    status = ('not_filed' if not filed else 'unresolved' if response is None and reason in (None, 'deadline_passed_unresolved')
              else 'violated' if reason or response > deadline else 'responded')
    return dict(filed=filed, target_at=at, filed_at=at if filed else None,
                deadline=deadline, status=status, response_at=response,
                closed_at=at if not filed else response, executor='c1' if response is not None else None,
                result=result, violation_reason=reason or ('late_response' if status == 'violated' else None),
                route_work=r(duration) if response is not None else None,
                route_wait=r(response-at-duration) if response is not None else None,
                route_cost=scalar(None, reason='not_configured' if filed and reason!='missing_route' else 'never_created'))


def hand(posted: int | tuple[int,int] | None=3, work=3, respond=(4,), by=5, duration=1, static='satisfied', runtime='satisfied',
         verdict=None, summary=1, target='Ledger.post.commit', returned=True):
    out: dict = dict(verdict=verdict or ('violated' if 'violated' in (static, runtime) else runtime),
               bounds={'truncated': False}, terminals=([terminal('returned', 2, 2)] if returned else []) +
               ([] if posted is None else [terminal('posted', posted, work)]))
    out['contest_paths'] = [dict(id='contract Controls #1 contest', rules='contest-p1-2', party='Vendor',
                                end=None, effect=target, route='Clerk.recheck', respond_by=by,
                                verdict=static, basis='構造上存在' if static == 'satisfied' else '構造の不足',
                                checks=dict(route_exists=dict(satisfied=True,reason=None),
                                            reversal_authority=dict(satisfied=static=='satisfied' or by==0),
                                            deadline_defined=dict(satisfied=by>0,reason=None if by>0 else 'nonpositive_deadline')),
                                missing=[] if static=='satisfied' else ['deadline_defined' if by==0 else 'reversal_authority'])]
    alts = [] if runtime == 'vacuous' else [alt(3)] + [alt(3, 3+by, t, v, duration) for t in respond for v in ('pass','fail')]
    out['contest_runtime'] = [dict(case_id=1, static_id='contract Controls #1 contest', target=target,
                                 target_kind='effect', alternatives=alts)]
    out['contest_summary'] = {'business_ended_open': scalar(0, summary)}
    return out


def refusal(code, message=None):
    return dict(verdict='unsupported' if code.startswith('U') else 'invalid', properties=[], terminals=[],
                diagnostics=[dict(code=code)] + ([dict(code='W-CUT-0')] if code=='U-SYN-0' else []))


def write(name, src, expected, derivation, invocation=None, oracle=None):
    assert name.startswith('sov-') and '//' not in src and '#' not in src
    directory = FIXTURES/name
    directory.mkdir(exist_ok=True)
    (directory/'model.relca').write_text(src)
    expected=copy.deepcopy(expected)
    if 'contest_runtime' in expected:
        cut=expected['bounds']['truncated']
        properties=[dict(id='safety no_resource_overcommit',verdict='inconclusive' if cut else 'satisfied')]
        for path in expected['contest_paths']:
            rows=[row for row in expected['contest_runtime'] if row.get('static_id',path['id'])==path['id']]
            alts=[a for row in rows for a in row['alternatives']]
            violated=any(a['status']=='violated' or a['violation_reason']=='deadline_passed_unresolved' for a in alts)
            runtime='violated' if violated else 'inconclusive' if cut else 'satisfied' if alts else 'vacuous'
            properties.extend([dict(id=path['id'],verdict=path['verdict']),dict(id=path['id']+'_response',verdict=runtime)])
        expected['properties']=properties
    (directory/'expected.json').write_text(json.dumps(expected, ensure_ascii=False, indent=2)+'\n')
    (directory/'calc.md').write_text('# '+name+'\n\nHand oracle (minutes), fixed before implementation. ADR 0040; design v0.2 §22.3.\n\n'+derivation+
        '\n\nCommit finish → same-tick filing/queue management → ordinary process/End → dispatch. '
        'Filing is nondeterministic, not a probability. Route work is excluded from the business '
        'terminal and included in occupation/overall work. A business End is not execution close. '
        'Existing responsibility/completion warnings are not errors. RED is not semantic verification.\n')
    if invocation:
        (directory/'invocation.json').write_text(json.dumps(invocation)+'\n')
    ORACLES[name] = oracle or {}


def pair(id_, name, src, old, new, good, bad, derivation, fault='change', invocation=None):
    broken=name+'.broken-'+fault
    write(name, src, good, id_+': '+derivation, invocation)
    write(broken, edit(src, old, new), bad, id_+': Single edit '+repr(old)+' → '+repr(new)+'. '+derivation, invocation)
    PAIRS.append(dict(id=id_, good=name, bad=broken, old=old, new=new))


def route_start():
    # P32 (boundary review M2). Added after 41f2305 hand-corrected four
    # committed oracles, so it is generated alone (main() is not rerun).
    src=edit(BASE,'people [c1, a1, cfo]','people [a1, cfo]')
    src=edit(src,'actor Clerk { executors [c1];','actor Clerk { capacity 1;')
    good=hand()
    for a in good['contest_runtime'][0]['alternatives']:
        a['executor']=None
    bad=hand(runtime='violated')
    stuck=alt(3,8)
    stuck.update(status='violated',violation_reason='deadlock',route_wait=r(0),
                 route_cost=scalar(None,reason='never_created'))
    bad['contest_runtime'][0]['alternatives']=[alt(3),stuck]
    pair('P32','sov-route-start',src,'actor Clerk { capacity 1;','actor Clerk { capacity 0;',good,bad,
         'Anonymous Clerk capacity1 (no person): approve fail End2/work2; pass post2..3, commit3, End3/work3. '
         'Filed route recheck3..4 responds4 <= deadline8, executor null. Declined closes3. Summary0..1 '
         '(End3 sees the filed route queued before dispatch). Broken capacity0: End2/End3 and work unchanged; '
         'the filed route is queued at3 and can never start, so after the business End no transition is '
         'enabled (W-DEADLOCK-0) at3 < deadline8. A filed route with no response at a deadlock is violated '
         '(deadlock), not a late response and not satisfied: contest_response violated, root violated (exit1). '
         'Static contest stays satisfied (route in Cut, a1 via Approver guard). Same shape as '
         'ctx-contest-branches.broken-zero-capacity (terminal target), here with an effect target. '
         'response/closed/executor/result/route_work null. route_wait0 (queued3, observed3) and route_cost '
         'never_created (route never started) follow the ADR 0022 row fields; these two were written after '
         'one look at the engine output, all other values before it. Executors [c1] with capacity0 would be '
         'E-CAP-0 (capacity differs from executor count), hence the anonymous Clerk.', 'zero-capacity')


def atomic():
    # P14 / F07 (CaseId/set integration). Generated alone
    # like route_start: main() is not rerun over hand-corrected oracles.
    own=edit(BASE,'people [c1, a1, cfo]','people [c1, a1, cfo, l1]')
    own=edit(own,'enum { committed }','enum { committed, held }')
    own=edit(own,'  actor Ledger { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed } } }',
             '  observed case { allow: bool = true; registered: bool = false }\n'
             '  actor Ledger { executors [l1]; capacity 1; capability post() -> Ack { commits; writes [registered]; duration fixed 1min; '
             'atomic { if allow == true { registered = true; result committed } else { result held } } } }')
    pg='  authority PostGrant { basis "posting rule"; grants l1 to Ledger.post; issued_by cfo }\n'
    own=edit(own,'  process Flow',pg+'  process Flow')
    own=edit(own,'reversible_by [a1]','reversible_by [l1]')
    pair('P14','sov-atomic',own,'allow: bool = true','allow: bool = false',hand(),hand(runtime='vacuous',summary=0),
         'sov-self-candidate with post as ADR 0035 atomic (writes [registered]; if allow == true { registered = true; '
         'result committed } else { result held }). approve0..2; fail End returned2/work2. pass: post2..3 by l1, '
         'allow true selects the commit branch, so commit3; filing at3: declined closes3, filed recheck3..4 by c1 '
         'responds4 <= deadline8 (pass/fail). End posted3/work3, route excluded; summary0..1. Static: candidates '
         'Approver.approve (guard) and Ledger.post itself (atomic, ADR 0040 U2); l1 reaches via Ledger.post and '
         'PostGrant, satisfied. Root satisfied (exit0). Broken allow false: post2..3 still finishes normally with '
         'held (the reject branch, ADR 0035 U5), registered stays false and no commit occurs, so no effect on any '
         'path: alternatives [], contest_response vacuous, static still satisfied (structural), root vacuous '
         '(exit5), End posted3/work3 (post work1 kept: a normal finish, not a requires refusal), summary0..0. '
         'A finish of a commits Capability is not a commit unless it takes the atomic commit branch (F07).', 'held')


def no_contest(src):
    return edit(src, '  contract Controls { '+CLAUSE+' }\n', '')


def main():
    pair('P01','sov-basic',BASE,GRANT,'',hand(),hand(static='violated'),
         'approve fail: End2/work2. pass: post2..3, End3/work3. Guard candidate Approver only; a1 has Permission and ApprovalGrant. Route3..4, deadline8. Removing grant leaves Permission あり、Authority 未接続.', 'no-authority')
    concurrent=edit(BASE,'capability recheck()', 'capability notify() -> Verdict { duration fixed 2min; result nondet { pass } }; capability recheck()')
    concurrent=edit(concurrent,POST,'ack = Ledger.post(); n = Clerk.notify(); end posted')
    concurrent=edit(concurrent,'  bounds', '  metrics { window 20min; currency "JPY" }\n  bounds')
    pair('P02','sov-concurrent',concurrent,'respond_by 5min','respond_by 2min',
         hand((5,6),5,(4,6)),hand((5,6),5,(4,6),by=2,runtime='violated'),
         'At3 notify and recheck compete for c1. Recheck3..4 then notify4..6 gives End6; notify3..5 then recheck5..6 gives End5. Ordinary work2+1+2=5 (not6). Deadlines8/5; late response6 exists only in short pair.', 'short')
    vac=edit(BASE,'duration fixed 2min; result nondet { pass, fail }','duration fixed 2min; result nondet { fail }')
    vg=hand(None,runtime='vacuous',summary=0)
    vb=hand(None,runtime='inconclusive'); vb['terminals']=[]; vb['bounds']['truncated']=True
    vb['contest_runtime'][0]['alternatives']=[]; vb['contest_summary']['business_ended_open']=scalar(None,reason='truncated')
    pair('P03','sov-vacuous',vac,'clock 20min','clock 1min',vg,vb,
         'Only fail is effective, so post is never called: static guard still exists, response vacuous, End2/work2. clock1 stops before approve finishes: inconclusive, not vacuous. Complete warnings: post/recheck not_called, else arm, posted end, c1 unassigned; truncation must not assert unreachable.', 'truncated')
    warning=no_contest(BASE)
    deferred=edit(warning,'include [Approver, Ledger, Clerk, Vendor]', 'include [Approver, Ledger, Clerk, Vendor]; defer Vendor.contest reason "structure only" loses "dispute frequency"')
    plain=dict(verdict='satisfied', terminals=[terminal('returned',2,2),terminal('posted',3,3)])
    pair('P04','sov-warning',deferred,'; defer Vendor.contest reason "structure only" loses "dispute frequency"','',
         dict(plain,absent_diagnostics=['W-CONTEST-0']),plain,
         'Both End2/3 and work2/3. Only undeclared contest loses its warning suppression: exactly one W-CONTEST-0 Vendor/Ledger.post.commit has no contest route or explicit Cut.', 'silent')
    pair('P05','sov-deadline',BASE,'respond_by 5min','respond_by 0min',hand(),hand(by=0,static='violated',runtime='violated'),
         'Deadline must be positive structurally; zero is a violated property, not invalid. Target3 +0=3 < response4. Actual response and closure remain4.', 'zero')
    own=edit(BASE,'people [c1, a1, cfo]','people [c1, a1, cfo, l1]')
    own=edit(own,'enum { committed }','enum { committed, held }')
    own=edit(own,'actor Ledger { capacity 1','actor Ledger { executors [l1]; capacity 1')
    own=edit(own,'result nondet { committed }','result nondet { committed, held }')
    pg='  authority PostGrant { basis "posting rule"; grants l1 to Ledger.post; issued_by cfo }\n'
    own=edit(own,'  process Flow',pg+'  process Flow')
    own=edit(own,'reversible_by [a1]','reversible_by [l1]')
    pair('P06','sov-self-candidate',own,pg,'',hand(),hand(static='violated'),
         'Two effective post values make Ledger.post a self candidate, alongside Approver guard. l1 qualifies only via Ledger; removing PostGrant leaves permission but no Authority. Both values commit3, response4.', 'no-grant')
    pair('P06b','sov-effective-single',own,'result nondet { committed, held }','result nondet { committed }',hand(),hand(static='violated'),
         'Ack still has two type values; only one effective result removes Ledger candidate. l1 is not Approver executor: executor_not_connected. End3/work3 and response4 unchanged.', 'single-result')
    mixed=edit(BASE,'[Ledger.post.commit]','[end.returned, Ledger.post.commit]')
    mixed=edit(mixed,CLAUSE,CLAUSE+'; contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min')
    mg=hand(); end=copy.deepcopy(mg['contest_paths'][0]); end.update(id='contract Controls #2 contest',rules='contest-p1-1',end='returned'); del end['effect']; mg['contest_paths'].append(end)
    er=dict(case_id=1,static_id='contract Controls #2 contest',target='returned',alternatives=[alt(2)]+[alt(2,7,3,v) for v in ('pass','fail')]); mg['contest_runtime'].append(er)
    mb=copy.deepcopy(mg); mb['verdict']='violated'
    for p in mb['contest_paths']:
        p.update(verdict='violated',basis='構造の不足',missing=['route_exists'])
        p['checks']['route_exists']['satisfied']=False
        # Reason text is implementation's established Cut vocabulary; its
        # semantic check/field, not a new diagnostic code, is the oracle here.
        del p['checks']['route_exists']['reason']
    for row in mb['contest_runtime']:
        at=3 if row.get('target_kind') else 2
        row['alternatives']=[alt(at),alt(at,at+5,reason='missing_route')]
    pair('P07','sov-mixed',mixed,'include [Approver, Ledger, Clerk, Vendor]','include [Approver, Ledger, Vendor]; exclude Clerk reason "r" loses "l"',mg,mb,
         'Fail End2 triggers terminal route2..3. Pass commit3 triggers effect route3..4. The two rows are mutually exclusive per path, not jointly active. Excluding Clerk removes both routes; filed rows have missing_route and null response/closure.', 'route-excluded')
    pair('P08','sov-one-site',BASE,'if approval == fail { end returned } else { }; '+POST,
         'if approval == fail { x = Ledger.post(); end returned } else { ack = Ledger.post(); end posted }',hand(),
         refusal('U-SEM-0','effect contest with multiple commit sites is unsupported'),
         'The exclusive arms contain two lexical sites, even though no run commits twice. Reject before exploration.', 'two-sites')
    cases=edit(BASE,'capability approve()', 'capability review() -> Verdict { duration fixed 1min; result nondet { pass } }; capability approve()')
    cases=edit(cases,'route Clerk.recheck','route Approver.review')
    cases=edit(cases,'cases 1','cases 2')
    cases=edit(cases,'  bounds','  arrivals fixed [0min, 0min]\n  policy fifo tie_by case_id\n  bounds')
    cg=hand(); cg['terminals']=[]; cg['contest_summary']['business_ended_open']=scalar(0,2)
    cg['case_outcomes']=[dict(case_id=i,terminals=[dict(terminal('returned',t,2),kind='end',completed_at=r(t)),dict(terminal('posted',p,3),kind='end',completed_at=r(p))]) for i,t,p in [(1,2,3),(2,4,5)]]
    cb={}
    for by,expected in [(5,cg),(1,copy.deepcopy(cg))]:
        expected['contest_paths'][0].update(route='Approver.review',respond_by=by)
        expected['contest_runtime']=[]
        for i,at,response in [(1,3,5),(2,5,6)]:
            a=alt(at,at+by,response,'pass'); a['executor']='a1'
            expected['contest_runtime'].append(dict(case_id=i,target='Ledger.post.commit',target_kind='effect',route='Approver.review',alternatives=[alt(at),a]))
        if by==1: cb=expected; cb['verdict']='violated'
    pair('P09','sov-cases',cases,'respond_by 5min','respond_by 1min',cg,cb,
         'a1 approve(case1)0..2 then approve(case2)2..4. First route queued3 waits until4, responds5; second target5 responds6. End case1=2/3, case2=4/5. Short deadlines4/6: only first late, second exactly on time. Summary counts 0..2 distinct cases.', 'short')
    sim=edit(BASE,'duration fixed 2min; result nondet { pass, fail }','duration fixed 2min; result nondet { pass }')
    sim=edit(sim,'  bounds','  simulation replications 10 seed 1 confidence 0.95\n  bounds')
    pair('P10','sov-sim',sim,GRANT,'',dict(verdict='unsupported',diagnostics=[dict(code='U-SEM-0')],contest_paths=hand()['contest_paths']),dict(verdict='violated',diagnostics=[dict(code='U-SEM-0')],contest_paths=hand(static='violated')['contest_paths']),
         'Simulation reaches post commit3 and must refuse the filing choice U-SEM-0, never sample it. Missing ApprovalGrant leaves structural violated dominant (exit1) over unsupported.', 'no-authority',{'command':'simulate'})
    label=BASE
    for old,new in [('Vendor','Supplier'),('Ledger','Book')]:
        before=label
        label=label.replace(old,new)
        assert label != before
    label=edit(label,'actor Book {','actor Book { grain ledger;')
    label=edit(label,'actor Supplier {','actor Supplier { grain party;')
    lg=hand(target='Book.post.commit'); lg['contest_paths'][0]['party']='Supplier'
    pair('P11','sov-label',label,'grain ledger;','grain document;',lg,lg,
         'Label/metamorphic control, not a business violation: renamed Actors and ledger/document grain cannot change commit3/response4/End3/work3. Source byte spans are normalized using independently paired tokens, not ignored globally.', 'other-grain')
    metrics=edit(BASE,'  bounds','  metrics { window 20min; currency "JPY" }\n  bounds')
    pair('P12','sov-metrics',metrics,'duration fixed 1min; result nondet { pass, fail }','duration fixed 2min; result nondet { pass, fail }',hand(),hand(respond=(5,),duration=2),
         'Terminal work posted3 excludes route1 (broken route2 also leaves3). Full case/overall work2..4, changed2..5; c1 work0..1 or0..2. Missing rates remain not_configured, not-filed never_created.', 'longer-route')
    for key,target in [('start','Ledger.post.start'),('finish','Ledger.post.finish'),('finish-value','Ledger.post.finish(committed)'),('cancel','Ledger.post.cancel_ack'),('state','registered'),('actor','Ledger'),('unknown-actor','Nobody.post.commit'),('unknown-cap','Ledger.absent.commit')]:
        src=BASE if key!='state' else edit(BASE,'  people','  observed case { registered: bool = false }\n  people')
        code='E-NAME-0' if key.startswith('unknown') else 'U-SYN-0'
        # Both target occurrences change together as one logical target substitution.
        badsrc=src.replace('Ledger.post.commit',target); assert badsrc!=src
        name='sov-target-'+key; bad=name+'.broken-target'
        write(name,src,hand(),'P13: valid commit control. End3/work3, response4.')
        write(bad,badsrc,refusal(code),'P13: target substitution in Standing and contest only. '+target+' must fail closed as '+code+'. No runtime values.')
        PAIRS.append(dict(id='P13',good=name,bad=bad,old='Ledger.post.commit',new=target,all=True))
    for key,old,new,code,msg in [
        ('no-commits','commits; ','','E-CON-0','commit requires a commits Capability'),
        ('duplicate','[Ledger.post.commit]','[Ledger.post.commit, Ledger.post.commit]','U-SEM-0','duplicate Standing effect'),
        ('nonparty','control party; ','','E-CON-0','Standing requires control party'),
        ('mismatch','standing over [Ledger.post.commit]','standing over [end.returned]','E-CON-0','contest must match party Standing')]:
        pair('P13','sov-'+key,BASE,old,new,hand(),refusal(code,msg),'Legal commit control: target3/response4/End3/work3. Broken form is a static diagnostic, not a runtime violation.', 'invalid')
    live_cut=hand(5,5,runtime='inconclusive')
    live_cut['bounds']['truncated']=True
    unresolved=alt(3,8); unresolved['route_cost']=scalar(None,reason='truncated')
    live_cut['contest_runtime'][0]['alternatives']=[alt(3),unresolved]
    live_cut['contest_runtime'][0]['coverage']='truncated'
    live_cut['contest_summary']['business_ended_open']=scalar(None,reason='truncated')
    pair('P15','sov-live-tasks',concurrent,'live_tasks 2','live_tasks 1',hand((5,6),5,(4,6)),live_cut,
         'At3 filed route and notify are two queued live Tasks before dispatch. Bound1 cuts that branch; declined branch still reaches End5. A surviving complete branch cannot turn global cut into satisfied.', 'one')
    eq=edit(BASE,'respond_by 5min','respond_by 1min')
    pair('P16','sov-equality',eq,'duration fixed 1min; result nondet { pass, fail }','duration fixed 2min; result nondet { pass, fail }',hand(by=1),hand(by=1,respond=(5,),duration=2,runtime='violated'),
         'Target3+1=deadline4=response4 succeeds. Duration2 responds5>4 fails; End3/work3 unchanged.', 'late')
    sleep=edit(BASE,POST,'ack = Ledger.post(); wait 5min; end posted')
    pair('P17','sov-sleep',sleep,'duration fixed 1min; result nondet { pass, fail }','duration fixed 6min; result nondet { pass, fail }',hand(8,3,summary=0),hand(8,3,respond=(9,),duration=6,runtime='violated'),
         'Post commit3; wait wakes8, not a Task. Route runs3..4 during sleep, closes4, execution waits End8. Broken route3..9, End8 open, deadline8 <9. Work remains3.', 'slow')
    anysrc=edit(BASE,'  actor Vendor', '  actor Fast { capacity 1; capability go() -> Verdict { duration fixed 2min; result nondet { pass }; cancel cooperative ack_within 1min } }\n  actor Slow { capacity 1; capability go() -> Verdict { duration fixed 6min; result nondet { fail }; cancel cooperative ack_within 1min } }\n  actor Vendor')
    anysrc=edit(anysrc,'duration fixed 1min; result nondet { pass, fail }','duration fixed 5min; result nondet { pass, fail }')
    anysrc=edit(anysrc,POST,'ack = Ledger.post(); fork race { f = Fast.go(); s = Slow.go() } join any race where result == pass tie nondet remaining cancel_then_drain if none -> no_match; end posted; end no_match')
    anysrc=edit(anysrc,'include [Approver, Ledger, Clerk, Vendor]','include [Approver, Ledger, Clerk, Vendor, Fast, Slow]')
    anysrc=edit(anysrc,'live_tasks 2','live_tasks 3')
    pair('P18','sov-any',anysrc,'respond_by 5min','respond_by 4min',hand(6,8,respond=(8,),duration=5),hand(6,8,respond=(8,),duration=5,by=4,runtime='violated'),
         'Fast3..5 adopted5; Slow runs3..6 then ack6, End6. Ordinary work2+1+2+3=8, result_at5. Route3..8 is not canceled/drained by normal Any. Deadline8/7. Fail approval path End2/result_at null.', 'short')
    # Any's visible result belongs to the terminal snapshot.
    for n in ('sov-any','sov-any.broken-short'):
        p=FIXTURES/n/'expected.json'; v=json.loads(p.read_bytes()); v['terminals'][1]['result_at']=r(5); p.write_text(json.dumps(v,ensure_ascii=False,indent=2)+'\n')
    exact=edit(deferred,'defer Vendor.contest reason','defer Vendor.contest.Ledger.post.commit reason')
    pair('P19','sov-cut-exact',exact,'Vendor.contest.Ledger.post.commit reason','Vendor.contest.Ledger reason',dict(plain,absent_diagnostics=['W-CONTEST-0']),plain,
         'Exact effect label suppresses one warning; a mere Vendor.contest.Ledger prefix must not. End/work2/3 unchanged.', 'prefix')
    tail=edit(BASE,'  actor Vendor','  actor Tail { capacity 1; capability tail() -> Verdict { duration fixed 10min; result nondet { pass } } }\n  actor Vendor')
    tail=edit(tail,POST,'ack = Ledger.post(); t = Tail.tail(); end posted')
    tail=edit(tail,'Clerk, Vendor]','Clerk, Vendor, Tail]')
    pair('P20','sov-end-open',tail,'duration fixed 10min','duration fixed 0min',hand(13,13,summary=0),hand(),
         'Route3..4 closes before Tail3..13 → End13/work13, summary0. Zero tail End3/work3 sees route open before dispatch; later response4 must not clear saved open flag:0..1.', 'zero-tail')
    twice=edit(BASE,CLAUSE,CLAUSE+'; '+CLAUSE)
    tg=hand(respond=(4,5)); tg['contest_paths'].append(dict(tg['contest_paths'][0],id='contract Controls #2 contest')); tg['contest_runtime'].append(dict(tg['contest_runtime'][0],static_id='contract Controls #2 contest'))
    tb=copy.deepcopy(tg)
    for row in tb['contest_runtime']: row['alternatives']=[alt(3)]+[alt(3,8,t,v,2) for t in (5,7) for v in ('pass','fail')]
    pair('P20b','sov-two-contests',twice,'duration fixed 1min; result nondet { pass, fail }','duration fixed 2min; result nondet { pass, fail }',tg,tb,
         'Two independent filed bits give four choices. Both filed serialize c1, responses4/5 (broken5/7). Each marginal includes both positions. End3/work3; business_ended_open max1, not2.', 'longer')
    pair('P21','sov-route-target',BASE,'route Clerk.recheck','route Ledger.post',hand(),refusal('U-SEM-0','effect target used as contest route is unsupported'),
         'Target process one site still cannot also be a route: second occurrence could recur. Fail closed before exploration.', 'recursive')
    rev=edit(own,'basis "posting rule";','basis "posting rule"; revocable_by cfo;')
    pair('P22','sov-restricted',rev,'basis "posting rule";','basis "posting rule"; expires 100min;',hand(),refusal('U-SEM-0','contest cannot reference restricted Authority PostGrant'),
         'Revocation record alone is not a restriction. Expiry100 is restricted even though target3<100, so reject; no silent eligibility filtering.', 'expiry')
    kernel=edit(own,'result nondet { committed, held }','outcomes kernel post_result')
    kernel=edit(kernel,'  process Flow','  kernel post_result { * -> { 0.5: committed; 0.5: held } }\n  process Flow')
    pair('P23','sov-kernel',kernel,'0.5: committed; 0.5: held','1: committed; 0: held',hand(),hand(static='violated'),
         'Two positive mass results qualify Ledger self candidate; held mass0 removes it. Branch count/type size do not qualify. Both effective values commit3, response4.', 'zero-held')
    # P25: static projection differs by target activation, not runtime table diff.
    diff=edit(BASE,'  process Flow','  process Skipping { approval = Approver.approve(); if approval == fail { end returned } else { }; end posted }\n  process Flow')
    intent='change contest Ledger.post.commit reason "effect route"'
    diff=edit(diff,'  bounds','  configuration Skipping { cut Scope; process Skipping }\n  configuration Posting { cut Scope; process Flow; intent { '+intent+' } }\n  bounds')
    pair('P25','sov-diff',diff,intent,'',dict(verdict='violated'),dict(verdict='violated'),
         'Skipping never calls post: static no_decision violated, runtime vacuous. Posting static/runtime satisfied. Diff root violated from side; contest target Ledger.post.commit from[]→one. Removing intent adds W-DIFF-1, never changes runtime.', 'no-intent',{'command':'diff','from':'Skipping','to':'Posting'})
    shared=edit(concurrent,'  people [c1, a1, cfo]','  resource R capacity 1\n  people [c1, a1, cfo, n1]')
    notify='capability notify() -> Verdict { duration fixed 2min; result nondet { pass } }; '
    shared=edit(shared,notify,'')
    shared=edit(shared,'  actor Vendor','  actor Notify { executors [n1]; capability notify() -> Verdict { uses R:1; duration fixed 2min; result nondet { pass } } }\n  actor Vendor')
    shared=edit(shared,'capability recheck() -> Verdict {','capability recheck() -> Verdict { uses R:1;')
    shared=edit(shared,'n = Clerk.notify()','n = Notify.notify()')
    shared=edit(shared,'Clerk, Vendor]','Clerk, Vendor, Notify]')
    pair('P26','sov-resource',shared,'capability recheck() -> Verdict { uses R:1;','capability recheck() -> Verdict {',hand((5,6),5,(4,6)),hand(5,5),
         'Different people c1/n1 isolate resource R capacity1. With both uses: route first End6/response4 or notify first End5/response6. Route use removed: both start3, End5,response4. R occupied3 units-min vs2 on posted filed branches (whole range0..3/0..2).', 'unshared')
    clock=edit(BASE,'duration fixed 1min; result nondet { pass, fail }','duration fixed 3min; result nondet { pass, fail }')
    clock=edit(clock,'respond_by 5min','respond_by 2min'); clock=edit(clock,'clock 20min','clock 4min')
    def cut_hand(passed):
        h=hand(runtime='violated' if passed else 'inconclusive'); h['bounds']['truncated']=True
        a=alt(3,5,reason='deadline_passed_unresolved' if passed else None); a['route_cost']=scalar(None,reason='truncated'); a['executor']='c1'
        h['contest_runtime'][0]['alternatives']=[alt(3),a]; h['contest_runtime'][0]['coverage']='truncated'
        h['contest_paths'][0]['respond_by']=2; h['contest_summary']['business_ended_open']=scalar(None,reason='truncated')
        return h
    pair('P28','sov-clock',clock,'clock 4min','clock 5min',cut_hand(False),cut_hand(True),
         'Route3..6; deadline5. clock4 has not reached deadline: inconclusive/unresolved reason null. Inclusive clock5 reaches deadline with no response: violated/unresolved deadline_passed_unresolved, not late_response. Response/closure/work/wait null; cost truncated; declined never_created.', 'deadline')
    policy=edit(concurrent,'actor Clerk {','actor Clerk { control policy;')
    policy=edit(policy,'  bounds','  policy fifo tie_by case_id then call_order assign first_idle\n  bounds')
    pair('P29','sov-policy',policy,'control policy;','control environment;',hand(5,5,(6,)),hand((5,6),5,(4,6)),
         'Policy normal call_order notify precedes synthetic contest route: notify3..5, End5, recheck5..6 only. Environment choice restores both schedules End5..6/response4 or6. FIFO without call_order does not settle same case/tick.', 'environment')
    rates='rate Approver per_minute 1; rate Ledger per_minute 1; rate Clerk per_minute 1; rate Approver.approve per_start 0; rate Ledger.post per_start 0; rate Clerk.notify per_start 0; rate Clerk.recheck per_start 0; '
    paid=edit(concurrent,'metrics { window','metrics { '+rates+'window')
    pair('P30','sov-paid',paid,'rate Clerk.recheck per_start 0; ','',hand((5,6),5,(4,6)),hand((5,6),5,(4,6)),
         'JPY/min all1 and start fees0: ordinary posted cost5, returned2. Filed posted total6 vs declined5, whole overall2..6. Route fee missing gives total/route not_configured but business posted remains5, declined never_created.', 'missing-route-fee')
    both=edit(BASE,'[Ledger.post.commit]','[end.posted, Ledger.post.commit]')
    both=edit(both,CLAUSE,CLAUSE+'; contest Vendor on end.posted route Clerk.recheck reversible_by [a1] respond_by 5min')
    before=both
    both=both.replace('respond_by 5min','respond_by 2min')
    assert both!=before
    bg=hand(respond=(4,5),by=2); ep=dict(bg['contest_paths'][0],id='contract Controls #2 contest',rules='contest-p1-1',end='posted'); del ep['effect']; bg['contest_paths'].append(ep)
    bg['contest_runtime'].append(dict(case_id=1,static_id='contract Controls #2 contest',target='posted',alternatives=bg['contest_runtime'][0]['alternatives']))
    bb=copy.deepcopy(bg); bb['verdict']='violated'
    for row in bb['contest_runtime']: row['alternatives']=[alt(3)]+[alt(3,5,t,v,2) for t in (5,7) for v in ('pass','fail')]
    pair('P31','sov-effect-and-end',both,'duration fixed 1min; result nondet { pass, fail }','duration fixed 2min; result nondet { pass, fail }',bg,bb,
         'Effect and End both at3. Four independent filing choices; both filed responses4/5 <= deadline5, End3/work3, summary0..1. Broken duration2: responses5/7, each row has late alternative7. One-filed response5 remains on time.', 'late')
    route_start()
    atomic()
    names=sorted(ORACLES)
    support=Path(__file__).parent
    (support/'effect_standing_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (support/'effect_standing_pairs.json').write_text(json.dumps(PAIRS,ensure_ascii=False,indent=2)+'\n')
    print(f'{len(PAIRS)} single-edit pairs, {len(names)} directories')


if __name__ == '__main__':
    import argparse
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output-fixtures',type=Path,default=FIXTURES)
    FIXTURES=parser.parse_args().output_fixtures
    assert FIXTURES.is_dir()
    main()
