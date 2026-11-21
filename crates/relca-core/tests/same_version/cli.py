"""ADR 0076 test-first acceptance. Observations are logs, never new oracles."""
import argparse
from fractions import Fraction
import gzip
import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import inputs as hand

ROOT = hand.DATA.parents[3]
BINARY = ''
RECORD = None
INJECT_TIME = False
INJECT_WIRE = False
SERIAL = 0
EXITS = dict(satisfied=0, violated=1, inconclusive=2, invalid=3, unsupported=4, vacuous=5)


def invoke(source, command='check', args=(), files=None, manifest=None):
    global SERIAL
    SERIAL += 1
    if INJECT_TIME:
        source = source.replace('duration fixed 1min', 'duration fixed 2min')
    with tempfile.TemporaryDirectory(prefix='same-version-', dir=ROOT / '.tmp') as d:
        root = Path(d)
        (root / 'model.relca').write_text(source)
        for name, text in (files or {}).items():
            (root / name).write_text(text)
        entry = 'model.relca'
        if manifest is not None:
            entry = 'manifest.json'
            (root / entry).write_text(json.dumps(manifest))
        argv = [BINARY, command, str(root / entry), *[
            str(root / arg[1:]) if arg.startswith('@') else arg for arg in args]]
        out = subprocess.run(argv, capture_output=True, text=True, cwd=ROOT)
        if RECORD:
            stem = RECORD / f'{SERIAL:04d}-{command}'
            stem.with_suffix('.relca').write_text(source)
            stem.with_suffix('.stdout.json').write_text(out.stdout)
            stem.with_suffix('.stderr').write_text(out.stderr)
            stem.with_suffix('.exit').write_text(str(out.returncode) + '\n')
        assert out.stdout.endswith('\n') and out.stdout.count('\n') == 1, (argv, out)
        value = json.loads(out.stdout)
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={value.get("verdict")} '
              f'diagnostics={[(d["code"], d["message"]) for d in value.get("diagnostics", [])]}', flush=True)
        if INJECT_WIRE:
            value['verdict'] = 'corrupted_observation'
        return out, value


def verdict(out, value, expected):
    assert out.returncode == EXITS[expected], (expected, out.returncode, value.get('diagnostics'), out.stderr)
    assert value['verdict'] == expected, (expected, value['verdict'])


def property_row(value, id=hand.ID):
    rows = [p for p in value['properties'] if p['id'] == id]
    assert len(rows) == 1, (id, value.get('properties'), value.get('diagnostics'))
    return rows[0]


def contract(value, expected, id=hand.ID, count=None, sampled=False):
    row = property_row(value, id)
    assert row['verdict'] == expected, row
    assert set(row) == ({'id', 'verdict', 'cases'} if count else {'id', 'verdict'}), row
    if count:
        cases = [{'case_id': i, 'arrival': 0, 'verdict': expected} for i in range(1, count + 1)]
        if sampled:
            for case in cases:
                case.update({'basis': 'witness' if expected == 'violated' else 'sample',
                             'witness': {'replication': 1} if expected == 'violated' else None})
        assert row['cases'] == cases, row


def terminals(value, expected):
    rows = value['terminals']
    if not rows and value.get('case_outcomes'):
        assert len(value['case_outcomes']) == 1
        rows = [r for r in value['case_outcomes'][0]['terminals'] if r['kind'] == 'end']
    actual = {r['end']: (r['elapsed']['min'], r['elapsed']['max'], r['work']['min'], r['work']['max']) for r in rows}
    assert actual == expected, (actual, expected)


def diagnostics(value):
    if isinstance(value, dict):
        yield from value.get('diagnostics', [])
        for key, child in value.items():
            if key != 'diagnostics':
                yield from diagnostics(child)
    elif isinstance(value, list):
        for child in value:
            yield from diagnostics(child)


def refuse(source, code, reason, command='check', args=(), manifest=None):
    out, value = invoke(source, command, args, manifest=manifest)
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    assert any(d['code'] == code and reason in d['message'] for d in diagnostics(value)), (code, reason, list(diagnostics(value)))
    assert code in out.stderr and reason in out.stderr, out.stderr
    return value


