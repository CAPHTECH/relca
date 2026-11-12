"""Independent ADR0037 inputs and arithmetic; never imports or runs Relca.

Run from any directory with python3 -B. Every mutation is a single replacement
and is checked to affect the source. Expected data is authored here, not captured.
"""
import json
import hashlib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent
PAIRS = []
NAMES = []
CONTROLS = []


def change(s, a, b):
    t = s.replace(a, b)
    assert s != t, a
    return t


def rng(a, b=None):
    return {"min": a, "max": a if b is None else b}


def terminal(end, time, work=None, result=None):
    return {"end": end, "elapsed": rng(*time) if isinstance(time, tuple) else rng(time),
            "work": rng(*(work if isinstance(work, tuple) else (time if work is None else work,))) ,
            "result_at": None if result is None else rng(result)}


def expected(verdict="satisfied", props=None, terminals=None, codes=(), truncated=False, **extra):
    codes = list(codes)
    if any(c in ('U-SYN-0', 'E-SYN-0') for c in codes):
        codes.append('W-CUT-0')
    e = {"verdict": verdict, "diagnostics": [{"code": c} for c in codes]}
    if verdict not in ("invalid", "unsupported"):
        e["bounds"] = {"truncated": truncated}
    else:
        e["bounds"] = {"truncated": None}
    if props is None:
        props = [] if verdict in ("invalid", "unsupported") else [verdict]
    e["properties"] = [{"id": f"contract Controls #{i+1} invariant", "verdict": v}
                       for i, v in enumerate(props)]
    if terminals is not None:
        e["terminals"] = terminals
    elif verdict in ("invalid", "unsupported"):
        e["terminals"] = []
    e.update(extra)
    return e


def model(declarations, process, clause, actors, bounds="cases 1; live_tasks 2; clock 10min", more=""):
    if not actors:
        declarations += '\n  actor Witness {}'
        actors = 'Witness'
    return (f'model StateInvariant version "0.2" {{\n{declarations}\n'
            f'  process Flow {{ {process} }}\n  cut C {{ include [{actors}] }}\n'
            f'  bounds {{ {bounds} }}\n{more}\n  contract Controls {{ {clause} }}\n}}\n')


APP_DECL = '''  type Verdict = enum { pass, fail }
  observed case { registered: bool = false; approval: optional<Verdict> }
  actor Approver {
    capacity 1
    capability approve() -> Verdict { duration fixed 2min; result nondet { pass, fail }; writes approval }
  }
  actor Ledger {
    capacity 1
    capability post() -> Verdict { duration fixed 1min; result nondet { pass }; writes registered; finish { registered = true } }
  }'''
APP_FLOW = 'j = Approver.approve(); if approval == fail { end returned } else {}; p = Ledger.post(); end posted'
APP_CLAUSE = 'invariant registered -> approval == pass'
APP = model(APP_DECL, APP_FLOW, APP_CLAUSE, 'Approver, Ledger')
APP_TIMES = [terminal('returned', 2), terminal('posted', 3)]
APP_CALC = ('Approval finishes at minute 2 with pass or fail (not probabilities). '
            'The fail guard ends returned at elapsed/work 2/2. Pass alone posts during [2,3], '
            'so posted elapsed/work is 3/3. registered is false at receipt, hence no activation there. '
            'At post finish registered=true and approval=pass. No invariant diagnostic is emitted.')

FIX_DECL = '''  type R = enum { ok }
  observed case { ok: bool = true }
  actor Fix { capacity 1; capability fix() -> R { duration fixed 1min; result nondet { ok }; start { ok = true } } }'''
FIX = model(FIX_DECL, 'r = Fix.fix(); end done', 'invariant ok == true', 'Fix')


def write(name, source, oracle, calc, invocation=None):
    assert '//' not in source and '#' not in source
    d = ROOT / 'fixtures' / name
    d.mkdir(exist_ok=True)
    (d / 'model.relca').write_text(source)
    (d / 'expected.json').write_text(json.dumps(oracle, ensure_ascii=False, indent=2) + '\n')
    if any(r.get('code') == 'W-CUT-0' for r in oracle.get('diagnostics', [])):
        old = json.loads((HERE / 'state_invariant_parse_cut_observation.json').read_bytes())
        calc += ('\n\n## Existing parser-error Cut behavior (observed once before C3)\n\n'
                 'Parsing fails before the '
                 'Cut AST is available, so the existing report adds W-CUT-0 even though the source '
                 'contains a Cut. Lowering errors retain only their error code. This is an old '
                 'behavior observation, not a capture of the new invariant expectation.\n\n'
                 'Pre-C3 binary SHA256: ' + old['binary_sha256'] + '. Exit: ' + str(old['exit']) +
                 '. Input:\n\n```relca\n' + old['source'] + '```\n\nstdout (verbatim):\n\n```json\n' +
                 old['stdout'] + '```\n\nstderr (verbatim):\n\n```text\n' + old['stderr'] + '```\n')
    (d / 'calc.md').write_text(f'# {name} — independent hand oracle\n\n'
                             'ADR 0037; design v0.2 §11.1; design v0.2 §12.4; design v0.2 §22.3. All times/work are minutes.\n\n'
                             + calc + '\n\nExpected data was frozen before implementation. '
                             'A failing pre-feature run is not semantic verification.\n')
    if invocation:
        (d / 'invocation.json').write_text(json.dumps(invocation, indent=2) + '\n')
    NAMES.append(name)


