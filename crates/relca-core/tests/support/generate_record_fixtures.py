"""ADR0038 hand-authored acceptance materializer, not an implementation oracle.

All expected arithmetic below is fixed before any new parser is run. The script
does not import Relca, invoke a binary, or read an implementation result.
"""
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[4]
OUT = Path(__file__).parent
ROWS, PAIRS = {}, []
ENTRY = 'type Entry = record { confidence: enum { high, low } }'
TYPES = 'type Verdict = enum { pass, fail }\ntype Ack = enum { committed, refused }\n'+ENTRY
STATE = 'observed case { entry: optional<Entry>; approval: optional<Verdict>; registered: bool = false }'
EXTRACT = 'actor Extractor { capacity 1; capability extract() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high }, Entry { confidence = low } }; writes entry } }'
APPROVE = 'actor Approver { capacity 1; capability approve(e: Entry) -> Verdict { duration fixed 2min; result nondet { pass, fail }; writes approval } }'
REVIEW = 'actor Clerk { capacity 1; capability review(e: Entry) -> Verdict { duration fixed 3min; result nondet { pass, fail } } }'
POST = 'actor Ledger { capacity 1; capability post() -> Ack { commits; duration fixed 1min; result nondet { committed }; writes registered; finish { registered = true } } }'
SAFETY = 'verify safety no_resource_overcommit'


def change(s, a, b, count=1):
    assert s.count(a) == count, (a, s.count(a), count)
    t = s.replace(a, b)
    assert s != t
    return t


def model(items, flow, queries='', contract='', live=1, clock=20):
    return f'model Records version "0.2" {{\n{items}\nprocess Flow {{ {flow} }}\n{contract}\nbounds {{ cases 1; live_tasks {live}; clock {clock}min }}\n{SAFETY}\n{queries}\n}}\n'


def terminal(name, elapsed, work=None):
    if isinstance(elapsed, int): elapsed = [elapsed, elapsed]
    if work is None: work = elapsed
    if isinstance(work, int): work = [work, work]
    return {'end':name, 'elapsed':dict(zip(['min','max'],elapsed)), 'work':dict(zip(['min','max'],work)), 'result_at':None}


def expected(verdict='satisfied', terminals=(), codes=(), probability=None, truncated=False):
    e = {'verdict':verdict, 'diagnostics':[{'code':c, 'severity':'error' if c.startswith(('E-','U-')) else 'warning'} for c in [*codes,'W-CUT-0']]}
    if verdict in ('invalid','unsupported'):
        e.update(properties=[], terminals=[], bounds={'truncated':None})
    else:
        e['bounds'] = {'truncated':truncated}
        e['terminals'] = list(terminals)
    if probability:
        e['properties'] = [{'id':'probability reached #1','verdict':verdict,'probability':{'min':probability[0],'max':probability[1]}}, {'id':'safety no_resource_overcommit','verdict':'inconclusive' if truncated else 'satisfied'}]
        # Report order follows verify declaration order: safety precedes queries.
        e['properties'].reverse()
    return e


def add(name, source, e, calc, phase='runtime', invocation=None, projection=None, message=None):
    assert name not in ROWS
    assert '//' not in source and '#' not in source
    ROWS[name] = {'source':source, 'expected':e, 'calc':calc, 'phase':phase,
                  'projection':projection, 'message':message, 'invocation':invocation}


def pair(base, fault, s, t, e, f, calc, phase='runtime', negative_phase=None, invocation=None, projection=None, message=None):
    assert s != t
    base = 'rec-'+base
    broken = base+'.broken-'+fault
    add(base,s,e,calc,phase,invocation,projection)
    add(broken,t,f,calc,negative_phase or phase,invocation,projection,message)
    PAIRS.append({'base':base,'broken':broken})


def reject(code):
    return expected('invalid' if code.startswith('E-') else 'unsupported',codes=[code])


def reach(*names):
    return '\n'.join('verify reachable end.'+n for n in names)