def traces(value):
    rows = [t for t in value['explanation']['counterexamples'] if t['id'] == hand.ID]
    assert len(rows) == 1, rows
    trace = rows[0]
    assert trace['status'] == trace['replay']['status'] == 'verified', trace
    return trace


def mutant(value, kind, id):
    kinds = [k for k in value['mutations'] if k['kind'] == kind]
    assert len(kinds) == 1, value['mutations']
    rows = [m for m in kinds[0]['mutants'] if m['id'] == id]
    assert len(rows) == 1, kinds[0]
    return rows[0]


class Controls(unittest.TestCase):
    def test_old_explain_verified_finish_prefixes(self):
        for name, time, target in [('a1_unfixed',4,'Builder.assemble'), ('a2_reused',4,'Approver.approve'),
                                   ('a3_no_recheck',4,'Approver.approve'), ('a4_latest_d',5,'Approver.approve')]:
            with self.subTest(name=name):
                out, value = invoke(hand.witness(name), 'explain')
                verdict(out, value, 'violated')
                trace = next(t for t in value['explanation']['counterexamples'] if t['id'] == 'contract Witness #1 invariant')
                assert trace['status'] == trace['replay']['status'] == 'verified', trace
                assert trace['steps'][-1]['time'] == time
                actor, capability = target.split('.')
                assert any(e['kind'] == 'finish' and e['actor'] == actor and e['capability'] == capability
                           for e in trace['steps'][-1]['events'])
                assert 'version' not in trace

    def test_old_simulate_invariant_reports_samples(self):
        for name, expected in [('a2_reuse','inconclusive'), ('a2_reused','violated')]:
            with self.subTest(name=name):
                source = hand.add(hand.deterministic(hand.witness(name)), 'simulation replications 4 seed 7 confidence 0.5')
                out, value = invoke(source, 'simulate')
                verdict(out, value, expected)
                contract(value, expected, 'contract Witness #1 invariant', count=1, sampled=True)
                assert value['case_outcomes'][0]['unfinished_count'] == 0
                assert value['case_outcomes'][0]['terminals'] == [{'kind':'end','end':'done','count':4}]

    def test_old_multi_case_and_clock_inputs_lower(self):
        source = hand.add(hand.witness('a2_reused').replace('cases 1','cases 2'), 'arrivals fixed [0min,0min]')
        out, value = invoke(source)
        verdict(out, value, 'violated')
        contract(value, 'violated', 'contract Witness #1 invariant', count=2)
        out, value = invoke(hand.witness('a2_reuse').replace('clock 10min','clock 2min'))
        verdict(out, value, 'inconclusive')
        contract(value, 'inconclusive', 'contract Witness #1 invariant')
        assert value['bounds']['truncated'] is True

    def test_contest_substrate_preserves_argument_refusal(self):
        out, value = invoke(hand.plain(hand.contest_source()))
        verdict(out, value, 'violated')
        assert any(d['code'] == 'U-SEM-0' and d['message'] == 'contest route requires arguments' for d in value['diagnostics'])
        assert property_row(value, 'contract RouteControl #1 contest')['verdict'] == 'violated'
        assert property_row(value, 'reachable end.done')['verdict'] == 'unsupported'
        assert value['terminals'] == []  # Semantic abort withdraws incomplete extrema.

    def test_command_envelopes_without_new_contract_are_legal(self):
        source = hand.add(hand.configured(hand.deterministic(hand.REUSE)), 'simulation replications 4 seed 7 confidence 0.5')
        for command, args in [('compare', ('--plans','A,B','--mode','exhaustive')),
                              ('compare', ('--plans','A,B','--mode','simulation')),
                              ('diff', ('--from','A','--to','B'))]:
            with self.subTest(command=command, args=args):
                out, value = invoke(source, command, args)
                verdict(out, value, 'satisfied')
        source = hand.add(hand.deterministic(hand.REUSE), 'verify probability reached end.done >= 1\nsimulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive','simulation'):
            with self.subTest(command='sensitivity', mode=mode):
                out, value = invoke(source, 'sensitivity', ('--mode',mode), manifest=hand.manifest(mode))
                verdict(out, value, 'satisfied' if mode == 'exhaustive' else 'inconclusive')
        source = hand.add(hand.REUSE.replace('  verify reachable end.done\n', ''),
            'contract Old { at_most_once Approver.approve.commit per case.id }')
        for scope in ('event-contracts',):
            with self.subTest(command='conform', scope=scope):
                out, value = invoke(source, 'conform', ('--log','@events.jsonl','--scope',scope), {'events.jsonl':hand.EMPTY_LOG})
                verdict(out, value, 'inconclusive')
                assert any(d.get('reason') == 'no_evidence' for d in value['diagnostics'])
        # Envelope-only controls also observe the original hand times.
        out, value = invoke(hand.REUSE)
        verdict(out, value, 'satisfied')
        terminals(value, hand.CASES['a2_reuse'][2])

    def test_fraction_recalculation(self):
        m = Fraction(2 if INJECT_TIME else 1, 1)
        self.assertEqual(5*m, Fraction(5, 1))
        self.assertEqual(3*m, Fraction(3, 1))
        self.assertEqual(m + max(m, m), Fraction(2, 1))
        self.assertEqual(m + m + m, Fraction(3, 1))
        self.assertEqual(Fraction(4, 1) - Fraction(3, 1), m)

    def test_contextual_word_remains_a_legal_old_name(self):
        variants = {
            'binding': hand.REUSE.replace('d = Writer.draft()', 'same_version = Writer.draft()').replace('check(d)', 'check(same_version)').replace('approve(d,k)', 'approve(same_version,k)'),
            'actor': hand.REUSE.replace('Writer', 'same_version'),
            'capability': hand.REUSE.replace('capability draft()', 'capability same_version()').replace('Writer.draft()', 'Writer.same_version()'),
            'parameter': hand.REUSE.replace('check(d:Doc)', 'check(same_version:Doc)'),
            'type': hand.REUSE.replace('Doc', 'same_version'),
            'source': hand.REUSE.replace('Request', 'same_version'),
            'process': hand.REUSE.replace('process Main', 'process same_version'),
            'contract': hand.add(hand.REUSE, 'contract same_version { permitted Approver.approve }'),
            'model': hand.REUSE.replace('model Versions', 'model same_version'),
        }
        for name, source in variants.items():
            with self.subTest(place=name):
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                terminals(value, hand.CASES['a2_reuse'][2])
                assert not any(d['code'].startswith(('U-', 'E-')) for d in value['diagnostics'])

    def test_old_summary_reused_has_no_assigned_check_at_commit(self):
        out, value = invoke(hand.REUSE, 'mutate', ('--kinds', 'summary_reused'))
        verdict(out, value, 'violated')
        m = mutant(value, 'summary_reused', 'summary_reused:Main:body/0:d->p')
        assert (m['result'], m['reason'], m['assigned_changes']) == ('undetected', 'no_assigned_check', []), m
        terminals(value['original'], hand.CASES['a2_reuse'][2])

    def test_existing_snapshot_bytes_all_channels(self):
        # Read existing goldens only. No snapshot is created or updated here.
        spec = importlib.util.spec_from_file_location('version_snapshot_reader', ROOT / 'scripts/snapshots.py')
        assert spec is not None and spec.loader is not None
        snapshots = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(snapshots)
        jobs, _ = snapshots.jobs()
        selected = {f'fixture-{name}' for name in ('serial-time', 'agency-lineage', 'agency-lineage.broken-reused',
                    'agy-vp-merge-scale', 'mut2-summary', 'explain-exec-separation')}
        # Also cover one existing invocation of every public command.
        seen = set()
        for job in jobs:
            if job['args'][0] not in seen:
                selected.add(job['id'])
                seen.add(job['args'][0])
        assert seen == {'check', 'explain', 'simulate', 'compare', 'diff', 'mutate', 'conform', 'sensitivity', 'verify'}, seen
        checked = 0
        for job in jobs:
            if job['id'] not in selected:
                continue
            with self.subTest(job=job['id']):
                paths = [snapshots.SAVED / (job['id'] + ext) for ext in ('.snap', '.snap.gz')]
                paths = [p for p in paths if p.is_file()]
                assert len(paths) == 1, paths
                path = paths[0]
                raw = gzip.decompress(path.read_bytes()) if path.suffix == '.gz' else path.read_bytes()
                expected = snapshots.unpack(raw)
                actual = snapshots.capture(Path(BINARY), job)
                if INJECT_WIRE:
                    actual = (actual[0] + b' ', actual[1], actual[2])
                errors = snapshots.compare(job, expected, actual)
                assert not errors, '\n'.join(errors)
                checked += 1
                print('LEGACY-BYTES', job['id'], flush=True)
        assert checked >= 9, checked