def pair(name, source, a, b, good, bad, calc, fault='changed', invocation=None):
    source2 = change(source, a, b)
    broken = name + '.broken-' + fault
    write(name, source, good, calc + '\n\nControl: ' + json.dumps(good), invocation)
    write(broken, source2, bad, calc + '\n\nSingle edit: ' + repr(a) + ' → ' + repr(b) +
          '\nFault/control oracle: ' + json.dumps(bad), invocation)
    PAIRS.append(dict(name=name, fault=fault, **{'from': a, 'to': b}))


def main():
    pair('sinv-approval', APP, 'if approval == fail { end returned } else {}; ', '',
         expected(terminals=APP_TIMES), expected('violated', terminals=[terminal('posted', 3)]),
         APP_CALC + ' Removing the guard admits fail registration at t=3; witness reads true/fail.', 'bypass')
    rewrite = change(APP, APP_CLAUSE, APP_CLAUSE + '; invariant Ledger.post.finish -> approval == pass')
    pair('sinv-rewrite', rewrite, 'end posted', 'again = Approver.approve(); end posted',
         expected(props=['satisfied']*2, terminals=APP_TIMES),
         expected('violated', props=['violated', 'satisfied'], terminals=[terminal('returned', 2), terminal('posted', 5)]),
         APP_CALC + ' Second approval occupies [3,5]. It may change approval to fail at 5. '
         'The event clause saw pass at 3 and remains satisfied; the state clause becomes violated at 5.', 'reapprove')
    vac = model(APP_DECL, 'j = Approver.approve(); end returned', APP_CLAUSE, 'Approver, Ledger')
    pair('sinv-vacuous', vac, 'end returned', 'p = Ledger.post(); end returned',
         expected('vacuous', terminals=[terminal('returned', 2)], codes=['W-UNREACH-0']),
         expected('violated', terminals=[terminal('returned', 3)]),
         'registered remains false throughout the complete control. Ledger.post is not_called (one W-UNREACH-0). '
         'Inserting post activates at 3 and the fail choice violates. No lexical posted end exists.', 'register')
    pair('sinv-clock', APP, 'clock 10min', 'clock 2min', expected(terminals=APP_TIMES),
         expected('inconclusive', terminals=[terminal('returned', 2)], truncated=True),
         APP_CALC + ' Clock2 permits approval finish and the returned end; post cannot finish at3. '
         'Incomplete exploration cannot report vacuous; no unreachable warnings on a cut.', 'truncated')
    transient = model('''  type V = enum { pass }
  observed case { po: optional<V>; receipt: optional<V> }
  actor Clerk { capacity 2
    capability po() -> V { duration fixed 5min; result nondet { pass }; writes po }
    capability receipt() -> V { duration fixed 3min; result nondet { pass }; writes receipt }
  }''', 'fork f { a = Clerk.po(); b = Clerk.receipt() }; join all f; end done',
        'invariant po == pass -> receipt == pass', 'Clerk')
    pair('sinv-transient', transient, 'fixed 5min', 'fixed 3min', expected(terminals=[terminal('done', 5, 8)]),
         expected('violated', terminals=[terminal('done', 3, 6)]),
         'Control receipt finishes at3, po at5: max(5,3)=5 elapsed and 5+3=8 work. '
         'At equal duration3, po-first finish exposes (pass,none) at3 between bundles, then receipt repairs. '
         'Both orders must be explored; elapsed3/work6 still completes.', 'same-time')
    latent = change(APP, '  actor Approver', '  latent case { extraction_correct: bool }\n  actor Approver')
    latent = change(latent, APP_CLAUSE, 'invariant registered -> extraction_correct == true')
    pair('sinv-latent', latent, 'if approval == fail', 'if extraction_correct == true',
         expected('violated', terminals=APP_TIMES), expected('invalid', codes=['E-POL-LATENT']),
         APP_CALC + ' Latent bool has two nondeterministic receipt valuations, not a uniform distribution. '
         'false/pass reaches a bad registered state. Reading truth in the process guard is E-POL-LATENT.', 'policy')
    cut = change(APP, '  actor Approver', '  type Difficulty = enum { routine, difficult }\n  actor Invoice { latent difficulty: Difficulty = routine }\n  actor Approver')
    cut = change(cut, 'include [Approver, Ledger]', 'include [Approver, Ledger, Invoice]')
    cut = change(cut, APP_CLAUSE, 'invariant Invoice.difficulty == routine && registered -> approval == pass')
    pair('sinv-cut', cut, 'include [Approver, Ledger, Invoice]', 'include [Approver, Ledger]; exclude Invoice reason "outside" loses "difficulty"',
         expected(terminals=APP_TIMES), expected('invalid', codes=['E-CUT-1']),
         APP_CALC + ' Invoice is read only on the antecedent. Its routine default makes that conjunct true. '
         'Excluding this read is E-CUT-1; the reason and loses text prevent an unrelated Cut warning.', 'excluded')
    shared = model('''  type R = enum { committed, rejected }
  actor Ledger { capacity 1; state posted: 0..2 = 0
    capability post() -> R { requires posted == 0 else rejected; duration fixed 1min; result nondet { committed }; writes posted; finish { posted += 1 } }
  }''', 'r = Ledger.post(); end done', 'invariant Ledger.posted <= 1', 'Ledger',
        'cases 2; live_tasks 1; clock 10min', '  arrivals fixed [0min,0min]')
    integer = lambda verdict, inv, ran='satisfied', **kw: expected(verdict, props=[], properties=[
        {'id':'contract Controls #1 invariant','verdict':inv}, {'id':'state_in_range','verdict':ran}], **kw)
    pair('sinv-shared', shared, 'requires posted == 0 else rejected; ', '', integer('satisfied','satisfied'),
         integer('violated','violated'),
         'Two cases arrive at0. One shared slot posts at1; guard rejects the next at1 without task work. '
         'Each case may work0 or1; joint total work1, refusal1, both ends1. Removing requires allows '
         'starts0/1 and finishes1/2, work1 each, total2, no refusal table. posted=2 is inside 0..2 '
         'but violates <=1 at2. Labels never create a per-case copy.', 'no-guard')
    ended = change(APP, 'capacity 1\n    capability approve', 'capacity 2\n    capability approve')
    ended = change(ended, '  actor Ledger {\n    capacity 1', '  actor Ledger {\n    capacity 1\n    state posted: 0..1 = 0')
    ended = change(ended, 'writes registered; finish { registered = true }', 'writes [registered, posted]; finish { registered = true; posted += 0 }')
    ended = change(ended, APP_CLAUSE, 'invariant approval == fail -> Ledger.posted == 0')
    ended = change(ended, 'cases 1;', 'cases 2;')
    ended = change(ended, '  contract Controls', '  arrivals fixed [0min,0min]\n  contract Controls')
    pair('sinv-ended-case', ended, 'posted += 0', 'posted += 1', integer('satisfied','satisfied'),
         integer('violated','violated','violated', bounds={'truncated':False,'range_violation_stops':True}),
         'Mixed outcomes: fail case ends2 with work2; pass case writes shared posted at3 with work3. '
         'The ended fail case is violated at global3, but completed_at/elapsed/work remain2. '
         'Across histories both case IDs have a violation (not both in the same history). Both-pass '
         'control ends3/4 with work3 each. Fault upper1 stops attempted2 at4, so posted terminal only3. '
         'Both-fail remains safe. A clean upper2 companion isolates the ended-case violation.', 'increment')
    clean = change(ended, '0..1 = 0', '0..2 = 0')
    pair('sinv-ended-clean', clean, 'posted += 0', 'posted += 1', integer('satisfied','satisfied'), integer('violated','violated'),
         'As sinv-ended-case, but upper2 permits both-pass ends3/4 and no range stop. '
         'Mixed histories still violate the already ended fail case at3, preserving its end2/work2.', 'increment')
    count = model('''  type R = enum { ok }
  observed case { count: 0..1 = 0 }
  actor Desk { capacity 1; capability take() -> R { duration fixed 1min; result nondet { ok }; writes count; finish { count += 1 } } }''',
        'r = Desk.take(); end done', 'invariant count <= 1', 'Desk')
    pair('sinv-range', count, 'end done', 's = Desk.take(); end done',
         integer('satisfied','satisfied', terminals=[terminal('done',1)]),
         integer('violated','inconclusive','violated', terminals=[], bounds={'truncated':False,'range_violation_stops':True},
                 state_range_violations=[{'time':2,'before':'1','attempted':'2','lower':'0','upper':'1'}]),
         'One take changes 0 to1 at1 and ends1/work1. Second take attempts1+1=2 at2; '
         '2 is not a reachable valuation. One range witness, boolean range_violation_stops=true, '
         'truncated=false. State invariant has no bad valuation but unexamined continuation: inconclusive.', 'twice')
    pair('sinv-initial', FIX, 'ok: bool = true', 'ok: bool = false', expected(terminals=[terminal('done',1)]),
         expected('violated', terminals=[terminal('done',1)]),
         'Receipt0 sees true in control and false in fault. Fix start0 repairs immediately, finish/end1 '
         'gives elapsed/work1 in both. Fault shortest path is receipt only: transition_count0, step0, time0. '
         'The violation must not prune or be erased by repair.', 'initial')
    start = model('''  type R = enum { ok }
  type Phase = enum { idle, running }
  observed case { phase: Phase = idle; registered: bool = false }
  actor Desk { capacity 1
    capability work() -> R { duration fixed 2min; result nondet { ok }; start { phase = running }; writes phase; finish { phase = idle } }
    capability post() -> R { duration fixed 1min; result nondet { ok }; writes registered; finish { registered = true } }
  }''', 'a = Desk.work(); b = Desk.post(); end done',
        'invariant phase == running -> registered == false', 'Desk')
    pair('sinv-start-effect', start, 'a = Desk.work(); b = Desk.post()', 'b = Desk.post(); a = Desk.work()',
         expected(terminals=[terminal('done',3)]), expected('violated', terminals=[terminal('done',3)]),
         'Control running0..2 has registered=false; post writes true at3 after idle. Fault post writes '
         'at1, work start1 creates running/true (bad), work finish3 repairs idle. Both elapsed/work3. '
         'Witness must be Start at1, not terminal.', 'order')
    label = change(shared, 'actor Ledger {', 'actor Ledger { grain document; source document;')
    pair('sinv-label', label, 'grain document', 'grain person', integer('satisfied','satisfied'), integer('satisfied','satisfied'),
         'Both labels use exactly one shared state bank: first post completes1; other refused1; '
         'joint work1, per-case work0..1, both end1. posted never exceeds1. document→person cannot '
         'change resource ownership. A ledger third variant is also tested.', 'person')
    mut = change(APP, 'capability post() -> Verdict {', 'capability post() -> Verdict { commits;')
    mut = change(mut, APP_CLAUSE, APP_CLAUSE + '; at_most_once Ledger.post.commit per case.id')
    mut_e = lambda: {'verdict':'unsupported','diagnostics':[]}
    pair('sinv-mutation', mut, APP_CLAUSE + '; ', '', mut_e(), mut_e(),
         APP_CALC + ' Mutation kinds2 and8 each have the same one guard-bypass site, two generated '
         'mutants in total, six unsupported kinds. Kind8 is detected by Controls #1 invariant only '
         'with that clause present. Without it, once stays satisfied and state guard gives no legacy '
         'Agency refusal check: kind8 undetected. Root remains unsupported/exit4, not violated.', 'no-invariant', {'command':'mutate'})
    cancel = model('''  type T = enum { pass, ok }
  observed case { changed: bool = false }
  actor Fast { capacity 1; capability win() -> T { duration fixed 1min; result nondet { pass }; cancel forbidden_after_start } }
  actor Slow { capacity 1; capability work() -> T { duration fixed 5min; result nondet { ok }; writes changed; finish { changed = true }; cancel cooperative ack_within 1min } }''',
        'fork f { fast = Fast.win(); slow = Slow.work() }; join any f where result == pass tie by [fast,slow] remaining cancel_then_drain if none -> rejected; end done; end rejected',
        'invariant changed == false', 'Fast, Slow', 'cases 1; live_tasks 2; clock 20min')
    pair('sinv-cancel', cancel, 'cancel cooperative ack_within 1min', 'cancel forbidden_after_start',
         expected(terminals=[terminal('done',2,3,1)], codes=['W-UNREACH-2']),
         expected('violated', terminals=[terminal('done',5,6,1)], codes=['W-UNREACH-2']),
         'Fast finishes1, chosen result_at1. Slow ack/end2 suppresses finish write, work=1+2=3. '
         'Forbidden cancellation drains to5 and writes true, work=1+5=6. Rejected lexical end is '
         'unreachable (W-UNREACH-2). Immediate ack variant ends1/work2, separately tested.', 'drain')
    sim = change(FIX, '  contract Controls', '  simulation replications 4 seed 17 confidence 0.95\n  contract Controls')
    pair('sinv-simulation', sim, 'ok: bool = true', 'ok: bool = false',
         {'verdict':'inconclusive','diagnostics':[], 'properties':[{'id':'contract Controls #1 invariant','verdict':'inconclusive'}],
          'simulation':{'completed_replications':4,'truncated_replications':0}},
         {'verdict':'violated','diagnostics':[], 'properties':[{'id':'contract Controls #1 invariant','verdict':'violated'}],
          'simulation':{'completed_replications':4,'truncated_replications':0}},
         'Four identical deterministic paths; draws0, done count4, end/work1. Control has no witness '
         'and is only inconclusive/sample, never safety proved. False initial fault violates every '
         'replication despite start repair; first witness replication1. Arrivals[0] variant also required.', 'initial', {'command':'simulate'})
    sticky = change(FIX, '  actor Fix', '  actor Bad { capacity 1; capability bad() -> R { duration fixed 1min; result nondet { ok }; writes ok; finish { ok = false } } }\n  actor Fix')
    sticky = change(sticky, 'include [Fix]', 'include [Bad, Fix]')
    pair('sinv-sticky', sticky, 'r = Fix.fix()', 'b = Bad.bad(); r = Fix.fix()',
         expected(terminals=[terminal('done',1)], codes=['W-UNREACH-0']), expected('violated', terminals=[terminal('done',2)]),
         'Control calls Fix only: Bad.bad not_called, elapsed/work1. Fault Bad finishes1 with ok=false, '
         'Fix start1 repairs and ends2 with ok=true. Sticky failure survives, and task/end execution continues.', 'bad')
    rejected = change(FIX, 'cases 1;', 'cases 2;')
    rejected = change(rejected, '  contract Controls', '  arrivals fixed [0min,0min]\n  queue capacity 1 on_overflow reject\n  contract Controls')
    rej_e = lambda codes: expected('vacuous', codes=codes, props=[], properties=[{'id':'contract Controls #1 invariant','verdict':'vacuous',
         'cases':[{'case_id':1,'arrival':0,'verdict':'vacuous'},{'case_id':2,'arrival':0,'verdict':'vacuous'}]}], terminals=[])
    good_rej = rej_e([])
    good_rej['properties'][0]['cases'][0]['verdict']='satisfied'
    pair('sinv-rejected', rejected, 'queue capacity 1', 'queue capacity 0', good_rej, rej_e(['W-UNREACH-0','W-UNREACH-2']),
         'Admission batches before dispatch. Queue1 admits case1 and rejects case2; case1 satisfied '
         'and ends1/work1, case2 vacuous with null times. Root vacuous5. Queue0 rejects both, '
         'so no valuation, no activation, no task/end; both rows vacuous. Task and done warnings are '
         'W-UNREACH-0 and W-UNREACH-2, exactly.', 'zero-queue')
    bare = change(FIX, 'invariant ok == true', 'invariant ok')
    pair('sinv-bool-type', bare, 'ok: bool = true', 'ok: optional<bool> = true',
         expected(terminals=[terminal('done',1)]), expected('invalid', codes=['E-TYPE-0']),
         'Nonoptional builtin bool lowers to ==true. Explicit true does not make optional<bool> '
         'nonoptional. Bare optional atom is E-TYPE-0 before execution; comparison remains legal.', 'optional')
    grouped = change(APP, APP_CLAUSE, 'invariant (registered) -> (approval == pass)')
    pair('sinv-grouping', grouped, '(approval == pass)', '(registered -> approval == pass)',
         expected(terminals=APP_TIMES), expected('unsupported', codes=['U-SYN-0']),
         APP_CALC + ' Parentheses group state expressions, not nested implication. Outer arrow is '
         'weakest; a second arrow inside the right group is U-SYN-0 at that arrow.', 'nested')

    # Static candidate13: each rejecting input has a separately executable control.
    static = [
        ('enum', change(bare, '  actor Fix', '  type Truth = enum { true, false }\n  actor Fix'), 'ok: bool = true', 'ok: Truth = true', 'E-TYPE-0'),
        ('binding-bare', APP, 'invariant registered', 'invariant j', 'U-SEM-0'),
        ('binding-compare', APP, 'approval == pass }', 'j == pass }', 'U-SEM-0'),
        ('unknown', APP, 'invariant registered', 'invariant absent_name', 'E-NAME-0'),
        ('chained', APP, APP_CLAUSE, APP_CLAUSE + ' -> registered', 'U-SYN-0'),
        ('terminal', APP, 'invariant registered', 'invariant end.posted', 'U-SYN-0'),
        ('receipt', APP, 'invariant registered', 'invariant case.received', 'U-SYN-0'),
        ('event-bare', change(FIX, 'invariant ok == true', 'invariant Fix.fix.start -> ok == true'), '-> ok == true', '-> ok', 'U-SYN-0'),
        ('event-nested', change(FIX, 'invariant ok == true', 'invariant Fix.fix.start -> ok == true'), '-> ok == true', '-> (ok -> ok == true)', 'U-SYN-0'),
        ('event-chained', change(FIX, 'invariant ok == true', 'invariant Fix.fix.start -> ok == true'), '-> ok == true', '-> ok == true -> ok == true', 'U-SYN-0'),
        ('operand', bare, 'invariant ok', 'invariant ok ==', 'E-SYN-0'),
        ('delimiter', bare, 'invariant ok', 'invariant (ok', 'E-SYN-0'),
        ('mixed', bare, 'invariant ok', 'invariant ok && ok || ok', 'E-SYN-0'),
        ('qualified-name', APP, 'invariant registered', 'invariant Absent.flag == true', 'E-NAME-0'),
        ('qualified-field', APP, 'invariant registered', 'invariant Ledger.absent == true', 'E-NAME-0'),
    ]
    for word in ['always','eventually','until','forall','exists','clock','task','queue','resource','obligation','history']:
        static.append((word, bare, 'invariant ok', f'invariant {word} ok', 'U-SYN-0'))
    # case.id alone stays unsupported before and after A3 membership is added.
    static.append(('case-id', bare, 'invariant ok', 'invariant case.id', 'U-SYN-0'))
    local = change(APP, 'capacity 1\n    capability post', 'capacity 1\n    state flag: bool = true\n    capability post')
    local = change(local, APP_CLAUSE, 'invariant Ledger.flag == true')
    static.append(('qualified-bare', local, 'invariant Ledger.flag == true', 'invariant Ledger.flag', 'U-SYN-0'))
    collision = change(APP, '  actor Approver', '  latent case { j: bool = false }\n  actor Approver')
    collision = change(collision, 'j = Approver', 'approvalV = Approver')
    collision = change(collision, APP_CLAUSE, 'invariant j == false')
    static.append(('collision', collision, 'approvalV = Approver', 'j = Approver', 'U-SEM-0'))
    for ident, source, a, b, code in static:
        times = APP_TIMES if 'Approver' in source else [terminal('done',1)]
        pair('sinv-static-' + ident, source, a, b, expected(terminals=times),
             expected('unsupported' if code.startswith('U-') else 'invalid', codes=[code]),
             ('Control uses only declared state and has ' + ('approval2/post1 (returned2, posted3).' if 'Approver' in source else 'Fix1, end/work1.') +
              f' Single replacement is rejected statically with exactly {code}; no runtime terminal, '
              'no probability, bounds.truncated=null. Both expression sides and nested groups are tested separately.'), ident)
    nbare = change(count, 'invariant count <= 1', 'invariant count')
    pair('sinv-static-integer', count, 'invariant count <= 1', 'invariant count',
         integer('satisfied','satisfied',terminals=[terminal('done',1)]), expected('invalid', codes=['E-TYPE-0']),
         'count0→1 at1, safe upper1 and work1. A bare integer is not bool even if its value is0.', 'bare')

    ci = change(APP, '  actor Approver', '  latent case { j: 0..1 = 0 }\n  actor Approver')
    ci = change(ci, 'j = Approver', 'approvalV = Approver')
    ci = change(ci, APP_CLAUSE, 'invariant j <= 1')
    pair('sinv-integer-collision', ci, 'approvalV = Approver', 'j = Approver',
         integer('satisfied','satisfied',terminals=APP_TIMES), expected('unsupported',codes=['U-SEM-0']),
         APP_CALC + ' Latent bounded j stays0 and <=1 is true. Renaming only the approve binding '
         'to j creates a latent/binding collision, U-SEM-0 even on the integer comparison arm. '
         'It cannot silently select one namespace.', 'alias')
    conjunction = change(transient, 'po: optional<V>; receipt: optional<V>', 'po: optional<V>; receipt: optional<V>; registered: bool = false')
    conjunction = change(conjunction, '  actor Clerk', '  actor Ledger { capacity 1; capability post() -> V { duration fixed 1min; result nondet { pass }; writes registered; finish { registered = true } } }\n  actor Clerk')
    conjunction = change(conjunction, 'include [Clerk]', 'include [Clerk, Ledger]')
    conjunction = change(conjunction, 'join all f; end done', 'join all f; p = Ledger.post(); end done')
    conjunction = change(conjunction, 'invariant po == pass -> receipt == pass', 'invariant registered -> po == pass && receipt == pass')
    pair('sinv-conjunction', conjunction, '&& receipt == pass', '&& receipt == none',
         expected(terminals=[terminal('done',6,9)]), expected('violated',terminals=[terminal('done',6,9)]),
         'po5 and receipt3 run in parallel (elapsed5, work8); post adds1 (elapsed6, work9). '
         'Only at6 registered is true, both comparisons hold. Changing right receipt constant '
         'to none violates at6. The outer implication is weaker than the unparenthesized &&.', 'consequent')
    alias = '''model StateAlias version "0.2" {
  type V = enum { pass, fail }
  observed case { approval: optional<V> }
  actor A { capacity 1
    capability good() -> V { duration fixed 1min; result nondet { pass }; writes approval }
    capability other() -> V { duration fixed 1min; result nondet { pass }; writes approval }
  }
  process Good { approval = A.good(); end done }
  process Other { approval = A.other(); end done }
  cut C { include [A] }
  configuration X { cut C; process Good }
  configuration Y { cut C; process Other }
  bounds { cases 1; live_tasks 1; clock 4min }
  contract Controls { invariant approval == pass -> approval == pass }
}
'''
    pair('sinv-alias', alias, 'capability other() -> V { duration fixed 1min; result nondet { pass }; writes approval }',
         'capability other() -> V { duration fixed 1min; result nondet { pass } }',
         expected(terminals=[terminal('done',1)],codes=['W-UNREACH-0']), expected('unsupported',codes=['U-SEM-0']),
         'X calls Good and finishes1/work1; Other capability is not_called (W-UNREACH-0). '
         'Every approval binding in both processes writes the matching state. Removing just '
         'Other writer must reject even selection X, whose own writer is unchanged. StateInvariant '
         'name validation is model-wide, not selected-process-local. Both X and Y are tested.',
         'other-writer', {'command':'check','configuration':'X'})

    # Fixed crossings from plan section3.4.
    future = model('  observed case { ok: bool = false }', 'end done', 'invariant ok == true', '',
                   'cases 1; live_tasks 1; clock 4min', '  arrivals fixed [5min]')
    pair('sinv-future', future, 'clock 4min', 'clock 5min', expected('inconclusive',truncated=True,terminals=[]),
         expected('violated',terminals=[]),
         'Future arrival5 is beyond clock4: no initialized state or witness, inconclusive/truncated. '
         'At clock5 receipt false violates, end5/elapsed0/work0. Future is never initialized early.', 'received')
    seed = model('  latent case { flag: bool = true }', 'end done', 'invariant flag == true', '')
    pair('sinv-seed', seed, 'flag: bool = true', 'flag: bool', expected(terminals=[terminal('done',0)]),
         expected('violated',terminals=[terminal('done',0)]),
         'Fixed true has one receipt valuation. Removing default creates two valuations true/false; '
         'false receipt violates (not seed). End0/work0 in both; latent nondet is not probability1/2.', 'nondet')
    mass = model('  type T = enum { bad, good }\n  latent case { x: T ~ { bad: 0, good: 1 } }',
                 'end done', 'invariant x == good', '')
    pair('sinv-seed-mass', mass, '{ bad: 0, good: 1 }', '{ bad: 0.5, good: 0.5 }',
         expected(terminals=[terminal('done',0)]), expected('violated',terminals=[terminal('done',0)]),
         'Only positive-mass good initializes in control; placeholder bad(ValueId0) is not a state. '
         'Fault has bad mass1/2 and good mass1/2, so bad receipt violates. No probability field is '
         'added to the invariant. Both end0/work0.', 'positive-bad')
    atomic = model('''  type R = enum { ok }
  observed case { a: bool = false; b: bool = false }
  actor Desk { capacity 1; capability run() -> R { duration fixed 1min; result nondet { ok }; writes [a,b]; finish { a = true; b = true } } }''',
        'r = Desk.run(); end done', 'invariant a == true -> b == true', 'Desk')
    pair('sinv-bundle', atomic, 'b = true', 'b = false', expected(terminals=[terminal('done',1)]),
         expected('violated',terminals=[terminal('done',1)]),
         'Both assignments form one finish bundle at1. Before (false,false) inactive; after '
         '(true,true) safe. Never inspect (true,false) midpoint. Fault final (true,false) violates '
         'at1. Work/end1 unchanged.', 'false')
    noprune = change(FIX, 'ok: bool = true', 'ok: bool = false')
    pair('sinv-no-prune', noprune, 'end done', 's = Fix.fix(); end done',
         expected('violated',terminals=[terminal('done',1)]), expected('violated',terminals=[terminal('done',2)]),
         'Receipt violates at0 in both inputs. Fix repairs at start but sticky failure remains; '
         'one/two positive tasks still finish and end with elapsed/work1 or2. Violation never prunes.', 'second')

    # Configured compare and preserve input pairs. Explain never takes configuration.
    comp = '''model StateInvariantCompare version "0.2" {
  type R = enum { ok }
  observed case { ok: bool = true }
  actor Desk { capacity 1
    capability good() -> R { duration fixed 1min; result nondet { ok }; writes ok; finish { ok = true } }
    capability bad() -> R { duration fixed 1min; result nondet { ok }; writes ok; finish { ok = false } }
  }
  process AFlow { r = Desk.good(); end done }
  process BFlow { r = Desk.good(); end done }
  cut C { include [Desk] }
  configuration A { cut C; process AFlow }
  configuration B { cut C; process BFlow }
  bounds { cases 1; live_tasks 1; clock 10min }
  contract Controls { invariant ok }
}'''
    for mode in ['exhaustive','simulation']:
        src = comp if mode=='exhaustive' else change(comp, '  contract Controls', '  simulation replications 4 seed 17 confidence 0.95\n  contract Controls')
        ce = lambda v: {'verdict':v,'diagnostics':[], 'plans':[{'configuration':p,'report':{'diagnostics':([] if mode=='simulation' else [{'code':'W-UNREACH-0'}])}} for p in ['A','B']]}
        pair('sinv-compare-' + mode, src, 'process BFlow { r = Desk.good()', 'process BFlow { r = Desk.bad()',
             ce('satisfied' if mode=='exhaustive' else 'inconclusive'), ce('violated'),
             'Both plans start ok=true. Good finish1 preserves; Bad finish1 clears. A/B control '
             'safe/safe; changed B safe/violated. Both end/work1. Simulation4 has zero draws and '
             'done count4 each; safe row inconclusive/sample, bad row witness replication1. '
             'AFlow and BFlow are distinct process identities, so sampled operation keys are not '
             'aliased. Exhaustive unused capability warning is one W-UNREACH-0 per plan; simulation emits none.',
             'bad-plan', {'command':'compare','plans':['A','B'],'mode':mode})
    diff = change(APP, '  process Flow', '  process ToFlow { ' + APP_FLOW + ' }\n  process Flow')
    diff = change(diff, '  bounds', '  configuration From { cut C; process Flow }\n  configuration To { cut C; process ToFlow; intent { preserve Controls } }\n  bounds')
    pair('sinv-preserve', diff, 'process ToFlow { ' + APP_FLOW + ' }', 'process ToFlow { j = Approver.approve(); end returned }',
         {'verdict':'satisfied','diagnostics':[],'from':{'report':{'diagnostics':[]}},'to':{'report':{'diagnostics':[]}}},
         {'verdict':'vacuous','diagnostics':[{'code':'W-DIFF-1'}]*4,'from':{'report':{'diagnostics':[]}},'to':{'report':{'diagnostics':[{'code':'W-UNREACH-0'}]}}},
         APP_CALC + ' Preserve matches the invariant suffix and Controls ID. Identical plans '
         'summary satisfied1/vacuous0. No-post ToFlow remains registered=false: summary satisfied0/vacuous1, '
         'root vacuous5. To returned2/work2, From returned2/posted3. Removing the sole Ledger call '
         'removes four independently computed projections: actor Ledger, relation Ledger.post, '
         'meaning end.posted, resource Ledger. Preserve annotates but does not justify these changes: '
         'exactly four root W-DIFF-1. To has Ledger.post not_called W-UNREACH-0. Both process names '
         'are referenced by configurations; no W-CONF-0.',
         'vacuous', {'command':'diff','from':'From','to':'To'})

    NAMES.sort()
    (HERE / 'state_invariant_names.json').write_text(json.dumps(NAMES,indent=2)+'\n')
    (HERE / 'state_invariant_pairs.json').write_text(json.dumps(PAIRS,indent=2)+'\n')
    # Independently source-bound old-syntax controls, not post-feature outputs.
    import re
    for name in NAMES:
        s = (ROOT/'fixtures'/name/'model.relca').read_text()
        if (ROOT/'fixtures'/name/'invocation.json').exists():
            inv = json.loads((ROOT/'fixtures'/name/'invocation.json').read_text())
            if inv['command'] in ('diff','compare','mutate','check'):
                continue
        e = json.loads((ROOT/'fixtures'/name/'expected.json').read_text())
        if e.get('verdict') in ('invalid','unsupported'):
            continue
        t = re.sub(r'  contract Controls \{[^{}]*\}', '  verify safety no_resource_overcommit', s)
        assert t != s, name
        CONTROLS.append({'name':name,'original':s,'legacy_control':t})
    (HERE/'state_invariant_legacy_controls.json').write_text(json.dumps(CONTROLS,indent=2)+'\n')
    (HERE/'state_invariant_population.rs').write_text('// Closed ADR0037 population, append-only.\n#[allow(dead_code)]\npub const NAMES: &[&str] = &[\n'+''.join(f'    "{n}",\n' for n in NAMES)+'];\n')
    hashes={n:{f:hashlib.sha256((ROOT/'fixtures'/n/f).read_bytes()).hexdigest()
               for f in ['model.relca','expected.json','calc.md','invocation.json']
               if (ROOT/'fixtures'/n/f).exists()} for n in NAMES}
    (HERE/'state_invariant_fixture_hashes.json').write_text(json.dumps(hashes,indent=2)+'\n')
    print(f'{len(PAIRS)} pairs, {len(NAMES)} inputs, {len(CONTROLS)} old-syntax runtime controls')


if __name__ == '__main__':
    main()