def main():
    proxy_items = '\n'.join([TYPES, STATE, 'type Difficulty = enum { routine, difficult }',
        'latent case { extraction_correct: bool = false }',
        'actor Invoice { source document; latent difficulty: Difficulty ~ { routine: 0.8, difficult: 0.2 } }',
        'actor Extractor { capacity 1; capability extract() -> Entry { duration fixed 1min; outcomes kernel ai_entry; kernel_reads latent [Invoice.difficulty]; writes [entry, extraction_correct] } }', REVIEW, POST,
        'kernel ai_entry given [Invoice.difficulty] { routine -> { 1: Entry { confidence = high } with extraction_correct = true }; difficult -> { 0.5: Entry { confidence = low } with extraction_correct = false; 0.5: Entry { confidence = high } with extraction_correct = false } }'])
    proxy_flow = 'e = Extractor.extract(); if e.confidence == low { r = Clerk.review(e); end reviewed } else { p = Ledger.post(); end posted }'
    proxy = model(proxy_items,proxy_flow,'verify probability reached registered == true && extraction_correct == false <= 0.1')
    proxy_e = expected(terminals=[terminal('posted',2),terminal('reviewed',4)],probability=['1/10','1/10'])
    proxy_e['terminals'][0]['probability'] = {'min':'9/10','max':'9/10'}
    proxy_e['terminals'][1]['probability'] = {'min':'1/10','max':'1/10'}
    proxy_calc = 'T01/T06: routine mass 4/5 gives correct high. Difficult mass 1/5 splits equally: low wrong 1/10, high wrong 1/10. Wrong posting=1/10, posted=4/5+1/10=9/10, reviewed=1/10. Posted elapsed/work=1+1=2min; reviewed=1+3=4min. Nondeterministic review outcomes receive no invented probabilities.'
    pair('proxy','latent-route',proxy,change(proxy,'if e.confidence == low','if Invoice.difficulty == difficult'),proxy_e,reject('E-POL-LATENT'),proxy_calc,negative_phase='lower')
    pair('kernel','sum',proxy,change(proxy,'0.5: Entry { confidence = high }','0.4: Entry { confidence = high }'),proxy_e,reject('E-PROB-0'),proxy_calc+' Broken difficult row has mass 1/2+2/5=9/10, not 1.',negative_phase='lower')
    case = model('\n'.join([TYPES,STATE,EXTRACT,REVIEW]),'e = Extractor.extract(); if entry.confidence == low { r = Clerk.review(e); end reviewed } else { end auto }',reach('reviewed','auto'))
    pair('case-var','no-write',case,change(case,'; writes entry',''),expected(terminals=[terminal('auto',1),terminal('reviewed',4)]),expected('violated',[terminal('auto',1)],['W-UNREACH-0','W-UNREACH-1','W-UNREACH-2']), 'T02: high returns auto at1/work1; low performs review3, ending4/work4. Removing only writes leaves case entry=none, so its field comparison is false regardless of binding e. reviewed is unreachable; review never starts, then arm and reviewed end are unreachable.')
    approval_flow = 'e = Extractor.extract(); a = Approver.approve(e); if a == fail { end returned } else { }; if e.confidence == low { r = Clerk.review(e); if r == fail { end returned } else { } } else { }; p = Ledger.post(); end posted'
    bypass_flow = 'e = Extractor.extract(); if e.confidence == low { a = Approver.approve(e); if a == fail { end returned } else { }; r = Clerk.review(e); if r == fail { end returned } else { } } else { }; p = Ledger.post(); end posted'
    contract = 'contract C { invariant Ledger.post.commit -> approval == pass }'
    approval = model('\n'.join([TYPES,STATE,EXTRACT,APPROVE,REVIEW,POST]),approval_flow,contract=contract)
    bypass = change(approval,approval_flow,bypass_flow)
    approval_e = expected(terminals=[terminal('posted',[4,7]),terminal('returned',[3,6])],codes=['W-AGY-1'])
    bypass_e = expected('violated',[terminal('posted',[2,7]),terminal('returned',[3,6])],['W-AGY-1','W-AGY-1'])
    approval_calc = 'T03: approval failure ends at1+2=3min. Low review failure ends1+2+3=6min. High posting=1+2+1=4min; low posting=1+2+3+1=7min; serial work equals elapsed. Bypass high commits at1+1=2min with approval=none and violates the contract. Low times stay7/6. Clerk review is bypassed by high in both, Approver only in broken. W-AGY-1 counts1/2.'
    pair('approval','bypass',approval,bypass,approval_e,bypass_e,approval_calc)
    pair('explain','bypass',approval,bypass,approval_e,bypass_e,approval_calc+' T10: explain has no configuration selector. Shortest violating commit is high at2min. Canonical entry String is Entry { confidence = high }; approval none; registered true. Reference replay must verify.',invocation={'command':'explain'})
    label = change(change(change(approval,'Entry','Slip',count=7),'confidence','grade',count=4),'actor Extractor {','actor Extractor { grain model;')
    pair('label','unrenamed',label,approval,approval_e,approval_e,approval_calc+' T09 metamorphic control: renaming Entry/confidence to Slip/grade and adding grain model changes only declared names/grain, source hash and mapped source positions, not semantics.')
    fork_items = '\n'.join([TYPES,change(STATE,'registered: bool = false','registered: bool = false; po, receipt: optional<Verdict>'),EXTRACT,
        'actor Clerk { capacity 2; capability match_po(e: Entry) -> Verdict { duration fixed 3min; result nondet { pass }; writes po }; capability match_receipt(e: Entry) -> Verdict { duration fixed 5min; result nondet { pass }; writes receipt } }'])
    forks = 'p = Clerk.match_po(e); r = Clerk.match_receipt(e)'
    fork_flow = f'e = Extractor.extract(); fork checks {{ {forks} }}; join all checks; end done'
    fork = model(fork_items,fork_flow,reach('done'),live=2)
    pair('fork','reextract',fork,change(fork,forks,'x = Extractor.extract(); y = Extractor.extract()'),expected(terminals=[terminal('done',6,9)]),reject('U-SEM-0'),'T04: fork starts after extraction1. Elapsed=1+max(3,5)=6min; work=1+3+5=9min. Re-extraction siblings both write entry: static conflict regardless of Extractor capacity1.',negative_phase='lower',message='fork siblings write the same observed variable')
    lineage_items = fork_items+'\nactor Invoice { source document }\nactor PurchaseOrder { source record }\nactor ReceiptRecord { source record }\n'+POST+'\nactor Approver { capacity 1; capability approve(e: Entry, p: Verdict, r: Verdict) -> Verdict { duration fixed 2min; result nondet { pass, fail } } }'
    lineage_items = change(lineage_items,'capability extract() -> Entry {','capability extract() -> Entry { reads [Invoice];')
    lineage_items = change(lineage_items,'capability match_po(e: Entry) -> Verdict {','capability match_po(e: Entry) -> Verdict { reads [PurchaseOrder];')
    lineage_items = change(lineage_items,'capability match_receipt(e: Entry) -> Verdict {','capability match_receipt(e: Entry) -> Verdict { reads [ReceiptRecord];')
    lineage_items = change(lineage_items,'result nondet { pass }; writes','result nondet { pass, fail }; writes',count=2)
    lineage = model(lineage_items,'e = Extractor.extract(); fork checks { '+forks+' }; join all checks; if p == fail || r == fail { end returned } else { }; a = Approver.approve(e, p, r); if a == fail { end returned } else { }; ack = Ledger.post(); end posted',live=2)
    le = expected(terminals=[terminal('posted',9,12),terminal('returned',[6,8],[9,11])])
    pair('lineage','relay',lineage,change(lineage,'reads [ReceiptRecord];',''),le,le,'T05: join t6/work9; approve finishes8/work11; post9/work12. Returns6/work9 or8/work11. Match judgements subject Invoice form one group (2 judgements,1 actor); approve(e,p,r) forms another (1 judgement,1 actor). Independent sources PurchaseOrder,ReceiptRecord shrink to PurchaseOrder when receipt read alone is removed. Approve subjects lose ReceiptRecord. No reads of parameter names; no W-LIN-2 without kernels.')
    inline = model('type Verdict = enum { pass, fail }\nobserved case { outcome: optional<Verdict> }\nactor Desk { capacity 1; capability decide() -> enum { withdraw, dispute } { duration fixed 1min; result nondet { withdraw, dispute } } }','d = Desk.decide(); if d == dispute { end disputed } else { end withdrawn }',reach('disputed','withdrawn'))
    ie = expected(terminals=[terminal('disputed',1),terminal('withdrawn',1)])
    pair('inline-result','write-declared',inline,change(inline,'duration fixed 1min;','writes outcome; duration fixed 1min;'),ie,reject('E-TYPE-0'),'T07: two nondeterministic result values, both elapsed/work1min. Inline result is nominally different from Verdict, so writes outcome is rejected before execution.',negative_phase='lower')
    nominal = change(change(inline,'type Verdict = enum { pass, fail }','type Choice = enum { withdraw, dispute }'),'optional<Verdict>','optional<Choice>')
    nominal = change(nominal,'-> enum { withdraw, dispute }','-> Choice')
    nominal = change(nominal,'duration fixed 1min;','writes outcome; duration fixed 1min;')
    pair('nominal','same-members',nominal,change(nominal,'-> Choice','-> enum { withdraw, dispute }'),ie,reject('E-TYPE-0'),'T16: Choice and inline result have exactly the same ordered members, but are distinct nominal types. Changing just the return type description makes writes ill typed. Both branches otherwise take1min/work1.',negative_phase='lower')
    def size(n):
        fields = '; '.join(f'f{i}: bool' for i in range(n))
        values = ', '.join(f'f{i} = true' for i in range(n))
        return model(f'type R = record {{ {fields} }}\nactor Work {{ capacity 1; capability run() -> R {{ duration fixed 1min; result nondet {{ R {{ {values} }} }} }} }}','r = Work.run(); end done',reach('done'))
    pair('size','oversize',size(10),size(11),expected(terminals=[terminal('done',1)]),reject('U-SEM-0'),'T08: 10 independent bool fields give 2^10=1024 domain values, but only one normal result. Adding one domain dimension gives2048, a static unsupported size, not a state-budget cut or partial domain.',negative_phase='lower',message='record domain exceeds the supported size')
    req = model('\n'.join([TYPES,STATE,'latent case { extraction_correct: bool = false }',EXTRACT,change(POST,'commits;','commits; requires entry.confidence == high else refused;')]),'e = Extractor.extract(); a = Ledger.post(); if a == refused { end held } else { end posted }',reach('held','posted'))
    pair('requires','latent-requires',req,change(req,'requires entry.confidence == high','requires extraction_correct == true'),expected(terminals=[terminal('held',1),terminal('posted',2)]),reject('E-POL-LATENT'),'T11: high starts post (1+1=2min/work2). Low refusal does not start, finish or commit post: held at1min/work1, registered false. Latent condition is invalid, not a runtime refusal.',negative_phase='lower')
    starter = 'actor Starter { capacity 1; capability begin() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } } }'
    none_contract = 'contract C { invariant Approver.approve.finish -> entry.confidence == high || entry.confidence == low }'
    none = model('\n'.join([TYPES,STATE,EXTRACT,APPROVE,starter]),'e0 = Starter.begin(); e = Extractor.extract(); a = Approver.approve(e0); end done',contract=none_contract)
    pair('none','order',none,change(none,'e = Extractor.extract(); a = Approver.approve(e0);','a = Approver.approve(e0); e = Extractor.extract();'),expected(terminals=[terminal('done',4)]),expected('violated',[terminal('done',4)]),'T12: both sides begin1min, extract1min, approve2min. Correct approve finishes t4 with entry high/low. Swapping only extract/approve makes approve finish t3 with entry none, so both field equality leaves are false. Later extraction at4 does not erase past violation. Total done4/work4 on both.')
    none_base = model('\n'.join([TYPES,STATE,EXTRACT,APPROVE]),'e = Extractor.extract(); a = Approver.approve(e); end done',contract=none_contract)
    pair('none-base','missing-write',none_base,change(none_base,'; writes entry',''),expected(terminals=[terminal('done',3)]),expected('violated',[terminal('done',3)]),'T12-base: original ADR duration1+2=3min/work3; removing only record write leaves none at approve finish3.')
    agency = change(case,'if entry.confidence == low','if e.confidence == low')
    ae = expected(terminals=[terminal('auto',1),terminal('reviewed',4)])
    pair('agency-f','case-guard',agency,case,ae,ae,'T13: both guards route high to auto1 and low to reviewed4. Binding guard derives Extractor.extract selector for both ends. Current case-state guard is not_derived(state_guards), no framing actors, complete=false. No Agency rule is changed.')
    default = model(ENTRY+'\nobserved case { entry: Entry = Entry { confidence = high } }','if entry.confidence == high { end auto } else { end reviewed }',reach('auto'))
    pair('default','default-field',default,change(default,'confidence = high','confidence = low'),expected(terminals=[terminal('auto',0)],codes=['W-UNREACH-1','W-UNREACH-2']),expected('violated',[terminal('reviewed',0)],['W-UNREACH-1','W-UNREACH-2']),'T17: no tasks, so elapsed/work0min. High default enters auto; low default enters reviewed. The unchosen arm and end each produce one unreachable warning. Default is not silently none.')
    sf = model(TYPES+'\n'+STATE+'\nactor Work { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { committed }; start { entry = Entry { confidence = high } }; writes entry; finish { entry = Entry { confidence = low } } } }','r = Work.run(); end done',contract='contract C { invariant Work.run.start -> entry.confidence == high; invariant Work.run.finish -> entry.confidence == low }')
    sfe = expected(terminals=[terminal('done',1)])
    pair('start-finish','no-start',sf,change(sf,'start { entry = Entry { confidence = high } }','start {}'),sfe,expected('violated',[terminal('done',1)]),'T18: start atomically installs high at0, finish installs low at1; result is Ack but explicit constants have Entry type. Removing start assignment alone violates start invariant at0; finish still low, done1/work1.')
    pair('finish','wrong-finish',sf,change(sf,'finish { entry = Entry { confidence = low } }','finish { entry = Entry { confidence = high } }'),sfe,expected('violated',[terminal('done',1)]),'T18-finish: high at start0; correct low at finish1, wrong high violates only finish invariant at1. Both end1/work1.')
    refusal = model(ENTRY+'\nobserved case { entry: optional<Entry>; allowed: bool = false }\nactor Work { capacity 1; capability run() -> Entry { requires allowed == true else Entry { confidence = low }; duration fixed 2min; result nondet { Entry { confidence = high } }; writes entry } }','e = Work.run(); if e.confidence == low { end held } else { end done }',reach('held'))
    pair('record-refusal','no-guard',refusal,change(refusal,'requires allowed == true else Entry { confidence = low }; ',''),expected(terminals=[terminal('held',0)],codes=['W-UNREACH-0','W-UNREACH-1','W-UNREACH-2']),expected('violated',[terminal('done',2)],['W-UNREACH-1','W-UNREACH-2']),'T19: allowed false rejects at0/work0 with binding low and entry none, exactly one capability refusal, no start/finish/commit. Removing guard starts the normal high result, writes at2, done2/work2. Refusal literal must not be applied to writes.')
    pair_types = 'type Color = enum { red, green, blue }\ntype Pair = record { color: Color; flag: bool }'
    literals = ', '.join(f'Pair {{ flag = {flag}, color = {color} }}' for color in ['red','green','blue'] for flag in ['true','false'])
    product = model(pair_types+f'\nactor Work {{ capacity 1; capability run() -> Pair {{ duration fixed 1min; result nondet {{ {literals} }} }} }}','p = Work.run(); if p.flag == false { end off } else { end on }',reach('off','on'))
    product_one = change(product,literals,'Pair { flag = false, color = green }')
    pair('product','one-value',product,product_one,expected(terminals=[terminal('off',1),terminal('on',1)]),expected('violated',[terminal('off',1)],['W-UNREACH-1','W-UNREACH-2']),'T15: six values in declaration-order mixed radix:0 red,true;1 red,false;2 green,true;3 green,false;4 blue,true;5 blue,false. flag=false projects exactly{1,3,5}; green projects{2,3}. All tasks1/work1. Control restricts results to ID3, making on unreachable; an implementation deleting ID3 from projection would additionally make off unreachable. IR fault is tested separately, not fabricated as an input feature.')
    singleton = model('type One = record { flag: enum { yes } }\nobserved case { one: optional<One> }','if one.flag == yes { end bad } else { end safe }',reach('safe'))
    pair('singleton','present',singleton,change(singleton,'one: optional<One>','one: optional<One> = One { flag = yes }'),expected(terminals=[terminal('safe',0)],codes=['W-UNREACH-1','W-UNREACH-2']),expected('violated',[terminal('bad',0)],['W-UNREACH-1','W-UNREACH-2']),'M03: even when all record members match the field test, optional none must be false. Explicit present singleton is true. No tasks, time/work0.')
    prob = model(pair_types+'\nobserved case { pair: optional<Pair> }\nactor Work { capacity 1; capability run() -> Pair { duration fixed 1min; outcomes kernel K; writes pair } }\nkernel K { * -> { 0.25: Pair { flag = true, color = red }; 0.75: Pair { flag = false, color = green } } }','p = Work.run(); end done','verify probability reached pair.flag == false >= 0.75')
    pair('prob-field','bound',prob,change(prob,'>= 0.75','>= 0.76'),expected(terminals=[terminal('done',1)],probability=['3/4','3/4']),expected('violated',[terminal('done',1)],probability=['3/4','3/4']),'T21: record ID0 mass1/4 and ID3 mass3/4, false projection includes ID3, so exact reached probability3/4. Threshold3/4 holds;19/25 does not. Literal order flag,color differs from declaration color,flag. Both done1/work1.')
    alias = model(TYPES+'\n'+STATE+'\n'+change(EXTRACT,', Entry { confidence = low }','')+'\nactor Updater { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { committed }; writes entry; finish { entry = Entry { confidence = low } } } }','entry = Extractor.extract(); u = Updater.run(); if entry.confidence == low { end fresh } else { end stale }',reach('fresh'))
    pair('alias','stale-binding',alias,change(alias,'finish { entry = Entry { confidence = low } }','finish { entry = Entry { confidence = high } }'),expected(terminals=[terminal('fresh',2)],codes=['W-UNREACH-1','W-UNREACH-2']),expected('violated',[terminal('stale',2)],['W-UNREACH-1','W-UNREACH-2']),'T22: same-name binding entry is an allowed result writer alias. Current state is high at1 then low at2, so fresh2/work2. Replacing Updater constant only with high gives stale2/work2. Reading old binding instead of current state is wrong.')
    arrival = change(proxy,'bounds {','arrivals fixed [0min]\nbounds {')
    arrival_e = expected(probability=['1/10','1/10'])
    arrival_e['terminals'] = []
    arrival_e['case_outcomes'] = [{'case_id':1,'arrival':0,'terminals':[{'kind':'end','end':'reviewed','elapsed':{'min':4,'max':4},'work':{'min':4,'max':4},'probability':{'min':'1/10','max':'1/10'}}, {'kind':'end','end':'posted','elapsed':{'min':2,'max':2},'work':{'min':2,'max':2},'probability':{'min':'9/10','max':'9/10'}}]}]
    cut_e = expected('inconclusive',probability=['0','1'],truncated=True)
    pair('legacy-arrival','clock',arrival,change(arrival,'clock 20min','clock 0min'),arrival_e,cut_e,'T25: exactly one arrival at0; same proxy mass as T01: wrong posting1/10, end times2/4 and work2/4 relative to receipt. Clock0 precedes extraction finish1, so no completion and all mass unresolved; reached bound[0,1], inconclusive with computational truncation. No conversion of incomplete cases to failures. The clock cut keeps existing W-CUT-0 only, not W-BUDGET-0; arrivals use case_outcomes.terminals and properties.cases.')
    static_pairs()
    context_pairs()
    materialize()


