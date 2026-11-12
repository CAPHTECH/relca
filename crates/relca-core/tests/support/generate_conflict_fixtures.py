"""ADR0041 hand oracles. No binary invoked; write only cf-* and own tables."""
import copy
import json
import re
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
PAIRS, ROWS = [], {}


def change(s, old, new):
    t = s.replace(old, new)
    assert t != s, old
    return t


CL = '''model Conflict version "0.2" {
  type R = enum { committed, conflict }
  type H = enum { free, taken }
  actor Ledger {
    capacity 2
    state holder: H = free
    state claims: 0..1 = 0
    capability claim() -> R {
      version_check [holder] else conflict
      commits
      duration fixed 1min
      result nondet { committed }
      writes [holder, claims]
      finish { holder = taken; claims += 1 }
    }
  }
  process P {
    r = Ledger.claim()
    if r == committed { end claimed } else { end retry_needed }
  }
  cut C { include [Ledger] }
  bounds { cases 2; live_tasks 1; clock 10min }
  arrivals fixed [0min,0min]
  policy fifo tie_by case_id
  verify reachable end.claimed
  verify reachable end.retry_needed
}
'''
CHECK = '      version_check [holder] else conflict\n'


def rng(n):
    return {'min': n, 'max': n}


def cases(spec):
    return [dict(case_id=i+1, arrival=a, terminals=[dict(kind='end',end=e,completed_at=rng(t),elapsed=rng(t-a),work=rng(w),result_at=None) for e,t,w in ts]) for i,(a,ts) in enumerate(spec)]


def expected(verdict='satisfied', case_rows=None, properties=None, codes=(), truncated=False):
    v: dict[str, Any] = dict(verdict=verdict, diagnostics=[{'code':c} for c in codes])
    v['absent_diagnostics'] = [c for c in ['E-SYN-0','E-TYPE-0','E-NAME-0','E-POL-LATENT','U-SYN-0','U-SEM-0'] if c not in codes]
    if verdict not in ('invalid','unsupported'):
        v['bounds'] = {'truncated':truncated}
        if case_rows is not None:
            v['terminals'],v['case_outcomes'] = [],case_rows
        if properties is not None:
            v['properties'] = properties
    return v


def prop(id, verdict='satisfied'):
    return dict(id=id, verdict=verdict)


DUAL = cases([(0,[('claimed',1,1),('retry_needed',1,1)])]*2)
SERIAL = cases([(0,[('claimed',1,1)]),(0,[('claimed',2,1)])])
YES = expected(case_rows=DUAL, properties=[prop('reachable end.claimed'),prop('reachable end.retry_needed'),prop('state_in_range')])
NO = expected('unsupported',codes=['U-SEM-0'])
SER = expected('violated',case_rows=SERIAL,properties=[prop('reachable end.claimed'),prop('reachable end.retry_needed','violated'),prop('state_in_range')])


def pair(base, source, oracle, old, new, bad_oracle, calc, fault='no-check', assumptions=None):
    bad = base+'.broken-'+fault
    assert base not in ROWS
    for name,s,e in [(base,source,oracle),(bad,change(source,old,new),bad_oracle)]:
        assert '//' not in s and '#' not in s
        ROWS[name] = dict(name=name,source=s,expected=copy.deepcopy(e),calc=calc,mode='check',assumptions=assumptions if name==base else None)
    PAIRS.append(dict(good=base,bad=bad,old=old,new=new))