def control_case(name):
    def test(self):
        source, expected, times, _ = hand.CASES[name]
        out, value = invoke(hand.witness(name))
        verdict(out, value, expected)
        contract(value, expected, 'contract Witness #1 invariant')
        terminals(value, times)
        assert value['bounds']['truncated'] is False
    return test


for _name in hand.CASES:
    setattr(Controls, 'test_witness_' + _name, control_case(_name))


def substrate_case(source, recovery=False):
    def test(self):
        out, value = invoke(hand.plain(source))
        verdict(out, value, 'inconclusive' if recovery else 'satisfied')
        assert not any(d['code'].startswith(('E-', 'U-')) for d in value['diagnostics']), value['diagnostics']
        # A separate copy of the one-minute baseline catches time calibration
        # even for response-unknown substrates with no completed end terminal.
        out, value = invoke(hand.REUSE)
        verdict(out, value, 'satisfied')
        terminals(value, hand.CASES['a2_reuse'][2])
    return test


for _name, _source, _, _ in hand.static_cases():
    setattr(Controls, 'test_static_substrate_' + _name, substrate_case(_source))
for _name, _source, _ in hand.combinations():
    setattr(Controls, 'test_combination_substrate_' + _name, substrate_case(_source, _name in ('recovery','resend')))


class Behavior(unittest.TestCase):
    def test_clock_cut_is_inconclusive_not_vacuous(self):
        source = hand.CASES['a2_reuse'][0].replace('clock 10min', 'clock 2min')
        out, value = invoke(source)
        verdict(out, value, 'inconclusive')
        contract(value, 'inconclusive')
        assert value['bounds']['truncated'] is True

    def test_two_cases_keep_individual_contract_verdicts(self):
        source = hand.CASES['a2_reused'][0].replace('cases 1', 'cases 2')
        source = hand.add(source, 'arrivals fixed [0min,0min]')
        out, value = invoke(source)
        verdict(out, value, 'violated')
        contract(value, 'violated', count=2)

    def test_same_capability_different_parameters_is_accepted(self):
        source = hand.add(hand.REUSE, '''actor Pair { capacity 1
          capability use(left:Doc, right:Doc) -> Ack { duration fixed 1min; result nondet { filed } } }''')
        source = source.replace('include [Request,', 'include [Pair,Request,')
        source = source.replace('p = Approver.approve(d,k)', 'p = Pair.use(d,d)')
        source = hand.contract(source, 'same_version Pair.use(left), Pair.use(right)')
        out, value = invoke(source)
        # No earlier Pair.use exists: valid syntax and names, runtime violation.
        verdict(out, value, 'violated')
        contract(value, 'violated')

    def test_scope_does_not_activate_outside_selected_configuration(self):
        source = hand.configured(hand.CASES['a2_reused'][0])
        source = source.replace('contract V {', 'contract V applies_to [B] reason "only B" {')
        out, value = invoke(source, args=('--configuration', 'B'))
        verdict(out, value, 'violated')
        contract(value, 'violated')
        out, value = invoke(source, args=('--configuration', 'A'))
        verdict(out, value, 'satisfied')
        assert not any(p['id'] == hand.ID for p in value['properties'])
        assert value['contract_scope'] == [{'id': hand.ID, 'contract': 'V', 'applies_to': ['B'],
            'reason': 'only B', 'status': 'not_applicable', 'masks_call': True}], value['contract_scope']
        assert out.stderr.count('W-CONF-2') == 1, out.stderr
        assert 'contract.scope.' + hand.ID in value['unchecked']

    def test_simulate_reports_samples_not_universal_safety(self):
        for name, expected in [('a2_reuse', 'inconclusive'), ('a2_reused', 'violated')]:
            with self.subTest(name=name):
                source = hand.add(hand.deterministic(hand.CASES[name][0]), 'simulation replications 4 seed 7 confidence 0.5')
                out, value = invoke(source, 'simulate')
                verdict(out, value, expected)
                contract(value, expected, count=1, sampled=True)
                assert value['simulation']['status'] == 'completed'
                assert value['case_outcomes'][0]['unfinished_count'] == 0
                assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'done', 'count': 4}]

    def test_mutate_assigns_same_version_to_summary_reused(self):
        out, value = invoke(hand.CASES['a2_reuse'][0], 'mutate', ('--kinds', 'summary_reused'))
        # d has exactly two consumers. Only its second consumer is a candidate.
        verdict(out, value, 'satisfied')
        contract(value['original'], 'satisfied')
        m = mutant(value, 'summary_reused', 'summary_reused:Main:body/0:d->p')
        assert len(next(k for k in value['mutations'] if k['kind'] == 'summary_reused')['mutants']) == 1
        contract(m['report'], 'violated')
        assert m['result'] == 'detected', m
        assert any(c['key'] == hand.ID and c['direction'] == 'worsened' for c in m['assigned_changes']), m['assigned_changes']
        assert not any(c['key'] == hand.ID for c in m['other_changes']), m['other_changes']
        assert any('same_version' in a['affects'] for a in value['assumptions'] if a['field'] == 'mutation.summary_scope'), value['assumptions']

    def test_mutate_reads_do_not_change_version_identity(self):
        out, value = invoke(hand.CASES['a2_reuse'][0], 'mutate', ('--kinds', 'evidence_missing'))
        verdict(out, value, 'violated')
        m = mutant(value, 'evidence_missing', 'evidence_missing:Writer.draft:reads:Request')
        contract(value['original'], 'satisfied')
        contract(m['report'], 'satisfied')
        assert m['result'] == 'undetected' and m['reason'] == 'no_worsening', m
        assert out.returncode == 1

    def test_explain_absent_or_refused_A_has_null_examined(self):
        for name, time in [('no_check',2), ('f7_rejected_A',2), ('a5_parallel',2), ('a5_slow_B',3)]:
            with self.subTest(name=name):
                out, value = invoke(hand.CASES[name][0], 'explain')
                verdict(out, value, 'violated')
                trace = traces(value)
                assert trace['version'] == {'binding': 'p', 'capability': 'Approver.approve',
                    'parameter': 'd', 'version': 'd', 'examined': None}, trace['version']
                assert trace['steps'][-1]['time'] == time