def static_pairs():
    items = TYPES+'\n'+STATE+'\n'+EXTRACT
    base = model(items,'e = Extractor.extract(); if e.confidence == high { end high } else { end low }')
    be = expected(terminals=[terminal('high',1),terminal('low',1)])
    cases = [
        ('s01-latent-case','observed case { entry: optional<Entry>; approval: optional<Verdict>; registered: bool = false }','latent case { entry: Entry = Entry { confidence = high } }','U-SYN-0','lower','latent record is unsupported'),
        ('s02-latent-actor','actor Extractor {','actor Extractor { latent secret: Entry = Entry { confidence = high };','U-SYN-0','lower','latent record is unsupported'),
        ('s03-optional-field','confidence: enum { high, low }','confidence: optional<Verdict>','U-SYN-0','parse','record field type is unsupported'),
        ('s04-integer-field','confidence: enum { high, low }','confidence: 0..3','U-SYN-0','parse','record field type is unsupported'),
        ('s05-person-field','confidence: enum { high, low }','confidence: PersonId','U-SYN-0','parse','record field type is unsupported'),
        ('s06-caseid-field','confidence: enum { high, low }','confidence: CaseId','U-SYN-0','parse','record field type is unsupported'),
        ('s07-set-field','confidence: enum { high, low }','confidence: set<CaseId>','U-SYN-0','parse','record field type is unsupported'),
        ('s08-nested-field','confidence: enum { high, low }','confidence: Other','U-SYN-0','lower','record field type is unsupported'),
        ('s09-inline-record-field','confidence: enum { high, low }','confidence: record { x: Verdict }','U-SYN-0','parse','record field type is unsupported'),
        ('s11-whole-compare','if e.confidence == high','if e == Entry { confidence = high }','U-SYN-0','parse','record comparison is unsupported'),
        ('s16-inline-record-result','extract() -> Entry','extract() -> record { confidence: Verdict }','U-SYN-0','parse','inline record is unsupported'),
        ('s17-actor-state','actor Extractor {','actor Extractor { state s: Entry = Entry { confidence = high };','U-SEM-0','lower','Actor record state is unsupported'),
        ('s20-missing','result nondet { Entry { confidence = high }, Entry { confidence = low } }','result nondet { Entry {} }','E-TYPE-0','lower','record value missing field'),
        ('s21-duplicate','result nondet { Entry { confidence = high }, Entry { confidence = low } }','result nondet { Entry { confidence = high, confidence = low } }','E-TYPE-0','lower','record value duplicate field'),
        ('s22-unknown','result nondet { Entry { confidence = high }, Entry { confidence = low } }','result nondet { Entry { tone = high } }','E-TYPE-0','lower','record value unknown field'),
        ('s23-undefined-field','if e.confidence == high','if e.tone == high','E-NAME-0','lower','undefined record field'),
        ('s24-ambiguous','actor Extractor {','actor e {}\nactor Extractor {','E-CON-0','lower','ambiguous Actor and record'),
        ('s25-member','if e.confidence == high','if e.confidence == medium','E-TYPE-0','lower',None),
        ('s27-field-default','confidence: enum { high, low }','confidence: enum { high, low } = high','U-SYN-0','parse',None),
        ('s27-field-distribution','confidence: enum { high, low }','confidence: enum { high, low } ~ { high:1 }','U-SYN-0','parse',None),
        ('s32-empty-declaration','record { confidence: enum { high, low } }','record {}','E-SYN-0','parse','record requires a field'),
        ('s33-duplicate-declaration','confidence: enum { high, low }','confidence, confidence: enum { high, low }','U-SEM-0','lower','duplicate record field'),
        ('s34-nominal-literal','result nondet { Entry { confidence = high }, Entry { confidence = low } }','result nondet { Other { confidence = high } }','E-TYPE-0','lower','record value has different type'),
    ]
    for name,a,b,code,phase,message in cases:
        s = base
        if name in ('s08-nested-field','s34-nominal-literal'):
            s = change(s,SAFETY,'type Confidence = enum { high, low }\ntype Other = record { confidence: Confidence }\n'+SAFETY)
        if name == 's05-person-field':
            s = change(s,STATE,'people [p]\n'+STATE)
        pair(name,'unsupported' if code.startswith('U') else 'invalid',s,change(s,a,b),be,reject(code),'T14 '+name+': the negative changes only the named syntax/type shape. Positive extraction is1min/work1 and has both high/low ends. Static negative has no evaluated time, probability or terminals. Expected '+code+' at '+phase+'; Cut undeclared contributes exactly one W-CUT-0.',negative_phase=phase,message=message)
    # Non-Eq operators are distinct tests, including the optional whole-value case.
    for i,op in enumerate(['!=','<','<=','>','>=']):
        pair('s28-operator-'+str(i),'comparison',base,change(base,'e.confidence == high','e.confidence '+op+' high'),be,reject('U-SYN-0'),'S28: fields support equality only. Operator '+op+' is unsupported, not a scalar ordering extension. Positive outcomes1/work1.',negative_phase='lower',message='record field comparison is unsupported')
    opt = model(ENTRY+'\nobserved case { entry: optional<Entry> }','if entry == none { end empty } else { end present }',reach('empty'))
    pair('s12-none','non-eq',opt,change(opt,'entry == none','entry != none'),expected(terminals=[terminal('empty',0)],codes=['W-UNREACH-1','W-UNREACH-2']),reject('U-SYN-0'),'S12: optional none equality stays accepted; whole-record non-equality is unsupported. Correct no-task end empty0/work0.',negative_phase='lower',message='record comparison is unsupported')
    scalar = model('type T = enum { a, b }\nobserved case { c: T = a }\nactor Work { capacity 1; capability run() -> T { duration fixed 1min; result nondet { a } } }','x = Work.run(); end done',reach('done'))
    for name,a,b in [
        ('s13-case','c: T','c: enum { a,b }'),
        ('s15-state','actor Work {','actor Work { state s: enum {a,b} = a;'),
        ('s15-latent-case','observed case { c: T = a }','latent case { c: enum {a,b} = a }'),
        ('s15-latent-actor','actor Work {','actor Work { latent s: enum {a,b} = a;'),
        ('s13-optional','c: T = a','c: optional<enum {a,b}>'),
    ]:
        pair(name,'inline',scalar,change(scalar,a,b),expected(terminals=[terminal('done',1)]),reject('U-SYN-0'),'S13/S15: forbidden inline type position is unsupported/exit4, not old invalid/exit3. Scalar control completes at1/work1.',negative_phase='parse',message='inline enum position is unsupported')
    param = change(scalar,'capability run()','capability run(p: T)')
    param = change(param,'x = Work.run();','x = Work.seed(); y = Work.run(x);')
    param = change(param,'actor Work {','actor Work { capability seed() -> T { duration fixed 1min; result nondet { a } };')
    pair('s14-parameter','inline',param,change(param,'p: T','p: enum {a,b}'),expected(terminals=[terminal('done',2)]),reject('U-SYN-0'),'S14: record-independent classification control seeds an argument1 and calls run1: done2/work2. Only parameter type changes to forbidden inline enum.',negative_phase='parse',message='inline enum position is unsupported')
    default = model(ENTRY+'\nobserved case { entry: Entry = Entry { confidence = high } }','end done',reach('done'))
    pair('s31-default','missing-default',default,change(default,' = Entry { confidence = high }',''),expected(terminals=[terminal('done',0)]),reject('U-SEM-0'),'S31: nonoptional observed record requires explicit initial value, never a fabricated first member. No-task positive completes0/work0.',negative_phase='lower')
    dist = change(default,' = Entry { confidence = high }',' ~ { Entry { confidence = high }: 1 }')
    pair('s26-distribution','distribution',default,dist,expected(terminals=[terminal('done',0)]),reject('U-SYN-0'),'S26: record initial distribution is syntactically unsupported, including forward declarations. Fixed canonical default is accepted0/work0.',negative_phase='parse',message='record distribution is unsupported')
    effect = model(TYPES+'\n'+STATE+'\nactor Work { capacity 1; capability run() -> Ack { duration fixed 1min; result nondet { committed }; writes entry; finish { entry = Entry { confidence = low } } } }','r = Work.run(); end done',reach('done'))
    for name,target in [('s10-field-write','entry.confidence = low'),('s30-field-arithmetic','entry.confidence += low')]:
        pair(name,'field',effect,change(effect,'entry = Entry { confidence = low }',target),expected(terminals=[terminal('done',1)]),reject('U-SYN-0'),'S10/S30: whole record constant assignment is accepted; field-only write/arithmetic is unsupported. Control done1/work1.',negative_phase='lower',message='record field assignment is unsupported')
    given = model(TYPES+'\n'+STATE+'\nlatent case { h: bool = true }\nactor Work { capacity 1; capability run() -> Ack { duration fixed 1min; outcomes kernel K; kernel_reads latent [h] } }\nkernel K given [h] { true -> { 1: committed }; false -> { 1: committed } }','r = Work.run(); end done',reach('done'))
    for name,target in [('s18-given','entry'),('s19-given-field','entry.confidence')]:
        pair(name,'record',given,change(given,'given [h]','given ['+target+']'),expected(terminals=[terminal('done',1)]),reject('U-SEM-0'),'S18/S19: kernel given cannot read a case record or its field. Control scalar bool covers both rows, done1/work1. Reject the record feature, not an undefined Actor fallback.',negative_phase='lower',message='record in kernel given is unsupported')


