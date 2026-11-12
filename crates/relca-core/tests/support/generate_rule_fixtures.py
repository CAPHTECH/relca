"""ADR0044 hand oracles. Materialize inputs, never execute Relca or harvest output.

T21/24, record and version_check crossings are specified in separate integration checks
until ADR0038/41/42 integration. Agency C5 seams are separately handed off there.
"""
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[4]
HERE = Path(__file__).resolve().parent


def edit(s, old, new):
    assert old in s, old
    t = s.replace(old, new)
    assert t != s, old  # assert_ne witness for every string-built control
    return t


RULE = 'actor ApprovalRule { control policy; version "r1"; decides approval_required: bool = true }'
CHANGE = 'authority RuleChange { basis "amendment procedure"; grants cfo to change ApprovalRule; not_by [a1] }'
AMEND = 'amend ApprovalRule from "r1" to "r2" by cfo under RuleChange { approval_required = false }'
INTENT = 'intent { change rule ApprovalRule reason "exempt approval" }'
CONFIG = f'''configuration AsIs {{ cut C; process Flow }}
configuration ToBeB {{ cut C; process Flow
  {AMEND}
  {INTENT}
}}'''
R = f'''model RuleExample version "0.2" {{
type V = enum {{ pass, fail }}
type Ack = enum {{ committed, held }}
people [a1, cfo]
observed case {{ approval: optional<V> }}
{RULE}
actor Approver {{ executors [a1]; capability approve() -> V {{ duration fixed 2min; result nondet {{ pass, fail }}; writes approval }} }}
actor Ledger {{ capacity 1; capability post() -> Ack {{ requires ApprovalRule.approval_required == false || approval == pass else held; commits; duration fixed 1min; result nondet {{ committed }} }} }}
authority ApprovalGrant {{ basis "approval table"; grants a1 to Approver.approve; issued_by cfo }}
{CHANGE}
process Flow {{ a = Approver.approve(); ack = Ledger.post(); if ack == committed {{ end posted }} else {{ end held }} }}
cut C {{ include [ApprovalRule, Approver, Ledger] }}
{CONFIG}
bounds {{ cases 1; live_tasks 2; clock 20min }}
verify safety no_resource_overcommit
contract K {{ invariant Ledger.post.commit -> approval == pass }}
}}
'''
R0 = edit(edit(R, CONFIG, ''), CHANGE, '')
catalog, pairs, deferred = [], [], []


def terminal(end, elapsed, work):
    return dict(end=end, elapsed=dict(min=elapsed, max=elapsed),
                work=dict(min=work, max=work), result_at=None)


def hand(kind='A'):
    if kind == 'ZERO':
        return dict(verdict='satisfied', bounds=dict(truncated=False),
                    terminals=[terminal('done', 0, 0)])
    if kind == 'PASS':
        return dict(verdict='satisfied',bounds=dict(truncated=False),terminals=[terminal('posted',3,3)])
    v = {'A': 'satisfied', 'B': 'violated', 'Q': 'vacuous', 'CUT': 'inconclusive'}[kind]
    ts = [terminal('posted', 3, 3), terminal('held', 2, 2)] if kind == 'A' else (
        [terminal('held', 2, 2)] if kind == 'Q' else (
            [] if kind == 'CUT' else [terminal('posted', 3, 3)]))
    # ADR0002 terminal tables are keyed by end, not source-order arrays.
    return dict(verdict=v, bounds=dict(truncated=kind == 'CUT'), terminals=ts)


def refusal(code, message=None, count=1):
    e = dict(verdict='unsupported' if code.startswith('U-') else 'invalid',
             properties=[], terminals=[], bounds=dict(truncated=None),
             diagnostics=[dict(code=code, severity='error')])
    return e, dict(error_code=code, error_message=message, error_count=count,
                   parser=code == 'U-SYN-0')


def inv(configuration: str | None = 'AsIs', command='check', **kw):
    if command == 'diff':
        return dict(command=command, **{'from': kw.get('from_', 'AsIs'), 'to': kw.get('to', 'ToBeB')})
    if command in ('explain', 'mutate'):
        return dict(command=command)
    return dict(command=command, configuration=configuration) if configuration else dict(command=command)


