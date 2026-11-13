"""ADR0046 hand materializer. Never invokes Relca or harvests implementation output.

Each edit is checked for a witness. The oracle describes only the new two rows;
the old four rows are protected separately by current goldens and semantic tests.
"""
import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
ROWS = {}
DESTINATION = ROOT / ('.tmp/prepared-fixtures' if '--prepare-only' in sys.argv else 'fixtures')


def edit(s, a, b):
    t = s.replace(a, b)
    assert t != s, a
    return t


X = '''model Expiry version "0.2" {
  type Verdict = enum { pass, fail }
  people [a1,cfo]
  actor Approver { executors [a1]; capability approve() -> Verdict { duration fixed 1min; result nondet { pass } } }
  authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }
  process Flow { v = Approver.approve(); end done }
  cut Scope { include [Approver] }
  bounds { cases 1; live_tasks 1; clock 10min }
  verify safety no_resource_overcommit
  contract Controls { authority_basis Approver.approve ApprovalGrant }
}
'''
S = '''model Summary version "0.2" {
  type Entry = enum { entry }
  type Verdict = enum { pass, fail }
  type Ack = enum { posted }
  actor Invoice { source document }
  actor PurchaseOrder { source record }
  actor Extract { via model "doc@v3"; capacity 1; capability make() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
  actor Clerk { capacity 1; capability match_po(e:Entry) -> Verdict { reads [PurchaseOrder]; duration fixed 1min; result nondet { pass, fail } } }
  actor Approver { capacity 1; capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail } } }
  actor Ledger { capacity 1; capability post(p:Verdict,a:Verdict) -> Ack { commits; duration fixed 1min; result nondet { posted } } }
  process Flow {
    e = Extract.make(); p = Clerk.match_po(e); a = Approver.approve(e)
    if p == pass && a == pass {} else { end rejected }
    z = Ledger.post(p,a); end done
  }
  cut Scope { include [Invoice,PurchaseOrder,Extract,Clerk,Approver,Ledger] }
  bounds { cases 1; live_tasks 2; clock 10min }
  verify reachable end.done
  verify safety no_resource_overcommit
}
'''


def auth(name='ApprovalGrant', bindings=None, result='detected', reason=None, keys=None):
    changes = []
    if result=='detected':
        keys=['contract Controls #1 authority_basis'] if keys is None else keys
        changes=[{'check':'properties','key':key,'field':'verdict','before':'satisfied','after':'violated','direction':'worsened'} for key in keys]
    return {'id': f'authority_expired:{name}:expires', 'operator': 'expire_authority',
            'site': {'authority': name, 'process': None, 'actor': None,
                     'capability': None, 'source': None, 'statement_path': None,
                     'retained_branch': None, 'affected_bindings': ['v'] if bindings is None else bindings},
            'result': result, 'reason': reason, 'assigned_changes':changes}


def summary(consumer='a', actor='Extract', binding='e', path='body/0', fresh=None,
            result='detected', reason=None, subject=None, independent=None, count=2,
            capability='Approver.approve'):
    subject = ['Invoice'] if subject is None else subject
    independent = [] if independent is None else independent
    m = {'id': f'summary_reused:Flow:{path}:{binding}->{consumer}',
         'operator': 'duplicate_summary',
         'site': {'process': 'Flow', 'actor': actor, 'capability': 'make',
                  'source': None, 'statement_path': path, 'retained_branch': None,
                  'affected_bindings': [binding, consumer],
                  'duplicate_binding': fresh or binding + '_dup'},
         'result': result, 'reason': reason}
    if result == 'detected':
        m['assigned_changes'] = [
            {'check': 'agency.lineage', 'key': consumer + ':' + capability,
             'field': f, 'before': v, 'after': v, 'direction': 'held'}
            for f, v in [('subject_sources', subject), ('independent_sources', independent)]] + [
            {'check': 'agency.lineage_groups', 'key': json.dumps(subject, separators=(',', ':')),
             'field': 'judgements', 'before': count, 'after': count, 'direction': 'held'}]
    elif result == 'undetected':
        m['assigned_changes'] = []
    return m