def context_pairs():
    # State invariant and atomic/CaseId-specific crossings are deferred until ADR0035/0037 integration.
    initial = ENTRY+'\nobserved case { entry: Entry = Entry { confidence = high } }\ntype Ack = enum { done, refused }'
    producer = 'actor Seed { capacity 1; capability seed() -> Entry { duration fixed 1min; result nondet { Entry { confidence = high } } } }'
    cap = 'actor Work { capacity 1; capability run(e: Entry) -> Ack { duration fixed 1min; result nondet { done }; requires entry.confidence == high else refused } }'
    s = model(initial+'\n'+producer+'\n'+cap,'e = Seed.seed(); r = Work.run(e); end done',reach('done'))
    pair('context-requires','parameter',s,change(s,'requires entry.confidence','requires e.confidence'),expected(terminals=[terminal('done',2)]),reject('U-SEM-0'),'T23 requires: high fixed case entry admits run after seed1; total2min/work2. A record parameter is lineage material, never a condition input. Both inputs have the same well-typed call argument.',negative_phase='lower',message='record parameter condition is unsupported')
    # Other state-only contexts have a real available process binding e, not an unknown name.
    simple = 'actor Work { executors [p]; capability run() -> Ack { duration fixed 1min; result nondet { done } } }'
    auth_items = initial+'\npeople [p,r]\n'+producer+'\n'+simple+'\nauthority G { basis "record scope"; grants p to Work.run; issued_by r; scope entry.confidence == high }'
    scope = model(auth_items,'e = Seed.seed(); r = Work.run(); end done',contract='contract C { authority_basis Work.run G }')
    pair('context-scope','binding',scope,change(scope,'scope entry.confidence','scope e.confidence'),expected(terminals=[terminal('done',2)]),reject('U-SEM-0'),'T23 scope: fixed case high permits grant, seed1+run1=2/work2. Binding-only e in scope is unsupported even though it exists in the process.',negative_phase='lower',message='record binding is unsupported in this condition')
    prob = model(initial+'\n'+producer,'e = Seed.seed(); end done','verify probability reached entry.confidence == high >= 1')
    pair('context-probability','binding',prob,change(prob,'reached entry.confidence','reached e.confidence'),expected(terminals=[terminal('done',1)],probability=['1','1']),reject('U-SEM-0'),'T23 probability: fixed high holds at receipt with exact probability1. Seed1/work1. A process binding is not a state-only probability query input.',negative_phase='lower',message='record binding is unsupported in this condition')
    event = model(initial+'\n'+producer,'e = Seed.seed(); end done',contract='contract C { invariant Seed.seed.finish -> entry.confidence == high }')
    pair('context-event','binding-control',event,change(event,'-> entry.confidence','-> e.confidence'),expected(terminals=[terminal('done',1)]),expected(terminals=[terminal('done',1)]),'T23 event: available binding e at its finish is readable, just as current case entry. Both equal high, done1/work1. This negative control is intentionally accepted, preventing over-restrictive state-only reuse.')
    duty_items = initial+'\npeople [p]\nactor Record { source record }\n'+producer+'\nactor Work { executors [p]; capability run() -> Ack { commits; reads [Record]; duration fixed 1min; result nondet { done }; writes entry; finish { entry = Entry { confidence = high } } } }'
    for name,formula in [
        ('compare','entry.confidence == high'),
        ('and','entry.confidence == high && entry.confidence == high'),
        ('or','entry.confidence == low || entry.confidence == high'),
        ('implies','entry.confidence == low -> entry.confidence == high'),
    ]:
        duty = model(duty_items+'\nresponsible p for ('+formula+')','e = Seed.seed(); r = Work.run(); end done',reach('done'))
        target = 'responsible p for ('+formula
        broken = change(duty,target,'responsible p for ('+formula.replace('entry.confidence','e.confidence',1))
        pair('context-duty-'+name,'binding',duty,broken,expected(terminals=[terminal('done',2)]),reject('U-SEM-0'),'T23 responsibility '+name+': p executes the actual record writer, directly reads source Record, and has ordinary permission. Seed1+run1=2min/work2. Responsibility is analytical output, not an added verification property. Replacing only the leftmost case field by binding e must be state-only unsupported, not a missing person/source diagnostic.',negative_phase='lower',message='record binding is unsupported in this condition')