def add(name, source, expected, invocation, oracle=None):
    assert not ('//' in source or '#' in source), name
    assert name not in [r['name'] for r in catalog]
    catalog.append(dict(name=name, source=source, expected=expected,
                        invocation=invocation, oracle=oracle or {}))


def pair(id, name, src, old, new, expected=None, bad=None, invocation=None,
         bad_invocation=None, oracle=None, bad_oracle=None, calculation=''):
    invocation = invocation or inv()
    good = 'rule-' + name
    broken = good + '.broken-' + id.lower()
    transformed = edit(src, old, new) if old != new else src
    assert transformed != src or bad_invocation != invocation
    add(good, src, expected or hand(), invocation, oracle)
    add(broken, transformed, bad or hand('B'), bad_invocation or invocation, bad_oracle)
    pairs.append(dict(id=id, good=good, broken=broken, before=old, after=new,
                      calculation=calculation, invocation_only=old == new))


def static(id, name, src, old, new, code, message=None, invocation=None, expected=None,
           calculation='', count=1):
    e, o = refusal(code, message, count)
    if invocation and invocation['command'] == 'diff':
        o['error_in_sides'] = True
        e = dict(verdict=e['verdict'], **{'from':dict(report=e), 'to':dict(report=e)})
    pair(id, name, src, old, new, expected=expected, bad=e, invocation=invocation,
         bad_oracle=o, calculation=calculation or f'{id}: one edit; {code}: {message or "name/type resolution"}. No execution, no time/work or success conversion.')