def main():
    pair('cf-version',CL,YES,CHECK,'',NO,
         'A01/A15. Minutes: both start0 and finish1. First writes holder=taken and claims1; all other running Tasks become stale. Second finishes conflict with work1 but NO write/Commit. Reverse order gives each case both ends. claims never attempts2. Without V the holder self-pair is unresolved G5.',
         assumptions={'Ledger.holder.concurrency':'version_check','Ledger.claims.concurrency':'commutative'})
    three=change(change(change(CL,'capacity 2','capacity 3'),'cases 2','cases 3'),'[0min,0min]','[0min,0min,0min]')
    three_e=copy.deepcopy(YES);three_e['case_outcomes']=cases([(0,[('claimed',1,1),('retry_needed',1,1)])]*3)
    pair('cf-three',three,three_e,CHECK,'',NO,'A02. Three starts0, first finish1 notifies BOTH others. Exactly one commit/two conflicts, claims1, total work3. Each can win. F04 notification-to-one attempts claims2 and violates range.')
    serial=change(change(CL,'capacity 2','capacity 1'),'claims: 0..1','claims: 0..2')
    no_retry=change(serial,'  verify reachable end.retry_needed\n','')
    pair('cf-serial',serial,SER,CHECK,'',SER,'A03. FIFO: case1 runs0..1, case2 starts AFTER first write and runs1..2. Both commit, claims2. retry_needed unreachable for BOTH, root violated1. Arrival-time stale incorrectly rejects case2; root alone cannot detect it.',assumptions={'Ledger.holder.concurrency':'version_check'})
    tick=change(change(CL,'[0min,0min]','[0min,1min]'),'claims: 0..1','claims: 0..2')
    tick_e=copy.deepcopy(YES);tick_e.update(verdict='violated',exit_code=1);tick_e['properties'][1]['verdict']='violated';tick_e['case_outcomes']=cases([(0,[('claimed',1,1)]),(1,[('claimed',2,1),('retry_needed',2,1)])])
    pair('cf-same-tick',tick,tick_e,'  policy fifo','  simulation replications 10 seed 1 confidence 0.95\n  policy fifo',NO,'A04. At1 start2-before-finish1 makes case2 stale; reverse does not. case1 claimed1 only, case2 both ends2 elapsed/work1. Root violated because case1 cannot retry. Simulation refuses residual choice, not uniform sampling.',fault='simulate')
    ROWS['cf-same-tick.broken-simulate']['mode']='simulate'
    lww=change(change(no_retry,CHECK,''),'state holder: H = free','state holder: H = free last_write_wins')
    le=expected(case_rows=SERIAL,properties=[prop('reachable end.claimed'),prop('state_in_range'),prop('no_lost_update Ledger.holder')])
    lb=copy.deepcopy(le);lb.update(verdict='violated',exit_code=1);lb['properties'][-1]['verdict']='violated';lb['case_outcomes']=cases([(0,[('claimed',1,1)])]*2)
    pair('cf-lww',lww,le,'capacity 1','capacity 2',lb,'A05. Serial finishes1,2, claims2, no stale overwrite. Capacity2 starts both0: second finish1 overwrites SAME taken value while stale, hence loss despite no value difference.',fault='overlap',assumptions={'Ledger.holder.concurrency':'last_write_wins'})
    clock=copy.deepcopy(le);clock.update(verdict='inconclusive',exit_code=2,bounds={'truncated':True});clock['properties']=[prop('reachable end.claimed','inconclusive'),prop('state_in_range','inconclusive'),prop('no_lost_update Ledger.holder','inconclusive')];clock['case_outcomes'][1]['terminals']=[]
    pair('cf-lww-clock',lww,le,'clock 10min','clock 1min',clock,'A06. case1 finish1; case2 finish2 beyond clock1 has NO terminal. No observed loss is not proof: inconclusive2, truncated. Do not impute work.',fault='clock')
    lock=change(change(change(no_retry,CHECK,''),'  actor Ledger','  resource ledger_lock capacity 1\n  actor Ledger'),'      commits','      uses ledger_lock: 1\n      commits')
    lock=change(lock,'    capacity 1','    capacity 2')
    ke=expected(case_rows=SERIAL,properties=[prop('reachable end.claimed'),prop('state_in_range')])
    pair('cf-lock',lock,ke,'ledger_lock capacity 1','ledger_lock capacity 2',NO,'A07. Amounts1+1>capacity1 excludes overlap; held until finish. FIFO times1,2 work1 each claims2. Capacity2 gives2>2 FALSE: holder self-pair unresolved.',fault='wide-lock',assumptions={'Ledger.holder.concurrency':'lock ledger_lock','Ledger.claims.concurrency':'lock ledger_lock'})
    race=(ROOT/'fixtures/aobs-capacity/model.relca').read_text()
    re_=expected(case_rows=cases([(0,[('posted',1,1),('denied',1,0)])]*2),properties=[prop('safety no_resource_overcommit'),prop('state_in_range')])
    rb=expected('violated');rb['bounds']['range_violation_stops']=True
    rb['case_outcomes']=cases([(0,[('posted',1,1)])]*2);rb['terminals']=[]
    rb['properties']=[prop('safety no_resource_overcommit','inconclusive'),prop('state_in_range','violated')]
    rb['state_range_violations']=[dict(case_id=i,time=1,variable='Ledger.posted',before='1',operand='1',attempted='2',lower='0',upper='1',phase='finish') for i in (1,2)]
    pair('cf-race',race,re_,'capacity 1','capacity 2',rb,'A08. NO FIFO, either case can win. capacity1 posted1/work1 or denied1/work0. capacity2 both requires see0 at0; first finish0->1, second attempts2>1 at1 and stops WITHOUT second completion. Range violated, resource safety inconclusive.',fault='two-writers')
    atomic=(ROOT/'fixtures/aset-cases.broken-capacity/model.relca').read_text()
    ae=expected(case_rows=cases([(0,[('done',1,1)])]*2),properties=[prop('contract C #1 at_most_once'),prop('reachable end.done')])
    modified=change(change(change(atomic,'  type T','  type S=enum{idle,busy}\n  type T'),'state keys:','state status:S=idle; state keys:'),'writes [keys, registered]','writes [keys, registered, status]')
    # Declare status on both sides but write it only on the fault side.
    modified=change(modified,'writes [keys, registered, status]','writes [keys, registered]')
    bad=change(change(modified,'writes [keys, registered]','writes [keys, registered, status]'),'keys += key; registered = true;','keys += key; registered = true; status = busy;')
    pair('cf-atomic-cap2',modified,ae,modified,bad,NO,'A09. InsertCase commutes; atomic reads current set at finish. Distinct callers yield keys={1,2}, registered=true, commits at1, at_most_once satisfied with NO mechanism_unmodeled. One semantic fault adds noncommutative status write (writes-list is syntax support).',fault='overwrite')
    latent=change(CL,'    state holder:','    latent risk: H = free\n    state holder:')
    pair('cf-latent',latent,YES,'[holder]','[holder, risk]',expected('invalid',codes=['E-POL-LATENT','U-SEM-0']),'A10. Known latent risk is not written: BOTH E-POL-LATENT and U-SEM-0 target is not written. holder V stays valid. No runtime.',fault='latent')
    one=change(change(no_retry,'cases 2;','cases 1;'),'  arrivals fixed [0min,0min]\n','')
    pair('cf-silent',no_retry,expected(case_rows=SERIAL),no_retry,one,expected(),'A11. Explicit V retains assumption at capacity1 and cases1. Text limits inactivity to inter-case conflicts; same-case route history is still tracked.',fault='one-case',assumptions={'Ledger.holder.concurrency':'version_check'})
    pair('cf-label',CL,YES,'Ledger','Book',YES,'A12. Actor rename preserves time/work/verdicts/state values/diagnostics. Use independent name+byte-offset map, not removal of semantic subtrees.',fault='renamed')
    pair('cf-label-lock',lock,ke,'ledger_lock','slots',ke,'A12 resource rename: same inequality2>1, times1,2, claims2. Name lock grants no privilege.',fault='renamed')
    pair('cf-agency',no_retry,expected(case_rows=SERIAL),CHECK,'',expected(case_rows=SERIAL),'A13c. Both variants finish1,2 claims2. With V: Unknown, state_guards once, claim refusal omitted. Removing ONLY V permits ordinary symbolic refusal.',fault='ordinary-result')
    gate='''model Gate version "0.2" {
 type V=enum{pass,fail};type R=enum{committed,conflict};observed case{gate:optional<V>}
 actor Source{capacity 1;capability read()->V{duration fixed 1min;result nondet{pass,fail}}}
 actor Judge{capacity 1;capability test(v:V)->V{duration fixed 1min;result nondet{pass,fail};writes gate}}
 actor Poster{capacity 1;capability post()->R{commits;requires gate==pass else conflict;duration fixed 1min;result nondet{committed}}}
 process P{s=Source.read();j=Judge.test(s);r=Poster.post();end done};cut C{include[Source,Judge,Poster]};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done
}
'''
    gate_e=expected();gate_e['terminals']=[dict(end='done',elapsed={'min':2,'max':3},work={'min':2,'max':3},result_at=None)]
    gate_b=expected();gate_b['terminals']=[dict(end='done',elapsed={'min':3,'max':3},work={'min':3,'max':3},result_at=None)]
    pair('cf-agency-requires',gate,gate_e,'requires gate==pass else conflict;','',gate_b,'A13b. Source+Judge=2 minutes. pass: Poster finish3/work3. fail: start refusal2/work2. Judge all_paths refusal of Poster.post, unlike V conservative omission.',fault='no-guard')
    ghost=change(CL,'  verify reachable end.claimed','  contract K { invariant Ledger.claim.commit -> r == committed }\n  verify reachable end.claimed')
    ge=copy.deepcopy(YES);ge['properties']=[prop('contract K #1 invariant')]+ge['properties'];gb=copy.deepcopy(ge);gb.update(verdict='violated',exit_code=1);gb['properties'][0]['verdict']='violated'
    pair('cf-no-ghost-commit',ghost,ge,'r == committed }\n  verify','r == conflict }\n  verify',gb,'A16. Every Commit has r=committed. Version-conflict Finish is NOT Commit although it consumes1 minute. Inverted invariant catches genuine commits; F03 false conflict Commit breaks positive invariant.',fault='inverted-invariant')
    av=change(change(change(atomic,'committed, rejected_duplicate','committed, rejected_duplicate, conflict'),'      commits','      version_check [keys] else conflict\n      commits'),'a = Ledger.post(case.id); end done','a = Ledger.post(case.id); if a==committed {end done} else {end conflict_end}')
    av=change(av,'verify reachable end.done','verify reachable end.done\n verify reachable end.conflict_end')
    ave=expected(case_rows=cases([(0,[('done',1,1),('conflict_end',1,1)])]*2),properties=[prop('contract C #1 at_most_once'),prop('reachable end.done'),prop('reachable end.conflict_end')])
    avb=expected('violated',case_rows=ae['case_outcomes'],properties=[prop('contract C #1 at_most_once'),prop('reachable end.done'),prop('reachable end.conflict_end','violated')])
    pair('cf-version-atomic',av,ave,'      version_check [keys] else conflict\n','',avb,'A17. V precedes atomic. After first insert other Task rejects without atomic evaluation/registered write. With V each case wins/loses1/work1, keys singleton; without V both done1/work1 keys{1,2}.')
    wide=change(CL,'    state holder:',''.join(f'    state padding{i}:bool=false\n' for i in range(65))+'    state holder:')
    pair('cf-wide-state',wide,YES,CHECK,'',NO,'A24. 65 preceding observed bools put holder ID above63. A01 oracle unchanged. Fixed u64 masks must not truncate/panic.',assumptions={'Ledger.holder.concurrency':'version_check','Ledger.claims.concurrency':'commutative'})
    comm='''model Loss version "0.2" {
 type T=enum{ok};actor Ledger{capacity 2;state n:0..2=0 last_write_wins;capability add()->T{writes n;finish{n+=1};duration fixed 1min;result nondet{ok}}}
 process P{r=Ledger.add();end done};cut C{include[Ledger]};bounds{cases 2;live_tasks 1;clock 5min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.done
}
'''
    ce=expected(case_rows=cases([(0,[('done',1,1)])]*2),properties=[prop('reachable end.done'),prop('state_in_range'),prop('no_lost_update Ledger.n')]);cb=copy.deepcopy(ce);cb.update(verdict='violated',exit_code=1);cb['properties'][-1]['verdict']='violated'
    pair('cf-lww-commutative',comm,ce,'n+=1','n=1',cb,'A23. Concurrent Add yields n2, no loss even with LWW. Assign1 twice yields n1; second stale SAME-value overwrite is loss.',fault='assign')
    static=change(no_retry,'state claims:', 'state spare:H=free\n    state claims:')
    se=expected(case_rows=SERIAL)
    tests=[('s01-close','[holder]','[holder','E-SYN-0','invalid'),('s02-result-missing','else conflict\n','else\n','E-SYN-0','invalid'),('s03-type','else conflict','else absent','E-TYPE-0','invalid'),('s04-name','[holder]','[unknown]','E-NAME-0','invalid'),('s05-unwritten','[holder]','[holder, spare]','U-SEM-0','unsupported'),('s06-duplicate-target','[holder]','[holder, holder]','U-SEM-0','unsupported'),('s07-duplicate-item',CHECK,CHECK+CHECK,'U-SEM-0','unsupported'),('s08-overlap','else conflict','else committed','U-SEM-0','unsupported'),('s09-start','      commits','      start {holder=taken}\n      commits','U-SEM-0','unsupported'),('s15-retry','      commits','      retry 1\n      commits','U-SYN-0','unsupported'),('s16-priority','      commits','      priority 1\n      commits','U-SYN-0','unsupported'),('s17-commutative','      commits','      commutative holder\n      commits','U-SYN-0','unsupported'),('s18-conflict-keyword','      commits','      conflict last_write_wins\n      commits','U-SYN-0','unsupported'),('s22-empty','[holder]','[]','E-SYN-0','invalid')]
    for id,old,new,code,verdict in tests:
        pair('cf-'+id,static,se,old,new,expected(verdict,codes=[code]),f'A14/{id}. Dedicated capacity1 control prevents G5 masking. Exactly one changed item. Fault statically {code}/{verdict}, no runtime/time.',fault='invalid-item')
    kernel='''model Kernel version "0.2" {
 type H=enum{free,taken,conflict};latent case{risk:bool=false};kernel K given[risk]{false->{1:taken};true->{1:taken}}
 actor Ledger{capacity 1;state holder:H=free;capability claim()->H{kernel_reads latent [risk];writes holder;finish{holder=taken};duration fixed 1min;outcomes kernel K}}
 process P{r=Ledger.claim();end done};cut C{include[Ledger]};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done
}
'''
    pair('cf-s10-kernel',kernel,expected(),'outcomes kernel K','version_check [holder] else conflict;outcomes kernel K',NO,'S10. Kernel given case latent bool only, NOT Actor observed. Control legal; adding V sole forbidden combination.',fault='version')
    for id,decl in [('s11-case-lww','observed case {flag:bool=false}'),('s12-percase-lww','actor A{instance per_case;latent flag:bool=false}'),('s13-latent-lww','actor A{latent flag:bool=false}')]:
        source='model Placement version "0.2" {\n '+decl+'\n process P{end done};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done\n}\n'
        pair('cf-'+id,source,expected(),'flag:bool=false','flag:bool=false last_write_wins',NO,f'{id}: NOT shared ActorObserved. LWW suffix U-SEM-0, not latent policy read. NO capabilities so no unrelated per_case restriction.',fault='suffix')
    mix='''model Mix version "0.2" {
 type T=enum{ok};actor Ledger{capacity 2;state x:0..6=2;
 capability a()->T{duration fixed 1min;result nondet{ok};writes x;finish{x+=1}}
 capability b()->T{duration fixed 1min;result nondet{ok};writes x;finish{x+=1}}}
 process P{a=Ledger.a();b=Ledger.b();end done};cut C{include[Ledger]};bounds{cases 2;live_tasks 1;clock 5min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.done
}
'''
    pair('cf-s14-mixed',mix,expected(case_rows=cases([(0,[('done',2,2)])]*2)),'finish{x+=1}}}\n process','finish{x-=1}}}\n process',NO,'S14. Four Add from2 end6, each case2 minutes/work2. b Subtract leaves self-pairs C but a-b unresolved. No blanket Add/Subtract acceptance.',fault='subtract')
    for entry,old,new in [('writes','writes [holder, claims]','writes [Other.x, claims]'),('finish','holder = taken','Other.x = taken'),('start','      commits','      start {Other.x=taken}\n      commits')]:
        src=change(change(no_retry,'  process P','  actor Other {state x:H=free}\n  process P'),'include [Ledger]','include [Ledger,Other]')
        pair('cf-s19-other-'+entry,src,se,old,new,expected('unsupported',codes=['U-SYN-0']),f'S19 {entry}. External Actor write unsupported at this independent grammar entrance.',fault='owner')
    for method,src in [('version',no_retry),('lock',lock),('lww',lww)]:
        src=change(change(change(src,'cases 2','cases 1'),'  arrivals fixed [0min,0min]\n',''),'live_tasks 1','live_tasks 2')
        src=change(src,'    if r == committed { end claimed } else { end retry_needed }','    end claimed')
        pair('cf-s20-fork-'+method,src,expected(),'r = Ledger.claim()','fork f {a = Ledger.claim(); b = Ledger.claim()}; join all f',NO,'S20. Same-case sibling holder writers structurally forbidden irrespective of '+method+'. Inter-case resolution does not bypass sibling check.',fault='siblings')
    pair('cf-s23-duplicate-lww',lww,le,'last_write_wins','last_write_wins last_write_wins',NO,'S23. Duplicate suffix U-SEM-0, never silently discarded.',fault='duplicate')
    extra(CL,no_retry,lww,le,lock,ke,lb)
    materialize()