def behavior_case(name):
    def test(self):
        source, expected, times, _ = hand.CASES[name]
        out, value = invoke(source)
        verdict(out, value, expected)
        contract(value, expected)
        terminals(value, times)
        assert 'version' not in value
        assert value['bounds']['truncated'] is False
    return test


for _name in hand.CASES:
    setattr(Behavior, 'test_' + _name, behavior_case(_name))


def derivation_case(name):
    def test(self):
        source, expected, _, _ = hand.CASES[name]
        out, base = invoke(hand.plain(source))
        verdict(out, base, 'satisfied')
        assert base['responsibilities'], 'exercise real responsibility rows'
        assert 'via_model_paths' in base
        out, value = invoke(source)
        verdict(out, value, expected)
        for key in ('agency', 'responsibilities', 'via_model_paths'):
            assert json.dumps(base.get(key), separators=(',', ':')) == json.dumps(value.get(key), separators=(',', ':')), key
        warnings = lambda v: [d for d in v['diagnostics'] if d['code'].startswith(('W-RESP', 'W-AGY', 'W-LIN'))]
        assert warnings(base) == warnings(value)
    return test


class Derivation(unittest.TestCase):
    pass


for _name in list(hand.CASES)[:9]:
    setattr(Derivation, 'test_' + _name, derivation_case(_name))