def legacy_projection(source):
    """Remove the new type layer only, to preflight old call/flow scaffolding.

    This is not a record oracle. The record IR/mixed-radix assertions remain
    independent fixed tables. No production parser/evaluator is imported here.
    """
    records = {}
    def declaration(m):
        name, _ = m.groups()
        if name in ('Entry','Slip','Other'): values = ['high','low']
        elif name == 'Pair': values = [c+'_'+b for c in ['red','green','blue'] for b in ['true','false']]
        elif name == 'One': values = ['yes']
        else: values = ['one']
        records[name] = values
        return 'type '+name+' = enum { '+', '.join(values)+' }'
    source = re.sub(r'type\s+(\w+)\s*=\s*record\s*\{\s*((?:[^{}]|enum\s*\{[^{}]*\})*)\s*\}',declaration,source)
    for name in records:
        def literal(m):
            fields = dict(re.findall(r'(\w+)\s*=\s*(\w+)',m[1]))
            if not fields: return m[0]
            if name == 'Pair': return fields['color']+'_'+fields['flag']
            if name == 'R': return 'one'
            return next(iter(fields.values()))
        source = re.sub(r'\b'+name+r'\s*\{([^{}]*)\}',literal,source)
    source = re.sub(r'(\w+)\.(?:confidence|grade|flag)\s*==\s*(high|low|yes)',r'\1 == \2',source)
    def pair_field(m):
        binding,field,value = m.groups()
        values = [c+'_'+b for c in ['red','green','blue'] for b in ['true','false'] if (b if field=='flag' else c)==value]
        return '('+' || '.join(binding+' == '+v for v in values)+')'
    source = re.sub(r'(\w+)\.(flag|color)\s*==\s*(true|false|red|green|blue)',pair_field,source)
    inline_types = re.findall(r'-> enum \{([^{}]*)\}',source)
    inline_index = 0
    def inline(m):
        nonlocal inline_index
        inline_index += 1
        return '-> Inline'+str(inline_index)
    source = re.sub(r'-> enum \{[^{}]*\}',inline,source)
    return source.replace('version "0.2" {','version "0.2" {\n'+'\n'.join('type Inline'+str(i+1)+' = enum {'+body+'}' for i,body in enumerate(inline_types)))


