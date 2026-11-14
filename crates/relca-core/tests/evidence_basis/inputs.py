"""ADR 0070 inputs. Edits and expectations come from sets, never binary output."""
from pathlib import Path

DATA = Path(__file__).resolve().parent
M0 = (DATA / 'm0.relca').read_text()
M5 = (DATA / 'm5.relca').read_text()


def edit(source, old, new):
    assert source.count(old) == 1, (old, source.count(old))
    return source.replace(old, new)


def add(source, text):
    i = source.rfind('}')
    return source[:i] + '\n' + text + '\n' + source[i:]


def basis(source, target='Clerk.match_po', sources='PurchaseOrder', direct=False):
    return add(source, f'contract Basis {{ evidence_basis {"direct " if direct else ""}{target} [{sources}] }}')


def remove_basis(source):
    return '\n'.join(line for line in source.splitlines() if not line.startswith('contract Basis {')) + '\n'


RECHECK = edit(M0, 'capability enter()', 'capability recheck() -> V { reads [Invoice]; duration fixed 1min; result nondet { pass } }\n    capability enter()')
FIXED_ARG = edit(edit(M0, 'approve(e:V)', 'approve(e:V,p:V)'), 'Approver.approve(e)', 'Approver.approve(e,p)')
DOMINATING = edit(M5, 'if e == pass { p = Clerk.match_po(e) } else {}', 'p = Clerk.match_po(e)')
LATENT = edit(M0, 'source document', 'source document; latent difficulty: bool = false')
LATENT = edit(LATENT, 'reads [Invoice]; ', '')
LATENT = edit(LATENT, 'approve(e:V) -> V { duration fixed 1min; result nondet { pass }',
              'approve(e:V) -> V { kernel_reads latent [Invoice.difficulty]; duration fixed 1min; outcomes kernel K')
LATENT = add(LATENT, 'kernel K given [Invoice.difficulty] { false -> { 1: pass }; true -> { 1: pass } }')
STATE_GUARD = add(M0, 'observed case { ready: bool = true }')
STATE_GUARD = edit(STATE_GUARD, 'x = Ledger.post(p,a)', 'if ready == true { x = Ledger.post(p,a) } else {}')
REQUIRES = add(M0, 'observed case { po: optional<V> }')
REQUIRES = edit(REQUIRES, 'result nondet { pass, fail }', 'result nondet { pass, fail }; writes po')
REQUIRES = edit(REQUIRES, 'approve(e:V) -> V {', 'approve(e:V) -> V { requires po == pass else fail;')
WAIT = edit(REQUIRES, 'requires po == pass else fail;', '')
WAIT = edit(WAIT, 'a = Approver.approve(e)', 'wait until po == pass; a = Approver.approve(e)')