def explain_case(name, binding, cap, version, examined, time):
    def test(self):
        out, value = invoke(hand.CASES[name][0], 'explain')
        verdict(out, value, 'violated')
        trace = traces(value)
        check_binding, check_version = examined
        assert trace['version'] == {'binding': binding, 'capability': cap, 'parameter': 'd', 'version': version,
            'examined': {'binding': check_binding, 'capability': 'Checker.check', 'parameter': 'd', 'version': check_version}}, trace['version']
        assert trace['steps'][-1]['kind'] == 'finish' and trace['steps'][-1]['time'] == time
        assert trace['steps'][-1]['violation']['id'] == hand.ID
        actor, capability = cap.split('.')
        assert any(e['actor'] == actor and e['capability'] == capability and e['kind'] == 'finish'
                   for e in trace['steps'][-1]['events']), trace['steps'][-1]
        if binding == 'a':
            assert not any(e['actor'] == 'Approver' for s in trace['steps'] for e in s['events'])
    return test


class Explain(unittest.TestCase):
    pass


for _args in [
    ('a1_unfixed', 'a', 'Builder.assemble', 'd', ('k','f'), 4),
    ('a2_reused', 'p', 'Approver.approve', 'd_dup', ('k','d'), 4),
    ('a3_no_recheck', 'p', 'Approver.approve', 'f', ('k','d'), 4),
    ('a4_latest_d', 'p', 'Approver.approve', 'd', ('k2','f'), 5),
]:
    setattr(Explain, 'test_' + _args[0], explain_case(*_args))