def row(kind, ms, blocked=False):
    return {'kind': kind, 'status': 'blocked' if blocked else 'generated' if ms else 'not_applicable',
            'reason': 'original_not_decidable' if blocked else None if ms else 'no_matching_site',
            'mutants': ms}


def put(name, source, calc, authorities=(), summaries=(), original='satisfied',
         completion='complete', reason=None, terminals=None, old_sites=None):
    no_expiry_time = name in ('mut2-expiry.broken-zero-duration', 'mut2-expiry-zero-finish')
    if no_expiry_time:
        # Hand cases: one zero-minute operation, arrival/finish at absolute 0.
        # This is the declared applicability decision, not a call to the producer.
        authorities = ()
        calc += ' ADR0068: no elapsed time; authority_expired is not_applicable/no_time_elapse, candidates 0.'
    no_selected = name in (
        'mut2-expiry.broken-zero-duration', 'mut2-expiry-shared.broken-no-basis',
        'mut2-expiry-replace.broken-zero-min', 'mut2-expiry-replace.broken-zero-h',
        'mut2-expiry-replace.broken-zero-d', 'mut2-expiry-waive.broken-waive-only',
        'mut2-expiry-inherited.broken-explicit-zero', 'mut2-expiry-zero-finish',
        'mut2-expiry-zero-finish.broken-explicit-zero',
    )
    blocked = original not in ('satisfied', 'violated')
    expected = {'verdict': original if blocked else 'unsupported',
                'original_verdict': original,
                'run': {'completion': 'blocked' if blocked else completion,
                        'reason': 'original_not_decidable' if blocked else reason,
                         'mutant_limit': 128, 'limit_reached': reason == 'mutant_limit'}}
    if no_expiry_time:
        expected['run']['candidates_total'] = 0
    if no_selected:
        assert not authorities and not summaries
        calc += ' Round2: all supported candidate counts are 0, but unsupported retains precedence (exit4); no success or no_selected_mutant is claimed.'
    diagnostic_codes={
        'mut2-gate':['E-NAME-0'],
        'mut2-gate.broken-unsupported':['U-SYN-0','W-CUT-0'],
        'mut2-configuration.broken-selector':['E-CONF-0'],
        'mut2-summary-judgement':[],
        'mut2-summary-judgement.broken-no-guard':[],
    }
    if name in diagnostic_codes:
        expected['original']={'diagnostics':[{'code':code} for code in diagnostic_codes[name]]}
    rows = [row('authority_expired', list(authorities), blocked),
            row('summary_reused', list(summaries), blocked)]
    if no_expiry_time:
        rows[0]['reason'] = 'no_time_elapse'
    record = {'expected': expected, 'rows': rows, 'terminals': terminals or {},
              'old_sites': old_sites, 'calculation': calc}
    manual = []
    for m in list(authorities)+list(summaries):
        if m['result']=='not_run':
            continue
        if m['operator']=='expire_authority':
            marker='authority '+m['site']['authority']+' {'
            begin=source.index(marker)
            end=source.index('}',begin)
            declaration=source[begin:end]
            if re.search(r'expires\s+\d+(?:min|h|d)',declaration):
                replacement=re.sub(r'expires\s+\d+(?:min|h|d)','expires 0min',declaration)
            else:
                replacement=declaration+'; expires 0min '
            changed=source[:begin]+replacement+source[end:]
        else:
            b,c=m['site']['affected_bindings']; fresh=m['site']['duplicate_binding']
            pattern=r'\b'+re.escape(c)+r'\s*=\s*\w+\.\w+\(([^)]*)\)'
            match=re.search(pattern,source)
            assert match, (name,c)
            args=match.group(1)
            new_args=re.sub(r'\b'+re.escape(b)+r'\b',fresh,args)
            assert args!=new_args
            changed=source[:match.start(1)]+new_args+source[match.end(1):]
            call=b+' = '+m['site']['actor']+'.make()'
            changed=edit(changed,call,call+'; '+fresh+' = '+m['site']['actor']+'.make()')
        assert source!=changed
        manual.append({'id':m['id'],'source':changed})
    record['hand_edits']=manual
    assert name not in ROWS
    ROWS[name] = record
    directory = DESTINATION / name
    directory.mkdir(exist_ok=True, parents=True)
    (directory / 'model.relca').write_text('\n'.join(line.rstrip() for line in source.splitlines())+'\n')
    (directory / 'expected.json').write_text(json.dumps(expected, ensure_ascii=False, indent=2)+'\n')
    (directory / 'invocation.json').write_text('{"command":"mutate"}\n')
    (directory / 'calc.md').write_text(f'# {name} — ADR 0046\n\n{calc}\n\n'
        'Expected values were derived before running the implementation. Time/work unit: minute.\n'
        'Mutation is not a coverage or real-world quality score. Rule-version mutation and the undeclared quality denominator remain unsupported here.\n'
        'The exact new-row oracle is mutation_default_oracle.json in core tests/support.\n'
        'The hand values are acceptance oracles, not implementation snapshots.\n')