# Each entry is (input, target, expected verdict). These are set calculations,
# not probabilities: nondeterministic outcomes have no uniform mass.
CASES = {
    'a1-direct-read': (basis(M0), 'Clerk.match_po', 'satisfied'),
    'a1-no-read': (basis(edit(M0, 'reads [PurchaseOrder]; ', '')), 'Clerk.match_po', 'violated'),
    'a2-argument': (basis(M0, sources='Invoice'), 'Clerk.match_po', 'satisfied'),
    'a2-direct-missing': (basis(M0, sources='Invoice', direct=True), 'Clerk.match_po', 'violated'),
    'a3-unused': (basis(RECHECK, 'Clerk.recheck', 'Invoice'), 'Clerk.recheck', 'vacuous'),
    'a3-used-control': (basis(RECHECK, sources='Invoice'), 'Clerk.match_po', 'satisfied'),
    'a4-control-not-data': (basis(M0, 'Approver.approve'), 'Approver.approve', 'violated'),
    'a4-fixed-arg': (basis(FIXED_ARG, 'Approver.approve'), 'Approver.approve', 'satisfied'),
    'a5-dominating': (basis(DOMINATING, 'Approver.approve'), 'Approver.approve', 'satisfied'),
    'f2-requires-not-data': (basis(REQUIRES, 'Approver.approve'), 'Approver.approve', 'violated'),
    'f2-wait-not-data': (basis(WAIT, 'Approver.approve'), 'Approver.approve', 'violated'),
    'f5-latent-not-data': (basis(LATENT, 'Approver.approve', 'Invoice'), 'Approver.approve', 'violated'),
    'f5-direct-control': (basis(edit(LATENT, 'approve(e:V) -> V {', 'approve(e:V) -> V { reads [Invoice];'), 'Approver.approve', 'Invoice'), 'Approver.approve', 'satisfied'),
    'f7-no-lineage-row': (basis(STATE_GUARD, 'Approver.approve'), 'Approver.approve', 'violated'),
    'f7-fixed-arg': (basis(edit(edit(STATE_GUARD, 'approve(e:V)', 'approve(e:V,p:V)'), 'Approver.approve(e)', 'Approver.approve(e,p)'), 'Approver.approve'), 'Approver.approve', 'satisfied'),
    'conjunction': (basis(M0, sources='Invoice, PurchaseOrder'), 'Clerk.match_po', 'satisfied'),
    'conjunction-upstream-removed': (basis(edit(M0, 'reads [Invoice]; ', ''), sources='Invoice, PurchaseOrder'), 'Clerk.match_po', 'violated'),
    'conjunction-downstream-removed': (basis(edit(M0, 'reads [PurchaseOrder]; ', ''), sources='Invoice, PurchaseOrder'), 'Clerk.match_po', 'violated'),
}


def fork_model(join='all', cancellation=False):
    source = edit(M0, 'live_tasks 1', 'live_tasks 2')
    source = edit(source, 'duration fixed 1min; result nondet { pass, fail }',
                  'duration fixed 3min; result nondet { pass, fail }' + ('; cancel immediate' if cancellation else ''))
    source = edit(source, 'p = Clerk.match_po(e)\n    if p == pass {} else { end returned }\n    a = Approver.approve(e)\n    x = Ledger.post(p,a)',
                  f'fork f {{ p = Clerk.match_po(e); a = Approver.approve(e) }}\n    join {join} f' +
                  (' where result == pass tie by [a,p] remaining cancel_then_drain if none -> posted' if cancellation else '') + '\n    ')
    if cancellation:
        source = edit(source, 'approve(e:V) -> V {', 'approve(e:V) -> V { cancel immediate;')
    return source


CASES['fork-all'] = (basis(fork_model(), sources='Invoice'), 'Clerk.match_po', 'satisfied')
CASES['fork-any-cancel'] = (basis(fork_model('any', True)), 'Clerk.match_po', 'vacuous')
CASES['same-capability-distinct-bindings'] = (
    basis(edit(M0, 'a = Approver.approve(e)', 'a = Approver.approve(e); b = Approver.approve(p)'), 'Approver.approve'),
    'Approver.approve', 'violated')


def response_cases():
    for name, suffix in [
        ('delivered', 'response_loss none'),
        ('unknown', 'response_loss sequence [unknown]'),
        ('resend-delivered', 'response_loss sequence [unknown,delivered] retry max_attempts 2 when receipt unknown backoff fixed 1min exhausted end returned'),
        ('result-retry', 'response_loss none retry max_attempts 2 when result [fail] backoff fixed 1min exhausted end returned'),
    ]:
        source = edit(M0, 'p = Clerk.match_po(e)', 'p = Clerk.match_po(e) ' + suffix)
        authority = add(source, 'authority G { basis "test"; grants c1 to Clerk.match_po }\ncontract Witness { authority_basis Clerk.match_po G }')
        yield name, basis(source), authority


def old_witness(name):
    """Manual finite equivalent of the *property*, not an evidence implementation.

    An observed flag is the hand-computed membership result. The existing
    event invariant checks Finish timing/activation and case aggregation only.
    It cannot validate the new implementation's source-set computation.
    """
    source, target, expected = CASES[name]
    source = remove_basis(source)
    good = expected != 'violated'
    flag = f'basis_ok: bool = {str(good).lower()}; '
    if 'observed case {' in source:
        source = edit(source, 'observed case {', 'observed case { ' + flag)
    else:
        source = add(source, 'observed case { ' + flag + ' }')
    return add(source, f'contract Witness {{ invariant {target}.finish -> basis_ok == true }}')