def extra(CL,no_retry,lww,le,lock,ke,lb):
    marker=change(change(CL,'    state holder:','    state started:0..2=0\n    state holder:'),'      commits','      start {started+=1}\n      commits')
    pair('cf-start-marker',marker,YES,'      start {started+=1}\n','',YES,'Unchecked start effects persist. Both Tasks increment started at0 so started2 even though only one finish commits. V only checks holder and never rolls back start. Removing start changes started2 to0 but not finish/terminal arithmetic.',fault='no-start')
    read='''model Read version "0.2" {
 type R=enum{ok};actor Ledger{source record;capacity 1;state holder:bool=false;capability read()->R{reads [Ledger];duration fixed 1min;result nondet{ok}}}
 process P{r=Ledger.read();end done};cut C{include[Ledger]};bounds{cases 1;live_tasks 1;clock 5min};verify reachable end.done
}
'''
    pair('cf-s21-read-qualified',read,expected(),'reads [Ledger]','reads [Ledger.holder]',expected('invalid',codes=['E-SYN-0']),'S21. A5 preserves observed pre-C5 classification: qualified reads is E-SYN-0 expected closing bracket, exit3. C5 owns later U-SYN-0 repair; this is NOT a new reads feature.',fault='qualified')
    minus='''model Minus version "0.2" {
 type T=enum{ok};actor Ledger{capacity 2;state n:0..2=2;capability sub()->T{writes n;finish{n-=1};duration fixed 1min;result nondet{ok}}}
 process P{r=Ledger.sub();end done};cut C{include[Ledger]};bounds{cases 2;live_tasks 1;clock 5min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.done
}
'''
    pair('cf-subtract',minus,expected(case_rows=cases([(0,[('done',1,1)])]*2)),'n-=1','n=0',NO,'All Subtract operations commute:2-1-1=0, times1/work1. Assign0 twice is NOT commutative even though idempotent.',fault='assign')
    pair('cf-grain',CL,YES,'  actor Ledger {','  actor Ledger { grain team',YES,'A12 grain control. Merely labeling a team cannot change inter-case updates, values, time/work or verdicts.',fault='team')
    picker='''model Pair version "0.2" {
 type Pick=enum{left,right};type R=enum{committed,conflict};type H=enum{free,taken}
 actor Picker{capacity 2;capability choose()->Pick{duration fixed 1min;result nondet{left,right}}}
 actor Ledger{capacity 2;state holder:H=free;
 capability a()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken};duration fixed 1min;result nondet{committed}}
 capability b()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken};duration fixed 1min;result nondet{committed}}}
 process P{p=Picker.choose();if p==left {a=Ledger.a()} else {b=Ledger.b()};end done}
 cut C{include[Picker,Ledger]};bounds{cases 2;live_tasks 1;clock 10min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify safety no_resource_overcommit
}
'''
    pe=expected(case_rows=cases([(0,[('done',2,2)])]*2))
    pair('cf-one-sided',picker,pe,'capability b()->R{version_check [holder] else conflict;','capability b()->R{',NO,'A18. Picker1+writer1 gives each case done2/work2. Both V: all paths exactly1 Commit/1 conflict Finish. Remove only b V: a-b AND b-b unresolved.',fault='one-sided')
    selflocks=change(change(change(picker,' actor Picker',' resource RA capacity 1;resource RB capacity 1; actor Picker'),'capability a()->R{','capability a()->R{uses RA:1;'),'capability b()->R{','capability b()->R{uses RB:1;')
    # Different locks solve each self-pair, never a-b. Serial choices may have
    # done3 rather than2, so do not attach the simultaneous marginal oracle.
    pair('cf-cross-pair',selflocks,expected(),'capability b()->R{uses RB:1;version_check [holder] else conflict;','capability b()->R{uses RB:1;',NO,'F26 isolated. RA solves a-a, RB solves b-b. Different names do NOT establish mutual exclusion for a-b. Removing b V leaves ONLY the mixed pair unresolved.',fault='one-sided')
    mixed='''model Mixed version "0.2" {
 type Pick=enum{left,right};type R=enum{committed,conflict};resource RA capacity 1
 actor Picker{capacity 2;capability choose()->Pick{duration fixed 1min;result nondet{left,right}}}
 actor Ledger{capacity 2;state n:0..2=0;
 capability a()->R{uses RA:1;version_check [n] else conflict;commits;writes n;finish{n=1};duration fixed 1min;result nondet{committed}}
 capability b()->R{version_check [n] else conflict;commits;writes n;finish{n+=1};duration fixed 1min;result nondet{committed}}}
 process P{p=Picker.choose();if p==left {a=Ledger.a()} else {b=Ledger.b()};end done}
 cut C{include[Picker,Ledger]};bounds{cases 2;live_tasks 1;clock 10min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify safety no_resource_overcommit
}
'''
    pair('cf-mixed-resolution',mixed,expected(),'capability b()->R{version_check [n] else conflict;','capability b()->R{',NO,'Pair-specific evidence: a-a is lock RA, a-b bilateral V, b-b Add commutative. Two a choices serialize finishes2,3; other choices finish2 with one version rejection. n remains1 and in range. Removing only b V leaves a-b unresolved while BOTH self-pairs remain solved.',fault='one-sided',assumptions={'Ledger.n.concurrency':'Ledger.a/Ledger.a: lock RA; Ledger.a/Ledger.b: version_check; Ledger.b/Ledger.b: commutative'})
    # A19 is also exercised publicly with a LWW escape hatch: dropping only y
    # from V leaves a legal model whose stale y overwrite becomes a loss.
    vector='''model Vector version "0.2" {
 type Pick=enum{long,short};type H=enum{free,taken};type R=enum{committed,conflict}
 actor Picker{capacity 2;capability choose()->Pick{duration fixed 1min;result nondet{long,short}}}
 actor Ledger{capacity 2;state x:H=free;state y:H=free last_write_wins;
 capability long()->R{version_check [x,y] else conflict;commits;writes[x,y];finish{x=taken;y=taken};duration fixed 5min;result nondet{committed}}
 capability short()->R{version_check [y] else conflict;commits;writes y;finish{y=taken};duration fixed 1min;result nondet{committed}}}
 process P{p=Picker.choose();if p==long {a=Ledger.long()} else {b=Ledger.short()};end done}
 cut C{include[Picker,Ledger]};bounds{cases 2;live_tasks 1;clock 10min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify safety no_resource_overcommit
}
'''
    pair('cf-check-vector',vector,expected(),'[x,y] else conflict','[x] else conflict',expected('violated'),'A19 public supplement. A short writer touches ONLY y at2, while long writer runs1..6. V[x,y] rejects long. Dropping y is legal because y has LWW, but then stale long overwrites y at6 and no_lost_update is violated. An x-only check cannot pass this control.',fault='first-only')
    aba='''model ABA version "0.2" {
 type Pick=enum{long,short};type H=enum{free,taken};type R=enum{committed,conflict}
 actor Picker{capacity 2;capability choose()->Pick{duration fixed 1min;result nondet{long,short}}}
 actor Ledger{capacity 2;state holder:H=free;
 capability long()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken};duration fixed 5min;result nondet{committed}}
 capability short()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken};duration fixed 1min;result nondet{committed}}
 capability release()->R{version_check [holder] else conflict;writes holder;finish{holder=free};duration fixed 1min;result nondet{committed}}}
 process P{p=Picker.choose();if p==long {a=Ledger.long();if a==conflict{end conflicted}else{end committed_end}} else {b=Ledger.short();c=Ledger.release();end released}}
 cut C{include[Picker,Ledger]};bounds{cases 2;live_tasks 1;clock 10min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.conflicted
}
'''
    pair('cf-aba',aba,expected(),'holder=free};duration','holder=taken};duration',expected(),'A20 trace oracle (not inferred from root). Prefix long1..6, short1..2 writes taken, release2..3 writes free. At6 value equals initial free yet long MUST conflict, work5. Control release=taken distinguishes restoration but never excuses stale history. Cases can independently pick either arm.',fault='no-restore')
    # A21 trace input: long and cancelled writer are NOT fork siblings. Each
    # process chooses one mode; the short Fast branch cancels only local slow.
    cancel='''model Cancel version "0.2" {
 type Pick=enum{long,short};type H=enum{free,taken};type R=enum{committed,conflict}
 actor Picker{capacity 2;capability choose()->Pick{duration fixed 1min;result nondet{long,short}}}
 actor Fast{capacity 2;capability go()->R{duration fixed 1min;result nondet{committed};cancel forbidden_after_start}}
 actor Ledger{capacity 2;state holder:H=free;
 capability long()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken};duration fixed 5min;result nondet{committed}}
 capability slow()->R{version_check [holder] else conflict;writes holder;finish{holder=taken};duration fixed 3min;result nondet{committed};cancel immediate}}
 process P{p=Picker.choose();if p==long {a=Ledger.long();if a==conflict{end conflicted}else{end committed_end}}else{fork group{f=Fast.go();s=Ledger.slow()};join any group where result==committed tie by [f,s] remaining cancel_then_drain if none -> cancelled;end cancelled}}
 cut C{include[Picker,Fast,Ledger]};bounds{cases 2;live_tasks 2;clock 10min};arrivals fixed[0min,0min];policy fifo tie_by case_id;verify reachable end.committed_end
}
'''
    pair('cf-cancel',cancel,expected(),'cancel immediate','cancel forbidden_after_start',expected(),'A21 trace oracle: long1..6; slow1..4 cancelled at2 by Fast1..2. Immediate ACK must NOT write/notify, hence long committed6. Drain writes at4, hence long conflicts6. Start effects, if separately added, are not rolled back.',fault='drain')
    started=change(cancel,'state holder:H=free;','state holder:H=free;state marker:H=free last_write_wins;')
    started=change(started,'capability long()->R{version_check [holder] else conflict;commits;writes holder;finish{holder=taken}', 'capability long()->R{version_check [holder,marker] else conflict;commits;writes [holder,marker];finish{holder=taken;marker=taken}')
    started=change(started,'capability slow()->R{','capability slow()->R{start{marker=taken};')
    pair('cf-cancel-start',started,expected(),'start{marker=taken};','',expected(),'A21 start-receipt supplement. Select prefix long start1 before slow start1; slow start writes marker=taken and notifies long on SECOND checked variable. Immediate cancel ACK2 cannot roll back that write/notification. Holder still free but long conflicts6. Removing only slow start makes long committed6. Marker LWW makes static start writer legal without allowing checked start writes.',fault='no-start')
    # New output modes remain separate jobs, not aliases of exhaustive proof.
    sample=change(lww,'  policy fifo','  simulation replications 10 seed 1 confidence 0.95\n  policy fifo')
    pair('cf-simulation',sample,expected('inconclusive'),'capacity 1','capacity 2',NO,'U16/sample. Serial execution has ten complete replications, NoLostUpdate inconclusive/sample/null witness, not satisfied. Capacity2 residual finish order is unsupported, no successful partial replication.',fault='overlap')
    ROWS['cf-simulation']['mode']=ROWS['cf-simulation.broken-overlap']['mode']='simulate'
    pair('cf-explain',lww,le,'capacity 1','capacity 2',lb,'A05/explain. Configuration-free input. Overlap gives one independently replayed shortest NoLostUpdate prefix ending Finish1 with same taken overwrite. Serial has none.',fault='overlap')
    ROWS['cf-explain']['mode']=ROWS['cf-explain.broken-overlap']['mode']='explain'
    for label,base,mode,good,bad in [('exhaustive',lww,'exhaustive','satisfied','violated'),('simulation',sample,'simulation','inconclusive','unsupported')]:
        conf=change(base,'  process P','  configuration Left{cut C;process P}\n  configuration Right{cut C;process P}\n  process P')
        name='cf-compare-'+label
        pair(name,conf,{'verdict':good},'capacity 1','capacity 2',{'verdict':bad},'Comparison preserves each side property/assumption contract; no ranks, no uniform scheduler. Both same declared configuration choices.',fault='overlap')
        for n in [name,name+'.broken-overlap']:ROWS[n]['mode']='compare';ROWS[n]['invocation']={'command':'compare','plans':['Left','Right'],'mode':mode}