def build():
    pair('T01', 'basic', R, '', '', bad_invocation=inv('ToBeB'),
         oracle=dict(profile='A'), bad_oracle=dict(profile='B'),
         calculation='AsIs: pass gives approve[0,2]+post[2,3]=3min elapsed/work; fail refuses before Start at2, held2/work2. K sees a pass commit and is satisfied. ToBeB false makes requires true for both approval values: posted3/work3 only; fail commits and K is violated. Nondeterminism has no 1/2 probability.')
    pair('T02', 'diff', R, '  '+INTENT, '', expected=dict(verdict='violated'),
         bad=dict(verdict='violated'), invocation=inv(command='diff'),
         oracle=dict(diff=True, w1=0, w3=1), bad_oracle=dict(diff=True, w1=1, w3=1),
         calculation='Same process gives zero old structural rows. Effective r1/true -> r2/false is exactly one rule modified row. K S->V folds V. No preservation/override/maintain/loses gives W-DIFF-3=1. Removing reason adds W-DIFF-1=1, not a new business verdict.')
    preserve = edit(R, INTENT, 'intent { change rule ApprovalRule reason "exempt approval"; preserve K on rule ApprovalRule }')
    pair('T03', 'preserve', preserve, 'approval_required = false', 'approval_required = true',
         expected=dict(verdict='violated'), bad=dict(verdict='satisfied'), invocation=inv(command='diff'),
         oracle=dict(preserve='violated', w1=0, w3=0), bad_oracle=dict(preserve='satisfied', w1=0, w3=0),
         calculation='Version difference remains r1->r2 in both. False allows failing approval commit (K V), true refuses it (K S). Preservation summary V1/Q0/S0 versus V0/Q0/S1; all other bins0, never count V as Q.')
    static('T04', 'not-by', R, 'by cfo under RuleChange', 'by a1 under RuleChange', 'E-CON-0', 'rule change by a person excluded by not_by')
    selfauth = edit(edit(R, CHANGE, CHANGE+'\nauthority RuleChange2 { basis "r2 table"; grants a1 to change ApprovalRule; under ApprovalRule "r1" }'), 'by cfo under RuleChange', 'by a1 under RuleChange2')
    static('T05', 'self-authorize', selfauth, 'under ApprovalRule "r1"', 'under ApprovalRule "r2"', 'E-CON-0', 'rule change is authorized only under the amended rule')
    prior = edit(R, CHANGE, edit(CHANGE, 'not_by [a1]', 'not_by [a1]; under ApprovalRule "r1"'))
    static('T06', 'prior-version', prior, 'under ApprovalRule "r1"', 'under ApprovalRule "r"', 'E-NAME-0')
    static('T06C', 'version-case', prior, 'under ApprovalRule "r1"', 'under ApprovalRule "R1"', 'E-NAME-0')
    manual = edit(R, CHANGE, CHANGE+'\nactor Manual { version "m1"; decides strict: bool = true }\nauthority ManualChange { basis "manual procedure"; grants cfo to change Manual }\nconfiguration Old { cut C; process Flow; amend Manual from "m1" to "m0" by cfo under ManualChange { strict = false } }')
    manual = edit(edit(manual, '[ApprovalRule, Approver, Ledger]', '[ApprovalRule, Approver, Ledger, Manual]'), CHANGE, edit(CHANGE, 'not_by [a1]', 'not_by [a1]; under Manual "m1"'))
    static('T06B', 'other-prior', manual, 'under Manual "m1"', 'under Manual "m0"', 'E-CON-0', 'rule change Authority is not in force under the prior rule')
    static('T07', 'no-grant', R, 'by cfo under RuleChange', 'by cfo under ApprovalGrant', 'E-CON-0', 'Authority does not grant change of the rule to the amender')
    renamed = edit(edit(R, 'ApprovalRule', 'Charter'), 'actor Charter {', 'actor Charter { grain document;')
    pair('T08', 'label', R, R, renamed, bad=hand(), oracle=dict(profile='A'), bad_oracle=dict(profile='A', rule_actor='Charter'),
         calculation='Renaming ApprovalRule to Charter and grain document does not change any event/guard/value. Compare semantic rows under explicit name mapping; hash, spans and embedded names legitimately differ. This is equivalence, not an additional fault pair.')
    pair('T08G', 'label-grain', R, 'actor Approver {', 'actor Approver { grain rule;', bad=hand(), oracle=dict(profile='A'), bad_oracle=dict(profile='A'),
         calculation='Approver has no decides; grain rule alone must not create rule metadata. Events and times unchanged3/2min.')
    static('T09', 'cut', R0, 'include [ApprovalRule, Approver, Ledger]', 'include [Approver, Ledger]; exclude ApprovalRule reason "r" loses "l"', 'E-CUT-1', 'condition reads an Actor excluded by the cut', invocation=inv(None))
    static('T09B', 'cut-missing', R, '[ApprovalRule, Approver, Ledger]', '[Approver, Ledger]', 'E-CUT-1', 'declared Actor ApprovalRule is neither included nor excluded by the cut')
    no_read = edit(R, 'ApprovalRule.approval_required == false || approval == pass', 'approval == pass')
    static('T09C', 'cut-amend', no_read, 'include [ApprovalRule, Approver, Ledger]', 'include [Approver, Ledger]; exclude ApprovalRule reason "r" loses "l"', 'E-CUT-1', 'amendment targets an Actor excluded by the cut')
    static('T09D', 'cut-both', R, 'include [ApprovalRule, Approver, Ledger]', 'include [Approver, Ledger]; exclude ApprovalRule reason "r" loses "l"', 'E-CUT-1', count=2,
           calculation='One Cut condition read (qualified Rule span) then one ToBeB amend target span. Two errors, not per-configuration duplication. AsIs selection must still check ToBeB.')
    pair('T10', 'agency', R0, 'version "r1"; decides', 'state', bad=hand(), invocation=inv(None),
         oracle=dict(profile='A'), bad_oracle=dict(mutable=True),
         calculation='R0 omits both configurations and RuleChange. Fixed Rule true derives normative refusal; ActorObserved is mutable even if initially fixed, so Unknown/state_guards/refusal[]; same business times3/2. Do not leave change/amend referring to a non-Rule.')
    pair('T11', 'reverse', R, '', '', expected=dict(verdict='violated'),
         bad=dict(verdict='invalid',diagnostics=[dict(code='E-CON-0',severity='error')]),
         bad_oracle=dict(reverse=True), invocation=inv(command='diff'), bad_invocation=inv(command='diff', from_='ToBeB', to='AsIs'),
         oracle=dict(diff=True, w1=0, w3=1), calculation='Forward has exact r1->r2 amendment. Reverse r2->r1 has none: root E-CON-0, no rule row; do not borrow from-side amendment.')
    reverse_decl = edit(R, 'configuration AsIs { cut C; process Flow }', 'configuration Base { cut C; process Flow }\nconfiguration AsIs { cut C; process Flow; amend ApprovalRule from "r1" to "r0" by cfo under RuleChange { approval_required = true } }')
    pair('T11B', 'reverse-declared', reverse_decl, '', '', expected=dict(verdict='satisfied'),
         bad=dict(verdict='invalid',diagnostics=[dict(code='E-CON-0',severity='error')]), invocation=inv(command='diff', from_='Base', to='AsIs'), bad_invocation=inv(command='diff', from_='ToBeB', to='AsIs'), bad_oracle=dict(reverse=True),
         calculation='Base r1->AsIs r0 declared, both require approval K S. ToBeB r2->AsIs r0 not declared despite to-side amendment r1->r0. No inferred chains.')
    partial = edit(edit(R, 'type Ack =', 'type Mode = enum { strict, open }\ntype Ack ='), RULE, edit(RULE, 'bool = true', 'bool = true; decides mode: Mode = open'))
    partial = edit(partial, 'requires ApprovalRule.approval_required == false || approval == pass else held', 'requires ApprovalRule.mode == open && (ApprovalRule.approval_required == false || approval == pass) else held')
    pair('T12', 'partial', partial, 'mode: Mode = open', 'mode: Mode = strict', expected=hand('B'), bad=hand('Q'), invocation=inv('ToBeB'), oracle=dict(partial='open'), bad_oracle=dict(partial='strict'),
         calculation='Only approval_required amended, mode remains declared open (not first member strict). Open: posted3/work3, K V. Strict: requires false, only held2/work2, zero commits -> K vacuous. All bounds have one declaration.')
    cases = edit(R, 'cases 1; live_tasks 2;', 'cases 2; live_tasks 2;')
    cases = edit(cases, 'verify safety', 'arrivals fixed [0min, 0min]\npolicy fifo tie_by case_id\nverify safety')
    pair('T13', 'cases', cases, 'policy fifo tie_by case_id\n', '', expected=dict(verdict='satisfied', bounds=dict(truncated=False)), bad=dict(verdict='satisfied', bounds=dict(truncated=False)), oracle=dict(cases='fifo'), bad_oracle=dict(cases='nondet'),
         calculation='Four approval combinations. a1 serializes cases: c1 approve0..2, c2 approve2..4. FIFO c1 posted3/held2, c2 posted5/held4; work posted3/held2 for each, waiting2 is not work. Without FIFO either case can be first: posted elapsed3..5, held2..4 for both. No probability invented.')
    issued = edit(R, CHANGE, edit(CHANGE, 'not_by [a1]', 'not_by [a1]; issued_by cfo'))
    pair('T15', 'self-issued', issued, '; issued_by cfo }\nprocess', ' }\nprocess', bad=hand(), oracle=dict(auth_warnings=1), bad_oracle=dict(auth_warnings=0),
         calculation='RuleChange grants cfo to self and issued_by cfo: W-AUTH-0 once in AsIs, although grant unused there; warning does not change satisfied. Removing issuer removes warning, not Authority validity.')
    future = edit(R, CHANGE, CHANGE+'\nauthority Future { basis "f"; grants cfo to change ApprovalRule; under ApprovalRule "r2" }')
    pair('T16', 'enables', future, 'basis "f"; grants cfo', 'basis "f"; grants a1', expected=hand('B'), bad=hand('B'), invocation=inv('ToBeB'), oracle=dict(auth_warnings=1,unused_future='cfo'), bad_oracle=dict(auth_warnings=0,unused_future='a1'),
         calculation='Valid prior RuleChange authorizes amendment. Separate unused Future becomes active r2, grants to amender cfo -> enables warning1, not E-CON. Grant a1 instead: warning0. Unused grant assumption remains both.')
    static('T17', 'repeat', R, AMEND, AMEND+'\n'+AMEND, 'U-SEM-0', 'repeated amendment of one rule is unsupported')
    maintain = edit(R, INTENT, 'intent { change rule ApprovalRule reason "exempt approval"; maintain rule ApprovalRule by cfo }')
    static('T18', 'maintain', maintain, 'maintain rule ApprovalRule', 'override rule ApprovalRule', 'U-SEM-0', 'override of a rule difference is unsupported', invocation=inv(command='diff'), expected=dict(verdict='violated'))
    catalog[-2]['oracle'] = dict(maintain=True)
    mini = '''model Cross version "0.2" {
people [cfo]
actor A { version "r1"; decides x: bool = true }
actor B { version "r1"; decides x: bool = true }
authority GA { basis "a"; grants cfo to change A; under B "r1" }
authority GB { basis "b"; grants cfo to change B }
process P { end done }
cut C { include [A, B] }
configuration AsIs { cut C; process P }
configuration ToBeB { cut C; process P
amend A from "r1" to "r2" by cfo under GA { x = false }
amend B from "r1" to "r2" by cfo under GB { x = false }
}
bounds { cases 1; live_tasks 1; clock 10min }
verify safety no_resource_overcommit
}
'''
    static('T19', 'cross-rule', mini, 'under B "r1"', 'under B "r2"', 'E-CON-0', 'rule change is authorized only under the amended rule', expected=hand('ZERO'), calculation='A and B differ. Both amendments start at declared r1. GA bound to B r2 cannot authorize A merely because it is a different Actor. Proper prior B r1 -> safety S, end0/work0.')
    unrelated = edit(edit(edit(mini, 'amend B from "r1" to "r2" by cfo under GB { x = false }', ''), 'actor B {', 'actor Unrelated { version "r2"; decides x: bool = true }\nactor B {'), 'include [A, B]', 'include [A, B, Unrelated]')
    static('T20', 'qualified-version', unrelated, 'under B "r1"', 'under B "r2"', 'E-NAME-0', expected=hand('ZERO'), calculation='Unrelated.r2 does not declare B.r2. Qualified (Actor,version) lookup fails E-NAME, not E-CON/fallback. Good counterpart end0/work0.')
    reordered = edit(R0, 'version "r1"; decides approval_required: bool = true', 'decides approval_required: bool = true; version "r1"')
    static('T22', 'version-after', reordered, 'decides approval_required: bool = true; ', '', 'U-SYN-0', 'unsupported syntax: version', invocation=inv(None))
    explain = edit(R0, 'approval_required: bool = true', 'approval_required: bool = false')
    pair('T23', 'explain', explain, 'approval_required: bool = false', 'approval_required: bool = true', expected=hand('B'), bad=hand(), invocation=inv(command='explain'), oracle=dict(explain=True), bad_oracle=dict(explain=False),
         calculation='No configuration present and no selector: fail approve at2, post commit at3 violates K; independent replay verified. True refuses fail, K S with no counterexample. Qualified state display ApprovalRule.approval_required.')
    # Every static row is its own physically materialized pair, not a loop that
    # replaces the first failure and never reaches later obligations.
    static('S02', 'default', R0, 'bool = true', 'bool', 'U-SEM-0', 'rule value requires a default', invocation=inv(None))
    static('S03', 'version-missing', R0, 'version "r1"; ', '', 'E-CON-0', 'rule Actor requires version', invocation=inv(None))
    for label, cap, old, new in [
        ('finish', 'capability probe() -> Flag { duration fixed 1min; result nondet { on }; writes scratch; finish { scratch = on } }', 'writes scratch; finish { scratch = on }', 'writes bit; finish { bit = on }'),
        ('start', 'capability probe() -> Flag { duration fixed 1min; result nondet { on }; start { scratch = on } }', 'start { scratch = on }', 'start { bit = on }'),
        ('result', 'capability probe() -> Flag { duration fixed 1min; result nondet { on }; writes scratch }', 'writes scratch', 'writes bit'),
        ('atomic', 'capability probe() -> Flag { writes scratch; duration fixed 1min; atomic { if scratch == on { result off } else { scratch = on; result on } } }', 'writes scratch; duration fixed 1min; atomic { if scratch == on { result off } else { scratch = on;', 'writes bit; duration fixed 1min; atomic { if scratch == on { result off } else { bit = on;'),
    ]:
        s = edit(edit(R0, 'type Ack =', 'type Flag = enum { off, on }\ntype Ack ='), RULE, edit(RULE, 'control policy;', 'control policy; capacity 1; state scratch: Flag = off; decides bit: Flag = off; '+cap+';'))
        static('S04'+label.upper(), 'write-'+label, s, old, new, 'U-SEM-0', 'rule values cannot be written during a run', invocation=inv(None), calculation='Uninvoked one-minute self-owned Flag probe; Rule bit and observed scratch have the same enum type. Only the write target changes (matching writes list follows assignment). Reject in lower before engine guard; no runtime value or time is produced for the bad input.')
    static('S05', 'conditional-amend', R, 'approval_required = false', 'approval_required = approval == pass', 'U-SYN-0', 'unsupported syntax: conditional rule value')
    static('S05D', 'conditional-default', R0, 'bool = true', 'bool = true || false', 'U-SYN-0', 'unsupported syntax: conditional rule value', invocation=inv(None))
    for id, name, old, new, code, msg in [
        ('S07','from','from "r1"','from "r0"','E-CON-0','amendment must start from the declared rule version'),
        ('S08','same-version','to "r2"','to "r1"','E-CON-0','amendment must change the rule version'),
        ('S09','non-rule-amend','amend ApprovalRule','amend Approver','E-CON-0','amend requires a rule Actor'),
        ('S10','non-rule-grant','to change ApprovalRule','to change Approver','E-CON-0','change grant requires a rule Actor'),
        ('S11','unknown-rule','to change ApprovalRule','to change Nobody','E-NAME-0',None),
        ('S12','unknown-value','{ approval_required = false }','{ missing = false }','E-NAME-0',None),
        ('S13','outside-type','{ approval_required = false }','{ approval_required = maybe }','E-TYPE-0',None),
        ('S24','duplicate-assignment','{ approval_required = false }','{ approval_required = false; approval_required = false }','U-SEM-0','duplicate rule assignment'),
    ]:
        static(id, name, R, old, new, code, msg)
    for restriction in ['expires 10min', 'scope approval == pass', 'delegated_from ApprovalGrant']:
        label = restriction.split()[0]
        for unused in [False, True]:
            src = edit(R, CHANGE, CHANGE+'\nauthority Unused { basis "u"; grants cfo to change ApprovalRule }') if unused else R
            target = 'basis "u";' if unused else 'basis "amendment procedure";'
            static('S14'+label.upper()+str(int(unused)), 'restricted-'+label+('-unused' if unused else ''), src, target, target+' '+restriction+';', 'U-SEM-0', 'restricted rule-change Authority is unsupported')
    static('S15', 'bound-capability', R, 'basis "approval table";', 'basis "approval table"; under ApprovalRule "r1";', 'U-SEM-0', 'version-bound Authority with Capability grants is unsupported')
    static('S17', 'per-case', R0, 'control policy;', 'control policy; instance per_case;', 'U-SEM-0', 'per-case rule Actor is unsupported', invocation=inv(None))
    static('S18', 'integer', R0, 'approval_required: bool = true', 'approval_required: 0..2 = 1', 'U-SYN-0', 'unsupported syntax: integer rule value type', invocation=inv(None))
    for ty in ['optional<bool>', 'set<CaseId>', 'PersonId', 'CaseId']:
        static('S19'+ty.split('<')[0].upper(), 'type-'+ty.split('<')[0].lower(), R0, 'approval_required: bool', 'approval_required: '+ty, 'U-SEM-0', 'rule value type is unsupported', invocation=inv(None))
    static('S20', 'non-rule-under', prior, 'under ApprovalRule "r1"', 'under Approver "r1"', 'E-CON-0', 'version binding requires a rule Actor')
    # ActorObserved kernel reads are already unsupported; use constant bool
    # kernel with no given for the substrate, adding just the rule read in fault.
    kernel = edit(R0, RULE, edit(RULE, 'control policy;', 'control policy; capacity 1; capability probe() -> V { duration fixed 1min; outcomes kernel KProbe };'))
    kernel = edit(kernel, 'process Flow', 'kernel KProbe { * -> { 1: pass } }\nprocess Flow')
    static('S21G', 'kernel-given', kernel, 'KProbe { * -> { 1: pass } }', 'KProbe given [ApprovalRule.approval_required] { true -> { 1: pass }; false -> { 1: fail } }', 'U-SEM-0', 'kernel reads of rule values are unsupported', invocation=inv(None))
    static('S21R', 'kernel-reads', kernel, 'outcomes kernel KProbe', 'kernel_reads latent [ApprovalRule.approval_required]; outcomes kernel KProbe', 'U-SEM-0', 'kernel reads of rule values are unsupported', invocation=inv(None))
    static('S21W', 'kernel-with', kernel, '1: pass }', '1: pass with approval_required = true }', 'U-SEM-0', 'rule values cannot be written during a run', invocation=inv(None))
    static('S23', 'grant-seen', R, 'grants cfo to change ApprovalRule; ', '', 'E-SYN-0', 'Authority requires basis and grants')
    static('S25', 'duplicate-state', R0, 'control policy;', 'control policy; state approval_required: bool = true;', 'U-SEM-0', 'duplicate state variable', invocation=inv(None))
    static('S25L', 'duplicate-latent', R0, 'control policy;', 'control policy; latent approval_required: bool = true;', 'U-SEM-0', 'duplicate state variable', invocation=inv(None))
    static('S25V', 'duplicate-version', R0, 'version "r1";', 'version "r1"; version "r1";', 'U-SEM-0', invocation=inv(None))
    static('S25D', 'duplicate-decides', R0, 'decides approval_required: bool = true', 'decides approval_required: bool = true; decides approval_required: bool = true', 'U-SEM-0', 'duplicate state variable', invocation=inv(None))
    static('S26L', 'last-write-wins', R0, 'decides approval_required: bool = true', 'decides approval_required: bool = true last_write_wins', 'U-SEM-0', 'last_write_wins outside shared Actor state is unsupported', invocation=inv(None))
    # Version-only change still requires one assignment, preserves the value.
    version_only = edit(R, 'approval_required = false', 'approval_required = true')
    pair('EDGE-VERSION', 'version-only-diff', version_only, '', '', expected=dict(verdict='satisfied'), bad=dict(verdict='invalid',diagnostics=[dict(code='E-CON-0',severity='error')]), invocation=inv(command='diff'), bad_invocation=inv(command='diff', from_='ToBeB', to='AsIs'), bad_oracle=dict(reverse=True), calculation='Values identical true but versions r1/r2 differ: one rule row. Forward S; reverse requires nonexistent r2->r1 amendment, invalid.')
    # Computational boundary is independent of business end/violation.
    pair('EDGE-CLOCK', 'clock', R, 'clock 20min', 'clock 2min', expected=hand('B'), bad=hand('CUT'), invocation=inv('ToBeB'), calculation='All actual post commits occur at3. Clock2 observes no commit and cuts: inconclusive, no terminal; clock3 (separate core control) includes commit, violated. Never turn unfinished case into success/failure.')
    # Make the preservation vacuity input genuinely different, not a duplicate
    # directory with an implementation fault masquerading as an input edit.
    vacuity = edit(preserve, 'result nondet { pass, fail }', 'result nondet { pass }')
    pair('EDGE-VACUOUS', 'preserve-vacuous', vacuity, 'result nondet { pass }', 'result nondet { fail }', expected=dict(verdict='satisfied'), bad=dict(verdict='violated'), invocation=inv(command='diff'), oracle=dict(preserve='satisfied'), bad_oracle=dict(preserve='violated', from_vacuous=True), calculation='All-pass commits on both sides K S/S. All-fail AsIs refuses all commits K Q, ToBeB commits fail K V. Preservation must keep source Q distinct from S; to V remains V, diff V.')