def configured(source):
    start = source.index('  process Main {')
    end = source.index('  bounds {', start)
    second = source[start:end].replace('process Main {', 'process Alternative {', 1)
    return add(source, second + '\nconfiguration A { cut S; process Main }\nconfiguration B { cut S; process Alternative }')


def deterministic(source):
    return source.replace('result nondet { pass, fail }', 'result nondet { pass }')


EMPTY_LOG = '{"kind":"header","schema":"relca-log/v1","clock":"test-clock","time_unit":"min","window":{"from":0,"to":10},"cases":[],"actor_map":[],"executor_map":[]}\n'


def sensitivity_manifest(mode):
    return {'schema': 'relca-sensitivity/v1', 'scope': {'kind': 'enumerated_models', 'rationale': 'evidence command gate'},
            'points': [{'id': 'basis', 'model': 'model.relca', 'configuration': None, 'scenario': None}],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': '1/2' if mode == 'simulation' else None}


def static_cases():
    source = add(LATENT, 'observed case { obs: bool = false }\nlatent case { hidden: bool = false }')
    for name, sources, code, reason in [
        ('empty', '', 'E-CON-0', 'evidence_basis_empty'),
        ('duplicate', 'Invoice, Invoice', 'E-CON-0', 'evidence_basis_duplicate_source'),
        ('unknown', 'Absent', 'E-NAME-0', 'undefined source'),
        ('actor', 'Clerk', 'E-CON-0', 'evidence_basis_not_source'),
        ('actor-state', 'Invoice.difficulty', 'E-CON-0', 'evidence_basis_not_source'),
        ('latent', 'hidden', 'E-CON-0', 'evidence_basis_not_source'),
        ('observed', 'obs', 'E-CON-0', 'evidence_basis_not_source'),
    ]:
        yield name, basis(source, sources=sources), code, reason
    for target in ('Absent.run', 'Clerk.absent'):
        yield target, basis(M0, target), 'E-NAME-0', 'undefined Actor.Capability'


def combination_cases():
    source = edit(M0, 'e = Clerk.enter()', 'repeat attempt at_most 1 { e = Clerk.enter(); leave } on_limit { end returned }')
    yield 'repeat', basis(source), 'loop_feature_combination'
    source = edit(M0, 'a = Approver.approve(e)',
                  'degrade Manual when p == pass { a = Approver.approve(e) } else { end returned }')
    # Avoid the branch-local a escaping into Ledger.post.
    source = edit(source, 'x = Ledger.post(p,a)', '')
    yield 'degrade', basis(source), 'evidence_basis_feature_combination'
    source = '''model Recovery version "0.2" {
type Ack = enum { ok, duplicate }
people [c1]
actor Invoice { source document }
actor Ledger { executors [c1]; state keys: set<CaseId>
  capability post(key:CaseId) -> Ack {
    reads [Invoice]; commits; duration fixed 1min; writes keys; idempotency key key
    atomic { if key in keys { result duplicate } else { keys += key; result ok } }
    cancel forbidden_after_start
  }
}
cut S { include [Invoice,Ledger] }
process Main { x = Ledger.post(case.id) response_loss sequence [unknown] on receipt_unknown { stop receipt_unknown }; end posted }
bounds { cases 1; live_tasks 1; clock 10min }
verify safety no_resource_overcommit
}'''
    yield 'recovery', basis(source, 'Ledger.post', 'Invoice'), 'evidence_basis_feature_combination'


if __name__ == '__main__':
    import json
    # Sources are shared with the Rust entry point, not computed semantics.
    # Both engine and reference must independently meet the hand verdicts.
    print(json.dumps({name: {'source': source, 'expected': expected,
                             'witness': old_witness(name)}
                      for name, (source, _, expected) in CASES.items()}))