def materialize():
    controls=[]
    for name,row in ROWS.items():
        if '.broken-' not in name:
            s=re.sub(r'\bversion_check\s*\[[^\]]*\]\s*else\s+\w+', '', row['source']).replace(' last_write_wins','')
            s=re.sub(r'\bcapacity [23]\b','capacity 1',s)
            controls.append(dict(name=name,original=row['source'],legacy_control=s,configuration='Left' if row['mode']=='compare' else None))
    for name,row in ROWS.items():
        row['expected'].pop('exit_code',None)
        serial_warning_bases={'cf-serial','cf-lww','cf-lww-clock','cf-lock','cf-silent','cf-label-lock','cf-agency','cf-explain'}
        if name.split('.broken-')[0] in serial_warning_bases and row['mode'] not in ('simulate','compare') and row['expected']['verdict'] not in ('unsupported','invalid','inconclusive'):
            row['warnings']=['W-UNREACH-1','W-UNREACH-2']
        else:
            row['warnings']=None
        d=ROOT/'fixtures'/name;d.mkdir(exist_ok=True)
        (d/'calc.md').write_text('# Independent hand calculation — ADR 0041\n\n'+row['calc']+'\n\nStage C: authored oracle, not observed implementation behavior.\n')
        (d/'expected.json').write_text(json.dumps(row['expected'],ensure_ascii=False,indent=2)+'\n')
        (d/'model.relca').write_text(row['source'])
        if row['mode']!='check':
            (d/'invocation.json').write_text(json.dumps(row.get('invocation',{'command':row['mode']}))+'\n')
    for file,data in [('conflict_names.json',sorted(ROWS)),('conflict_pairs.json',PAIRS),('conflict_legacy_controls.json',controls),('conflict_oracles.json',[{k:v for k,v in r.items() if k not in ('source','expected','calc')} for r in ROWS.values()])]:
        (HERE/file).write_text(json.dumps(data,indent=2)+'\n')
    (HERE/'conflict_wire_names.rs').write_text('&[\n'+''.join(f'    "{n}",\n' for n in sorted(ROWS))+']\n')
    (HERE/'conflict_pair_tests.rs').write_text('acceptance! {\n'+''.join(f'    pair_{i:03} => "{p["good"]}",\n' for i,p in enumerate(PAIRS,1))+'}\n')
    print(len(PAIRS),'pairs',len(ROWS),'directories')


if __name__=='__main__':
    main()