class Static(unittest.TestCase):
    def test_contest_target_is_unsupported_per_property(self):
        out, value = invoke(hand.contest_source())
        # The pre-existing contest is violated because route arguments are not
        # executable. Preserve that unrelated verdict; only this row is U-SEM.
        verdict(out, value, 'violated')
        assert any(d['code'] == 'U-SEM-0' and d['message'] == 'same_version_contest_route' for d in value['diagnostics'])
        assert 'same_version_contest_route' in out.stderr
        contract(value, 'unsupported', 'contract V #2 same_version')
        assert property_row(value, 'contract RouteControl #1 contest')['verdict'] == 'violated'
        assert property_row(value, 'reachable end.done')['verdict'] == 'unsupported'


def static_test(source, code, reason):
    def test(self):
        refuse(source, code, reason)
    return test


for _name, _source, _code, _reason in hand.static_cases():
    setattr(Static, 'test_' + _name, static_test(_source, _code, _reason))
for _name, _source, _reason in hand.combinations():
    if _name != 'repeat':  # Still a refusal for mutate, not for check since ADR 0080 R5.
        setattr(Static, 'test_combination_' + _name, static_test(_source, 'U-SEM-0', _reason))


class Gates(unittest.TestCase):
    def test_repeat_reuses_the_executed_draft_in_check(self):
        source = next(s for name, s, _ in hand.combinations() if name == 'repeat')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert property_row(value)['verdict'] == 'satisfied'

    def test_compare(self):
        source = hand.add(hand.configured(hand.deterministic(hand.CASES['a2_reuse'][0])), 'simulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                out, value = invoke(source, 'compare', ('--plans', 'A,B', '--mode', mode))
                expected = 'satisfied' if mode == 'exhaustive' else 'inconclusive'
                verdict(out, value, expected)
                row, = (r for r in value['tables']['properties'] if r['id'] == hand.ID)
                assert [c['verdict'] for c in row['values']] == [expected, expected], row
                assert 'contract_witnesses' in value['unchecked']

    def test_diff(self):
        out, value = invoke(hand.configured(hand.CASES['a2_reuse'][0]), 'diff', ('--from', 'A', '--to', 'B'))
        verdict(out, value, 'satisfied')
        row = property_row(value)
        assert row['from'] == row['to'] == 'satisfied', row
        assert 'contract_witnesses' in value['unchecked']

    def test_sensitivity(self):
        source = hand.add(hand.deterministic(hand.CASES['a2_reuse'][0]), 'verify probability reached end.done >= 1\nsimulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                out, value = invoke(source, 'sensitivity', ('--mode', mode), manifest=hand.manifest(mode))
                expected = 'satisfied' if mode == 'exhaustive' else 'inconclusive'
                verdict(out, value, expected)
                assert property_row(value['points'][0]['report'])['verdict'] == expected

    def test_conform_event_contracts_is_explicitly_unchecked(self):
        source = hand.CASES['a2_reuse'][0].replace('  verify reachable end.done\n', '')
        for scope in ('event-contracts',):
            with self.subTest(scope=scope):
                out, value = invoke(source, 'conform', ('--log', '@events.jsonl', '--scope', scope), {'events.jsonl': hand.EMPTY_LOG})
                verdict(out, value, 'unsupported')
                assert any(d['code'] == 'U-LOG-1' and d.get('reason') == 'unsupported_contract' for d in value['diagnostics']), value['diagnostics']
                assert 'U-LOG-1' in out.stderr
                assert 'same_version' in value['unchecked']
                assert any(r['item'] == hand.ID and r['reason'] == 'unsupported_request' for r in value['not_checked']), value['not_checked']

    def test_verify_keeps_external_backend_boundary(self):
        out, value = invoke(hand.CASES['a2_reuse'][0], 'verify')
        verdict(out, value, 'unsupported')
        assert 'external backend is not connected' in out.stderr
        backend = value['ledger']['backends']['external']
        assert backend['status'] == 'not_connected' and backend['reason'] == 'external_backend_not_connected'

    def test_mutate_keeps_merge_boundary_and_tests_original_repeat_site(self):
        refuse(hand.CASES['a3_recheck'][0], 'U-SEM-0', 'merge_feature_combination', 'mutate', ('--kinds', 'summary_reused'))
        source = next(s for name, s, _ in hand.combinations() if name == 'repeat')
        out, value = invoke(source, 'mutate', ('--kinds', 'summary_reused'))
        verdict(out, value, 'satisfied')
        row, = (r for r in value['mutations'] if r['kind'] == 'summary_reused')
        mutant, = row['mutants']
        assert mutant['site']['statement_path'] == 'body/0/repeat/0'
        assert mutant['result'] == 'detected'


class RecordedResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.records = []

    def addSuccess(self, test):
        super().addSuccess(test)
        self.records.append({'test': test.id(), 'status': 'GREEN'})

    def addFailure(self, test, err):
        super().addFailure(test, err)
        self.records.append({'test': test.id(), 'status': 'RED', 'failure': str(err[1])})

    def addError(self, test, err):
        super().addError(test, err)
        self.records.append({'test': test.id(), 'status': 'ERROR', 'failure': str(err[1])})

    def addSubTest(self, test, subtest, err):
        super().addSubTest(test, subtest, err)
        self.records.append({'test': test.id(), 'subtest': str(subtest), 'status': 'RED' if err else 'GREEN',
                             **({'failure': str(err[1])} if err else {})})

    def stopTestRun(self):
        super().stopTestRun()
        if RECORD:
            (RECORD / 'summary.json').write_text(json.dumps(self.records, indent=2) + '\n')


if __name__ == '__main__':
    p = argparse.ArgumentParser()
    p.add_argument('--binary', required=True)
    p.add_argument('--record-dir', type=Path)
    p.add_argument('--inject-time-fault', action='store_true')
    p.add_argument('--inject-wire-fault', action='store_true')
    args, rest = p.parse_known_args()
    BINARY = str(Path(args.binary).resolve())
    RECORD = args.record_dir
    if RECORD:
        assert RECORD.resolve().is_relative_to(ROOT.resolve())
        RECORD.mkdir(parents=True, exist_ok=True)
    INJECT_TIME = args.inject_time_fault
    INJECT_WIRE = args.inject_wire_fault
    (ROOT / '.tmp').mkdir(exist_ok=True)
    unittest.main(argv=[__file__, *rest], testRunner=unittest.TextTestRunner(resultclass=RecordedResult, verbosity=2))