def write(support_only=False):
    build()
    for r in catalog:
        if r['invocation']['command']!='diff' or r['oracle'].get('error_in_sides'):continue
        n=r['name']
        side=('A','B')
        if n=='rule-preserve.broken-t03':side=('A','A')
        if n=='rule-reverse.broken-t11' or n=='rule-reverse-declared.broken-t11b':side=('B','A')
        if n=='rule-reverse-declared' or n.startswith('rule-version-only-diff'):side=('A','A')
        if n=='rule-preserve-vacuous':side=('PASS','PASS')
        if n=='rule-preserve-vacuous.broken-edge-vacuous':side=('Q','B')
        r['oracle']['side_expected']={key:hand(profile) for key,profile in zip(('from','to'),side)}
    byname = {r['name']: r for r in catalog}
    for p in pairs:
        for name in [p['good'], p['broken']]:
            r = byname[name]
            d = ROOT/('.tmp/pending-rule' if support_only else 'fixtures')/name
            d.mkdir(exist_ok=True)
            (d/'model.relca').write_text(r['source'])
            # The existing fixture contract represents unselected check by
            # *absence* of invocation.json, not malformed {command:check}.
            if r['invocation'] != dict(command='check'):
                (d/'invocation.json').write_text(json.dumps(r['invocation'], indent=2)+'\n')
            (d/'expected.json').write_text(json.dumps(r['expected'], indent=2)+'\n')
            calc = f'''# {name} — {p['id']} (ADR 0044)

Hand oracle, authored before implementation; not captured program output.

{p['calculation']}

## Pair witness

Good: {p['good']}; control: {p['broken']}.
{'Only invocation selection changes; model bytes identical.' if p['invocation_only'] else 'Exactly the transformation stored in support/rule_pairs.json, with assert_ne after substitution.'}
This input expects {r['expected']['verdict']}. All times/work are minutes.
No nondeterminism is assigned a probability. No undefined rate is treated as zero.

## Static/runtime and derivation boundary

{json.dumps(r['oracle'], ensure_ascii=False, indent=2)}

Runtime base arithmetic: pass approve0..2/post2..3 (elapsed/work3), fail rejected
before Start at2 (elapsed/work2). Rule false makes both commit at3; K violated.
Static refusal has no exploration/terminal/number claim. Additional probe
Capabilities are uninvoked. Warnings do not alter the business verdict.
Full rule/Authority/Agency assertions are in rule_actor.rs and rule_actor_cli.rs.
Post-C5-only framing/effects/difference assertions are explicitly deferred,
not claimed exercised on this pre-C5 worktree. RED is not semantic verification.
'''
            (d/'calc.md').write_text(calc)
    (HERE/'rule_names.json').write_text(json.dumps(sorted(byname), indent=2)+'\n')
    (HERE/'rule_pairs.json').write_text(json.dumps(pairs, indent=2)+'\n')
    (HERE/'rule_oracles.json').write_text(json.dumps([{k:v for k,v in r.items() if k!='source'} for r in catalog], indent=2)+'\n')
    (HERE/'rule_cases.rs').write_text('\n'.join(f'#[test]\nfn {r["name"].replace("-","_").replace(".","_")}() {{ fixture({json.dumps(r["name"])}); }}' for r in catalog)+'\n')
    (HERE/'rule_substrates.json').write_text(json.dumps([dict(name=r['name'],source=substrate(r['source'])) for r in catalog if '.broken-' not in r['name']],indent=2)+'\n')
    ordinary=[r for r in catalog if r['invocation']['command']!='diff']
    static=sum(r['expected']['verdict'] in ('invalid','unsupported') for r in ordinary)
    print(f'{len(pairs)} transformations; {len(catalog)} directories; {len(ordinary)-static} runtime / {static} static / {len(catalog)-len(ordinary)} diff selections')


def substrate(src):
    """Erase A6 only, leaving all ordinary capability bodies/types untouched."""
    ts=list(re.finditer(r'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[^\s]',src))
    stack=[]; pairs={}
    for i,t in enumerate(ts):
        if t.group()=='{':stack.append(i)
        elif t.group()=='}':pairs[stack.pop()]=i
    remove=[]
    for i,t in enumerate(ts):
        if t.group() not in ('authority','configuration') or i+2 not in pairs:continue
        end=ts[pairs[i+2]].end()
        block=src[t.start():end]
        if t.group()=='configuration' or ' to change ' in block:remove.append((t.start(),end))
    for a,b in reversed(remove):src=src[:a]+src[b:]
    src=re.sub(r'version "(?:r[0-9]*|m[0-9]*)";?','',src)
    return src.replace('decides ','state ')


if __name__ == '__main__':
    import sys
    write('--support-only' in sys.argv)