def materialize(write=True):
    import sys
    metadata_only = '--metadata-only' in sys.argv
    names = sorted(ROWS)
    if write:
        (OUT/'record_names.json').write_text(json.dumps(names,indent=2)+'\n')
        (OUT/'record_fixture_names.rs').write_text('&[\n'+''.join('    '+json.dumps(name)+',\n' for name in names)+']\n')
        (OUT/'record_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    oracle = {}
    for name,row in ROWS.items():
        e = row['expected']
        if row['phase'] == 'runtime' and 'properties' not in e:
            properties = []
            invariant_count = row['source'].count('invariant ')
            for i in range(invariant_count):
                verdict = 'satisfied'
                if e['verdict'] == 'violated':
                    if name == 'rec-start-finish.broken-no-start': verdict = 'violated' if i==0 else 'satisfied'
                    elif name == 'rec-finish.broken-wrong-finish': verdict = 'violated' if i==1 else 'satisfied'
                    else: verdict = 'violated'
                properties.append({'id':f'contract C #{i+1} invariant','verdict':verdict})
            if 'authority_basis Work.run G' in row['source']:
                properties.append({'id':'contract C #1 authority_basis','verdict':'satisfied'})
            properties.append({'id':'safety no_resource_overcommit','verdict':'satisfied'})
            ends = {t['end'] for t in e.get('terminals',[])}
            for end in re.findall(r'verify reachable end\.(\w+)',row['source']):
                properties.append({'id':'reachable end.'+end,'verdict':'satisfied' if end in ends else 'violated'})
            e['properties'] = properties
        if name.startswith('rec-legacy-arrival'):
            p = e['properties'][1]
            probability = p.pop('probability')
            p['cases'] = [{'case_id':1,'arrival':0,'verdict':p['verdict'],'probability':probability}]
        if not write:
            continue
        oracle[name] = {k:v for k,v in row.items() if k not in ('source','expected','calc','projection','invocation')}
        if metadata_only:
            continue
        directory = ROOT/'fixtures'/name
        directory.mkdir(exist_ok=True)
        (directory/'model.relca').write_text(row['source'])
        (directory/'expected.json').write_text(json.dumps(row['expected'],indent=2)+'\n')
        (directory/'calc.md').write_text('# '+name+'\n\n'+row['calc']+'\n\nExpected values are hand derivations, not captured implementation output. Units: minutes. New acceptance stops RED before implementation.\n')
        if row['invocation']:
            (directory/'invocation.json').write_text(json.dumps(row['invocation'])+'\n')
    if not write:
        return
    (OUT/'record_oracle.json').write_text(json.dumps(oracle,indent=2)+'\n')
    skeletons = {name:legacy_projection(row['source']) for name,row in ROWS.items() if row['phase']=='runtime'}
    (OUT/'record_legacy_skeletons.json').write_text(json.dumps(skeletons,indent=2)+'\n')
    cases = '\n'.join('#[test]\nfn '+re.sub(r'[^a-zA-Z0-9_]','_',name)+'() { hand::acceptance("'+name+'"); }' for name in names)
    (OUT/'record_fixture_cases.rs').write_text('// Generated by generate_record_fixtures.py; hand oracles remain in calc.md.\n'+cases+'\n')
    print(len(names),'directories;',len(PAIRS),'pairs;',sum(r['phase']=='runtime' for r in ROWS.values()),'runtime inputs')


if __name__ == '__main__':
    main()