def pair(name, source, broken, fault, calc, a=(), s=(), ba=(), bs=(), **kw):
    put('mut2-'+name, source, calc, a, s, **kw)
    put('mut2-'+name+'.broken-'+fault, broken, calc, ba, bs, **kw)


def main():
    pair('expiry', X, edit(X, 'fixed 1min', 'fixed 0min'), 'zero-duration',
         'T01: finish1 > expiry0 gives satisfied→violated. Control finish0 ≤ expiry0 stays satisfied.',
         a=[auth()], ba=[auth(result='undetected', reason='no_worsening')])
    shared = edit(edit(edit(X, ' } }\n  authority',
        ' } capability review() -> Verdict { duration fixed 1min; result nondet { pass } } }\n  authority'),
        'issued_by cfo', 'grants a1 to Approver.review; issued_by cfo'),
        'end done', 'w = Approver.review(); end done')
    shared = edit(shared, 'authority_basis Approver.approve ApprovalGrant',
        'authority_basis Approver.approve ApprovalGrant; authority_basis Approver.review ApprovalGrant')
    pair('expiry-shared', shared, edit(shared,
         'authority_basis Approver.approve ApprovalGrant; authority_basis Approver.review ApprovalGrant', ''),
         'no-basis', 'T02: one Authority, two clauses, finish times1 and2 >0. One candidate, affected [v,w].',
         a=[auth(bindings=['v','w'],keys=['contract Controls #1 authority_basis','contract Controls #2 authority_basis'])])
    replace = edit(X, 'issued_by cfo', 'issued_by cfo; expires 30min')
    put('mut2-expiry-replace', replace, 'T03: finish1 ≤30; replace (never append a second expires) with0, 1>0.', [auth()])
    for unit in ['min','h','d']:
        put('mut2-expiry-replace.broken-zero-'+unit, edit(replace, '30min', '0'+unit),
            'T03/T23: explicit amount0 is an identity edit in every unit; no candidate. Original finish1>0 is violated.', original='violated')
    delegated = (ROOT/'fixtures/auth-delegation-name/model.relca').read_text()
    delegated = edit(delegated, '  bounds', '  cut Scope { include [A] }\n  bounds')
    put('mut2-expiry-delegated', delegated, 'T04: Child5→0; finish2>0. Parent10 untouched.', [auth('Child',['a'])])
    put('mut2-expiry-delegated.broken-parent-named', edit(delegated, 'authority_basis A.run Child', 'authority_basis A.run Parent'),
        'T04: Parent10→0 but Child5 remains; 5>0 yields E-CON-0, static_invalid. No exploration.',
        [auth('Parent',['a'],'invalid_mutant','static_invalid')], completion='inconclusive', reason='mutant_incomplete')
    contest = edit((ROOT/'fixtures/resp-contest/model.relca').read_text(),
        'respond_by 5min', 'respond_by 5min; authority_basis Approver.approve ApprovalGrant')
    put('mut2-expiry-contest', contest, 'T05: reader1+approve1=2; restricted contest Authority is U-SEM-0 before exploration.',
        [auth(bindings=['approval'], result='invalid_mutant', reason='static_unsupported')], completion='inconclusive', reason='mutant_incomplete')
    no_contest = edit(contest, 'contest Vendor on end.returned route Clerk.recheck reversible_by [a1] respond_by 5min;', '')
    no_contest = edit(no_contest, 'include [Invoice,Reader,Clerk,Approver,Vendor]',
        'include [Invoice,Reader,Clerk,Approver,Vendor]; defer Vendor.contest reason "structure only" loses "dispute frequency"')
    put('mut2-expiry-contest.broken-no-contest', no_contest, 'T05 control: finish2>0 is violated, detected; unused Clerk warning is not detection.', [auth(bindings=['approval'])])
    separate = edit(S, '  actor Clerk', '  actor Reader { capacity 1; capability make() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }\n  actor Clerk')
    separate = edit(edit(edit(separate, 'p = Clerk.match_po(e)', 'r = Reader.make(); p = Clerk.match_po(e)'), 'a = Approver.approve(e)', 'a = Approver.approve(r)'), '[Invoice,PurchaseOrder,Extract,Clerk', '[Invoice,PurchaseOrder,Extract,Reader,Clerk')
    put('mut2-summary', S, 'T06: e1,p2,a3,done4/work4, rejected3/work3. Duplicate adds1: done5/5,rejected4/4. a subject{Invoice}, independent{}, group judgements2.', summaries=[summary()], terminals={'done':[4,5,4,5], 'rejected':[3,4,3,4]}, old_sites=6)
    put('mut2-summary.broken-separate-producers', separate, 'T06 control: two distinct producers each consumed once; candidate0, shared source still Invoice. done5,rejected4.')
    fork = edit(S, '  actor Approver { capacity 1; capability approve(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail } } }\n', '')
    fork = edit(fork, '  actor Clerk', '  actor ReceiptRecord { source record }\n  actor Clerk')
    fork = edit(fork, 'actor Clerk { capacity 1;', 'actor Clerk { capacity 2;')
    fork = edit(fork, 'duration fixed 1min; result nondet { pass, fail } } }', 'duration fixed 3min; result nondet { pass, fail } } capability match_receipt(e:Entry) -> Verdict { reads [ReceiptRecord]; duration fixed 5min; result nondet { pass, fail } } }')
    fork = edit(fork, 'p = Clerk.match_po(e); a = Approver.approve(e)', 'fork checks { p = Clerk.match_po(e); r = Clerk.match_receipt(e) } join all checks')
    fork = edit(edit(edit(edit(fork, 'a == pass', 'r == pass'), 'Ledger.post(p,a)', 'Ledger.post(p,r)'), 'Extract,Clerk,Approver,Ledger', 'Extract,Clerk,Ledger,ReceiptRecord'), 'clock 10min', 'clock 8min')
    put('mut2-summary-fork', fork, 'T07: 1+max(3,5)+1=7 elapsed; work10. Duplicate:8/work11. rejected6/work9→7/work10.', summaries=[summary('r', capability='Clerk.match_receipt', independent=['ReceiptRecord'])], terminals={'done':[7,8,10,11], 'rejected':[6,7,9,10]})
    put('mut2-summary-fork.broken-clock', edit(fork,'clock 8min','clock 7min'), 'T07: original done7 ≤clock7, mutant done8>clock7; truncated is inconclusive, not detection.', summaries=[summary('r',result='inconclusive',reason='truncated')], completion='inconclusive',reason='mutant_incomplete')
    note = 'capability note(e:Entry) -> Verdict { duration fixed 1min; result nondet { pass, fail } } '
    judgement = edit(S, 'capability match_po', note+'capability match_po')
    judgement = edit(edit(judgement, 'a = Approver.approve(e)', 'n = Clerk.note(e)'), 'a == pass','n == pass')
    judgement = edit(judgement, '  actor Ledger', '  actor Sink { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { posted } } }\n  actor Ledger')
    judgement = edit(edit(judgement,'Ledger.post(p,a)','Sink.post()'),'Approver,Ledger]','Approver,Ledger,Sink]')
    for unused in ['Approver','Ledger']:
        declaration=next(line for line in S.splitlines() if line.startswith('  actor '+unused+' '))+'\n'
        judgement=edit(judgement,declaration,'')
    judgement=edit(judgement,'Extract,Clerk,Approver,Ledger,Sink]','Extract,Clerk,Sink]')
    pair('summary-judgement', judgement, edit(judgement,'if p == pass && n == pass {} else { end rejected }',''), 'no-guard',
         'T08: no-argument Sink is an effect. Only guard connects p/n to J; removing it leaves lineage empty. done4→5, rejected3→4.',
         s=[summary('n',capability='Clerk.note')], bs=[summary('n',result='undetected',reason='no_assigned_check')])
    put('mut2-gate', edit(S,'e = Extract.make()','e = Extrac.make()'), 'T09: E-NAME-0 blocks all six generated kinds; no mutation trials.', original='invalid')
    put('mut2-gate.broken-unsupported', edit(S,'reads [Invoice];','reads [Invoice]; retry 1;'), 'T09: retry semantics absent, U-SYN-0 blocks six kinds. Parsing fails before an AST is available, so report::check_with retains empty_report Cut=undeclared and finish adds W-CUT-0 (report.rs:1178–1185,1511–1526). The warning is not mutation detection.', original='unsupported')
    for n, name in [(63,'mut2-limit'),(64,'mut2-limit.broken-boundary'),(65,'mut2-limit.broken-many-sites')]:
        names = [f'S{i:02}' for i in range(n)]
        model = 'model Budget version "0.2" {\n type T = enum { ok }\n people [p,cfo]\n'
        model += ''.join(f' actor {x} {{ source document }}\n' for x in names)
        model += ' actor Probe { executors [p]; capability read() -> T { reads ['+','.join(names)+']; duration fixed 1min; result nondet { ok } } }\n'
        model += ' authority G { basis "b"; grants p to Probe.read; issued_by cfo }\n process Flow { end done; end missing }\n'
        model += ' cut Scope { include ['+','.join(names+['Probe'])+'] }\n bounds { cases 1; live_tasks 1; clock 10min }\n verify reachable end.missing\n contract C { authority_basis Probe.read G }\n}\n'
        put(name, model, f'T10: {n} reads ×2={2*n} old sites, then1 Authority. Independent missing-end query is violated, Authority vacuous. Elapsed/work0. Trials min(128,{2*n+1}).',
            [auth('G',[], 'undetected' if n==63 else 'not_run', 'no_worsening' if n==63 else 'mutant_limit')],
            original='violated', completion='complete' if n==63 else 'inconclusive', reason=None if n==63 else 'mutant_limit', old_sites=2*n)
    label = S
    for a,b in [('Extract','Desk'),('Clerk','Robot'),('Approver','Ledger2'),('Ledger {','Book {'),('Ledger.post','Book.post'),(',Ledger]',',Book]')]:
        label = edit(label,a,b)
    label = edit(edit(edit(label,'via model "doc@v3"; ',''),'actor Ledger2 {','actor Ledger2 { grain service; via model "doc@v3";'),'e = Desk.make(); p = Robot.match_po(e); a = Ledger2.approve(e)', 'doc = Desk.make(); p = Robot.match_po(doc); a = Ledger2.approve(doc)')
    pair('label', S, label, 'renamed', 'T11: names/grain/via placement do not change candidate count1 or held sets; elapsed4→5, independent source count1.',s=[summary()],bs=[summary(actor='Desk',binding='doc',capability='Ledger2.approve')])
    pair('summary-fresh', S, edit(S,'z = Ledger.post(p,a); end done','z = Ledger.post(p,a); e_dup = Extract.make(); end done'), 'occupied',
         'T12: unused e_dup consumes no arguments, so one summary candidate remains. Fresh changes e_dup→e_dup2. done4→5 vs5→6, rejected3→4.', s=[summary()],bs=[summary(fresh='e_dup2')])
    pair('summary-commits', S, edit(S, 'capability make() -> Entry {','capability make() -> Entry { commits;'), 'commits',
         'T13a: normal result has two consumers. Commits producer is excluded, not duplicated.',s=[summary()])
    state = edit(S,'actor Extract {','actor Extract { state flag: bool = false;')
    for phase in ['start','finish']:
        side_effect = 'start { flag = true }' if phase=='start' else 'writes flag; finish { flag = true }'
        pair('summary-state-'+phase, state, edit(state,'reads [Invoice];','reads [Invoice]; '+side_effect), 'writer',
             f'T13b: Actor state {phase} side effect excludes producer. Read-only state declaration does not. Time4→5.',s=[summary()])
    atomic_base = edit(edit(edit(S,'enum { entry }','enum { entry, duplicate }'),'  actor Invoice','  observed case { flag: bool = false }\n  actor Invoice'),
        'reads [Invoice];','reads [Invoice]; writes flag; finish { flag = true }')
    atomic = edit(atomic_base,'finish { flag = true }','atomic { if flag == true { result duplicate } else { flag = true; result entry } }')
    atomic = edit(atomic,'result nondet { entry }','')
    pair('summary-atomic',atomic_base,atomic,'atomic',
         'T13c: case flag is not read by a process guard, ordinary writes may duplicate (done4→5). Atomic alone excludes producer. No commits or Actor state used.',s=[summary()])
    pair('summary-fork-producer', S, edit(S,'e = Extract.make();','fork src { e = Extract.make() } join all src;'), 'branch',
         'T13e: fork branch producer is not a block statement insertion site; no candidate. Time remains4.',s=[summary()])
    waive = edit((ROOT/'fixtures/auth-waive/model.relca').read_text(), '  bounds','  cut Scope { include [Officer,B,Wait] }\n  bounds')
    waive = edit(waive,'contract Controls {','contract Controls { authority_basis Officer.release G;')
    put('mut2-expiry-waive',waive,'T13f: release1,wait4=5; delivery waived at1≤deadline2. Added expires is restricted waive U-SEM-0.',[auth('G',['o'],'invalid_mutant','static_unsupported')],completion='inconclusive',reason='mutant_incomplete')
    put('mut2-expiry-waive.broken-waive-only',edit(waive,'authority_basis Officer.release G;',''),'T13f: authorized_by alone is not a named basis site. No expiry candidate, done5/work5.')
    put('mut2-configuration',X,'T13g: unconfigured source has one expiry candidate, finish1>0.',[auth()])
    put('mut2-configuration.broken-selector',edit(X,'  bounds','  configuration C { cut Scope; process Flow }\n  bounds'),'T13g: configuration without selector E-CONF-0, six blocked rows.',original='invalid')
    cases = edit(edit(X,'cases 1; live_tasks 1; clock 10min','cases 2; live_tasks 1; clock 20min'), '  bounds','  arrivals fixed [0min,5min]\n  bounds')
    cases = edit(cases,'issued_by cfo','issued_by cfo; expires 3min')
    put('mut2-expiry-cases',cases,'T15: absolute finishes1,6. Original cases satisfied/violated (whole violated); expires0 makes both violated, only case1 worsens. Each lead/work1.',[auth(keys=['contract Controls #1 authority_basis:case:1'])],original='violated')
    put('mut2-expiry-cases.broken-longer',edit(cases,'expires 3min','expires 6min'),'T15: inclusive boundary6 makes both original cases satisfied; expiry0 worsens whole and both cases.',[auth(keys=['contract Controls #1 authority_basis','contract Controls #1 authority_basis:case:1','contract Controls #1 authority_basis:case:2'])])
    three = edit(edit(edit(S,'capability match_po',note+'capability match_po'),'a = Approver.approve(e)','a = Approver.approve(e); n = Clerk.note(e)'), 'a == pass','a == pass && n == pass')
    pair('three-consumers',three, edit(edit(three,'note(e:Entry)','note()'),'Clerk.note(e)','Clerk.note()'), 'no-third-argument',
         'T16: e consumers p,a,n => two independent mutants, group3 (done5→6/rejected4→5). No-argument n removes third consumer and group returns2.',
         s=[summary(count=3),summary('n',count=3,capability='Clerk.note')],bs=[summary()])
    guard = edit(edit(edit(S,'enum { entry }','enum { high, low }'),'result nondet { entry }','result nondet { high, low }'),'z = Ledger.post','if e == high {} else { end rejected }\n    z = Ledger.post')
    pair('guard-unchanged',guard,edit(guard,'if e == high','if a == pass'),'different-guard',
         'T17: duplicate only changes consumer args. Guard reading e is never rewritten. Aggregate reports alone cannot witness this; private AST obligation is explicit.',s=[summary()],bs=[summary()])
    writer = edit(edit(S,'  actor Invoice','  observed case { flag: bool = false }\n  actor Invoice'),'reads [Invoice];','reads [Invoice]; writes flag; finish { flag = true }')
    pair('guarded-write',writer,edit(writer,'e = Extract.make();','e = Extract.make(); if flag == true {} else {}'), 'observed-guard',
         'T18: case write without later flag guard may duplicate; with later observed flag guard exclude, even if branches empty. Time4.',s=[summary()])
    other = edit(S,'  actor Invoice','  actor Extra { source record }\n  actor Other { capacity 1; capability make() -> Entry { reads [Extra]; duration fixed 1min; result nondet { entry } } }\n  actor Invoice')
    other = edit(edit(edit(edit(other,'approve(e:Entry)','approve(e:Entry,r:Entry)'), 'a = Approver.approve(e)','a = Approver.approve(e,r)'), 'e = Extract.make();','r = Other.make(); e = Extract.make();'), '[Invoice,PurchaseOrder,Extract','[Extra,Other,Invoice,PurchaseOrder,Extract')
    pair('no-producer-source',other,edit(other,'reads [Invoice];',''), 'no-source',
         'T19: producer Invoice source belongs to a subject{Extra,Invoice}, count1, detected. Removing only producer read leaves subject{Extra} but producer source empty: undetected/no_assigned_check (candidate retained). done5→6/rejected4→5.',
         s=[summary(path='body/1',subject=['Extra','Invoice'],count=1)],bs=[summary(path='body/1',result='undetected',reason='no_assigned_check')])
    # T21 names in disjoint syntactic classes; each mutation starts from original.
    for cls, declaration, cut in [
        ('case','observed case { e_dup: bool = false }',''),
        ('latent','latent case { e_dup: bool = false }',''),
        ('actor','actor e_dup { source record }',',e_dup'),
        ('type','type e_dup = enum { marker }',''),
        ('people','people [e_dup]',''),
        ('authority','people [owner]; authority e_dup { basis "record"; grants owner to Approver.approve }','')]:
        occupied = edit(S,'  type Entry', '  '+declaration+'\n  type Entry')
        if cut:
            occupied = edit(occupied,'Approver,Ledger]','Approver,Ledger'+cut+']')
        pair('fresh-'+cls,S,occupied,'occupied',f'T21: {cls} name e_dup is occupied; first fresh e_dup2. Candidate1, time4→5.',s=[summary()],bs=[summary(fresh='e_dup2')])
    for arm in ['then','else']:
        body = 'e = Extract.make(); p = Clerk.match_po(e); a = Approver.approve(e)\n    if p == pass && a == pass {} else { end rejected }\n    z = Ledger.post(p,a); end done'
        nested = edit(S,'  actor Invoice','  actor Switch { capacity 1; capability choose() -> Verdict { duration fixed 1min; result nondet { '+('pass' if arm=='then' else 'fail')+' } } }\n  actor Invoice')
        nested = edit(nested,body, 'choose = Switch.choose(); if choose == pass { '+(body if arm=='then' else 'end other')+' } else { '+(body if arm=='else' else 'end other')+' }')
        nested = edit(nested,'Approver,Ledger]','Approver,Ledger,Switch]')
        pair('nested-'+arm,nested,edit(nested,'e = Extract.make();','fork src { e = Extract.make() } join all src;'),'fork-producer',
             'T22: pure result outer guard, nested block producer path body/1/'+arm+'/0; fork producer excluded. Switch1 plus S4 gives elapsed5→6.',s=[summary(path='body/1/'+arm+'/0')])
    pair('fresh-fork',S,edit(S,'e = Extract.make();','fork e_dup { unused = Extract.make() } join all e_dup; e = Extract.make();'),'occupied',
         'T21: fork name e_dup occupies a global name class. Unused branch result has no consumer. e at body/1 duplicates with e_dup2, done5→6.',
         s=[summary()],bs=[summary(path='body/1',fresh='e_dup2')])
    inherited = edit(X,'authority ApprovalGrant { basis "approval table"; grants a1 to Approver.approve; issued_by cfo }',
         'authority Parent { basis "parent"; grants a1 to Approver.approve; issued_by cfo; expires 0min }\n  authority ApprovalGrant { basis "child"; grants a1 to Approver.approve; issued_by a1; delegated_from Parent }')
    inherited = edit(edit(inherited,'end done','end done; end missing'),'verify safety no_resource_overcommit','verify reachable end.missing')
    put('mut2-expiry-inherited',inherited,'T23: inherited effective0 is not an explicit AST expires. Append expires0 once. Original independent missing-end query violated opens gate; Authority stays violated, no_worsening.',[auth(result='undetected',reason='no_worsening')],original='violated')
    put('mut2-expiry-inherited.broken-explicit-zero',edit(inherited,'delegated_from Parent','delegated_from Parent; expires 0min'),
        'T23: explicit amount0 is already an identity; no candidate. Missing-end query remains violated.',original='violated')
    repeated=edit(edit(S,'approve(e:Entry)','approve(e:Entry,other:Entry)'),'a = Approver.approve(e)','a = Approver.approve(e,e)')
    pair('repeated-argument',repeated,S,'single-argument',
         'T22/F19: a call consuming e twice is one consumer. Both args must change to e_dup in one mutant. Time4→5, group2; the single-argument control has the same lineage.',s=[summary()],bs=[summary()])
    occupied=edit(S,'  type Entry','  observed case { e_dup: bool = false }\n  type e_dup2 = enum { marker }\n  type Entry')
    pair('fresh-two',occupied,edit(occupied,'  type e_dup2 = enum { marker }\n',''),'one-class',
         'T21: case e_dup and type e_dup2 reserve two names, so fresh e_dup3. Removing only the type leaves fresh e_dup2. Task count and time unchanged.',s=[summary(fresh='e_dup3')],bs=[summary(fresh='e_dup2')])
    zero_finish=edit(replace,'fixed 1min','fixed 0min')
    pair('expiry-zero-finish',zero_finish,edit(zero_finish,'expires 30min','expires 0min'),'explicit-zero',
         'T23: explicit30→0 is a real AST edit but finish0 remains within expiry0, no_worsening. Already explicit0 is no site. Both original models satisfied at0.',a=[auth(result='undetected',reason='no_worsening')])
    budget_sources=[f'BudgetSource{i:02}' for i in range(60)]
    budget_prefix='  people [a1,cfo]\n'+''.join('  actor '+n+' { source record }\n' for n in budget_sources)
    budget_prefix+='  actor Probe { capacity 1; capability read() -> Entry { reads ['+','.join(budget_sources)+']; duration fixed 1min; result nondet { entry } } }\n'
    budget_summary=edit(three,'  actor Invoice',budget_prefix+'  actor Invoice')
    budget_summary=edit(budget_summary,'actor Approver { capacity 1;','actor Approver { capacity 1; executors [a1];')
    budget_summary=edit(budget_summary,'  process Flow','  authority G { basis "budget"; grants a1 to Approver.approve; issued_by cfo }\n  contract C { authority_basis Approver.approve G }\n  process Flow')
    budget_summary=edit(budget_summary,'Approver,Ledger]','Approver,Ledger,Probe,'+','.join(budget_sources)+']')
    put('mut2-limit-summary',budget_summary,
        'T10 extension: 60 unused reads×2 + S old6 =126 old trials. Authority runs127. Summary e→a runs128, e→n not_run129. Authority expiry is checked despite whole-run generation/comparison remaining unchecked.',
        [auth('G',['a'],keys=['contract C #1 authority_basis'])],[summary(count=3),summary('n',count=3,capability='Clerk.note',result='not_run',reason='mutant_limit')],
        completion='inconclusive',reason='mutant_limit',old_sites=126)
    put('mut2-limit-summary.broken-no-third-argument',edit(edit(budget_summary,'note(e:Entry)','note()'),'Clerk.note(e)','Clerk.note()'),
        'T10 extension control: same126 old trials + Authority1 + Summary1 =128 exactly, complete. Authority checked on both sides; only whole-run completion differs.',
        [auth('G',['a'],keys=['contract C #1 authority_basis'])],[summary()],old_sites=126)
    names = sorted(ROWS)
    (HERE/'mutation_default_names.json').write_text(json.dumps(names,indent=2)+'\n')
    (HERE/'mutation_default_oracle.json').write_text(json.dumps(ROWS,ensure_ascii=False,indent=2)+'\n')
    print(len(names),'hand inputs written')


if __name__ == '__main__':
    main()
