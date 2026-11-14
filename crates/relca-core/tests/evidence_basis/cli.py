"""ADR 0070 stage-1 acceptance; run directly or through the Rust CLI test.

No current-binary output is an oracle. --record-dir records observations only.
Controls use handwritten old DSL; Behavior always uses the real new syntax.
"""
import argparse
from fractions import Fraction
import json
from pathlib import Path
import subprocess
import tempfile
import unittest

import inputs as hand

ROOT = hand.DATA.parents[3]
BINARY = ''
RECORD = None
INJECT = False
INJECT_VERDICT = False
SERIAL = 0
EXITS = dict(satisfied=0, violated=1, inconclusive=2, invalid=3, unsupported=4, vacuous=5)
BASIS_ID = 'contract Basis #1 evidence_basis'


def invoke(source, command='check', args=(), files=None, manifest=None):
    global SERIAL
    SERIAL += 1
    if INJECT:
        # Temporary input fault for GREEN calibration, never production edits.
        source = source.replace('duration fixed 1min', 'duration fixed 2min')
    with tempfile.TemporaryDirectory(prefix='evidence-basis-', dir=ROOT / '.tmp') as d:
        root = Path(d)
        (root / 'model.relca').write_text(source)
        for name, content in (files or {}).items():
            (root / name).write_text(content)
        entry = 'model.relca'
        if manifest is not None:
            entry = 'manifest.json'
            (root / entry).write_text(json.dumps(manifest))
        argv = [BINARY, command, str(root / entry), *[
            str(root / x[1:]) if x.startswith('@') else x for x in args]]
        out = subprocess.run(argv, capture_output=True, text=True, cwd=ROOT)
        if RECORD:
            prefix = RECORD / f'{SERIAL:04d}-{command}'
            prefix.with_suffix('.relca').write_text(source)
            prefix.with_suffix('.stdout.json').write_text(out.stdout)
            prefix.with_suffix('.stderr').write_text(out.stderr)
            prefix.with_suffix('.exit').write_text(str(out.returncode) + '\n')
        assert out.stdout.endswith('\n') and out.stdout.count('\n') == 1, (argv, out)
        value = json.loads(out.stdout)
        if INJECT_VERDICT:
            # A one-field wire corruption for an entry point that never reads
            # the DSL (verify). Raw recorded stdout remains the real output.
            value['verdict'] = 'satisfied'
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={value.get("verdict")} '
              f'diagnostics={[(d["code"], d["message"]) for d in value.get("diagnostics", [])]}', flush=True)
        return out, value


def verdict(out, value, expected):
    assert out.returncode == EXITS[expected], (expected, out.returncode, value.get('diagnostics'), out.stderr)
    assert value['verdict'] == expected, (expected, value['verdict'])


def property_row(value, id=BASIS_ID):
    rows = [p for p in value['properties'] if p['id'] == id]
    assert len(rows) == 1, (id, value.get('properties'), value.get('diagnostics'))
    return rows[0]


def contract(value, expected, id=BASIS_ID, case_count=None):
    row = property_row(value, id)
    assert row['verdict'] == expected, row
    # This is the existing contract wire shape, not a new evidence field in check.
    if case_count is None:
        assert set(row) == {'id', 'verdict'}, row
        return
    assert set(row) == {'id', 'verdict', 'cases'}, row
    assert len(row['cases']) == case_count, row
    assert {r['case_id'] for r in row['cases']} == set(range(1, case_count + 1)), row
    assert all(r['verdict'] == expected for r in row['cases']), row


def refuse(source, code, reason, command='check', args=(), files=None, manifest=None):
    out, value = invoke(source, command, args, files, manifest)
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    # Commands can own diagnostics on their ordinary nested reports.
    def diagnostics(v):
        if isinstance(v, dict):
            yield from v.get('diagnostics', [])
            for k, child in v.items():
                if k != 'diagnostics':
                    yield from diagnostics(child)
        elif isinstance(v, list):
            for child in v:
                yield from diagnostics(child)
    observed = list(diagnostics(value))
    matches = [d for d in observed if d['code'] == code and reason in d['message']]
    assert matches, (code, reason, observed)
    assert code in out.stderr and reason in out.stderr, out.stderr
    return value


def terminals(value, expected):
    rows = value['terminals']
    if not rows and value.get('case_outcomes'):
        assert len(value['case_outcomes']) == 1
        rows = [r for r in value['case_outcomes'][0]['terminals'] if r['kind'] == 'end']
    got = {t['end']: (t['elapsed']['min'], t['elapsed']['max'], t['work']['min'], t['work']['max']) for t in rows}
    assert got == expected, (got, expected)


def ordinary_m0(value):
    terminals(value, {'posted': (4, 4, 4, 4), 'returned': (2, 2, 2, 2)})


def mutation_kind(value, name):
    rows = [k for k in value['mutations'] if k['kind'] == name]
    assert len(rows) == 1, name
    return rows[0]


class Controls(unittest.TestCase):
    def test_old_m0_hand_times_and_terminal_inventory(self):
        out, value = invoke(hand.M0)
        verdict(out, value, 'satisfied')
        ordinary_m0(value)
        assert property_row(value, 'reachable end.posted')['verdict'] == 'satisfied'
        assert value['bounds']['truncated'] is False

    def test_old_m5_hand_times_and_dominating_control(self):
        for source, times in [(hand.M5, (3, 4, 3, 4)), (hand.DOMINATING, (4, 4, 4, 4))]:
            with self.subTest(dominating=source == hand.DOMINATING):
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                terminals(value, {'posted': times})

    def test_fraction_recalculation_is_not_a_probability(self):
        # All durations in this ADR have denominator 1. There is no probability
        # measure on the two nondeterministic branches.
        minute = Fraction(2 if INJECT else 1, 1)
        self.assertEqual(minute + minute, Fraction(2, 1))
        self.assertEqual(4 * minute, Fraction(4, 1))
        self.assertEqual(Fraction(1, 2) + Fraction(1, 2), 1)  # count ratio only
        self.assertEqual(Fraction(2, 2) - Fraction(1, 2), Fraction(1, 2))

    def test_old_context_words_as_names(self):
        for word in ('evidence_basis', 'direct'):
            for place in ('binding', 'actor', 'capability', 'source', 'contract'):
                with self.subTest(word=word, place=place):
                    source = hand.M0
                    if place == 'binding':
                        source = source.replace('e = Clerk.enter()', f'{word} = Clerk.enter()').replace('match_po(e)', f'match_po({word})').replace('Approver.approve(e)', f'Approver.approve({word})')
                    elif place == 'actor':
                        source = source.replace('Clerk', word)
                    elif place == 'capability':
                        source = source.replace('match_po', word)
                    elif place == 'source':
                        source = source.replace('Invoice', word)
                    else:
                        source = hand.add(source, f'contract {word} {{ permitted Clerk.match_po }}')
                    out, value = invoke(source)
                    verdict(out, value, 'satisfied')
                    ordinary_m0(value)

    def test_old_parameter_reads_still_have_no_ir_effect(self):
        out, base = invoke(hand.M0)
        verdict(out, base, 'satisfied')
        source = hand.edit(hand.M0, 'reads [PurchaseOrder]', 'reads [e,PurchaseOrder]')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        ordinary_m0(value)
        for key in ('agency', 'properties', 'responsibilities'):
            assert value.get(key) == base.get(key), key

    def test_old_mutation_assignment_is_preserved(self):
        for kind in ('evidence_missing', 'original_access_removed'):
            with self.subTest(kind=kind):
                out, value = invoke(hand.M0, 'mutate', ('--kinds', kind))
                # A surviving assigned mutation is violated, not a build failure.
                verdict(out, value, 'violated')
                ordinary_m0(value['original'])
                mutants = mutation_kind(value, kind)['mutants']
                assert len(mutants) == 2
                by_site = {f'{m["site"]["actor"]}.{m["site"]["capability"]}:reads:{m["site"]["source"]}': m for m in mutants}
                assert set(by_site) == {'Clerk.enter:reads:Invoice', 'Clerk.match_po:reads:PurchaseOrder'}
                if kind == 'evidence_missing':
                    assert by_site['Clerk.enter:reads:Invoice']['result'] == 'undetected'
                    assert by_site['Clerk.enter:reads:Invoice']['reason'] == 'no_worsening'
                    assert by_site['Clerk.match_po:reads:PurchaseOrder']['result'] == 'detected'
                else:
                    assert all(m['result'] == 'undetected' and m['reason'] == 'no_assigned_check' for m in mutants)

    def test_response_authority_substrate(self):
        for name, _, source in hand.response_cases():
            with self.subTest(name=name):
                out, value = invoke(source)
                verdict(out, value, 'inconclusive' if name in ('unknown', 'resend-delivered') else 'satisfied')
                row = property_row(value, 'contract Witness #1 authority_basis')
                assert row['verdict'] == 'satisfied', row
                response_times(name, value)

    def test_combination_substrates_parse_and_lower(self):
        for name, source, _ in hand.combination_cases():
            with self.subTest(name=name):
                out, value = invoke(hand.remove_basis(source))
                verdict(out, value, 'inconclusive' if name == 'recovery' else 'satisfied')
                assert not any(d['code'].startswith(('E-', 'U-')) for d in value['diagnostics'])
                if name == 'recovery':
                    assert value['case_outcomes'][0]['terminals'][0]['kind'] == 'receipt_unknown'
                    assert value['case_outcomes'][0]['terminals'][0]['observed_elapsed'] == {'min': 1, 'max': 1}
                else:
                    # enter duration is one minute even in the repeat/degrade
                    # substrates. This assertion catches the calibration edit.
                    ordinary_m0(value) if name == 'repeat' else terminals(value, {'posted': (3, 3, 3, 3), 'returned': (2, 2, 2, 2)})

    def test_static_body_substrates_are_legal_before_new_clause(self):
        for name, source, _, _ in hand.static_cases():
            with self.subTest(name=name):
                out, value = invoke(hand.remove_basis(source))
                verdict(out, value, 'satisfied')
                ordinary_m0(value)

    def test_old_explain_finish_prefix_and_state_guard_empty_lineage(self):
        for name, finish in [('a1-no-read', 2), ('a4-control-not-data', 3), ('f7-no-lineage-row', 3)]:
            with self.subTest(name=name):
                out, value = invoke(hand.old_witness(name), 'explain')
                verdict(out, value, 'violated')
                trace = next(t for t in value['explanation']['counterexamples'] if t['id'] == 'contract Witness #1 invariant')
                assert trace['replay']['status'] == 'verified', trace
                events = [(s['time'], e['actor'], e['capability']) for s in trace['steps'] for e in s['events'] if e['kind'] == 'finish']
                expected = [(1, 'Clerk', 'enter'), (2, 'Clerk', 'match_po')]
                if finish == 3:
                    expected.append((3, 'Approver', 'approve'))
                assert events == expected, events
                assert trace['steps'][-1]['time'] == finish
                if name == 'f7-no-lineage-row':
                    assert value['agency']['lineage'] == [], value['agency']['lineage']

    def test_old_simulate_contract_sample_boundary(self):
        for name, expected in [('a1-direct-read', 'inconclusive'), ('a1-no-read', 'violated')]:
            with self.subTest(name=name):
                source = hand.add(hand.deterministic(hand.old_witness(name)), 'simulation replications 4 seed 7 confidence 0.5')
                out, value = invoke(source, 'simulate')
                verdict(out, value, expected)
                assert property_row(value, 'contract Witness #1 invariant')['verdict'] == expected
                assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}]
                # A separate exhaustive substrate pins the hand time, so the
                # same temporary duration fault cannot escape this control.
                out, check = invoke(hand.deterministic(hand.remove_basis(hand.CASES[name][0])))
                verdict(out, check, 'satisfied')
                terminals(check, {'posted': (4, 4, 4, 4)})

    def test_old_command_envelope_inputs_are_valid(self):
        out, base = invoke(hand.M0)
        verdict(out, base, 'satisfied')
        ordinary_m0(base)
        source = hand.add(hand.configured(hand.deterministic(hand.M0)), 'simulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(command='compare', mode=mode):
                out, value = invoke(source, 'compare', ('--plans', 'A,B', '--mode', mode))
                verdict(out, value, 'satisfied')
        out, value = invoke(source, 'diff', ('--from', 'A', '--to', 'B'))
        verdict(out, value, 'satisfied')
        source = hand.add(hand.deterministic(hand.M0), 'verify probability reached end.posted >= 1\nsimulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(command='sensitivity', mode=mode):
                out, value = invoke(source, 'sensitivity', ('--mode', mode), manifest=hand.sensitivity_manifest(mode))
                verdict(out, value, 'satisfied' if mode == 'exhaustive' else 'inconclusive')
        conform_source = hand.add(hand.M0.replace('  verify reachable end.posted\n', ''),
                                  'contract Legacy { at_most_once Ledger.post.commit per case.id }')
        out, value = invoke(conform_source, 'conform', ('--log', '@events.jsonl', '--scope', 'event-contracts'), {'events.jsonl': hand.EMPTY_LOG})
        verdict(out, value, 'inconclusive')
        assert any(d['code'] == 'W-LOG-2' and d.get('reason') == 'no_evidence' for d in value['diagnostics']), value['diagnostics']


def witness_test(name):
    def test(self):
        out, value = invoke(hand.old_witness(name))
        verdict(out, value, hand.CASES[name][2])
        contract(value, hand.CASES[name][2], 'contract Witness #1 invariant', case_count=1 if name == 'f2-wait-not-data' else None)
        witness_times(name, value)
    return test


for _name in hand.CASES:
    setattr(Controls, 'test_witness_' + _name.replace('-', '_'), witness_test(_name))


def witness_times(name, value):
    if name == 'a5-dominating':
        terminals(value, {'posted': (4, 4, 4, 4)})
    elif name == 'fork-all':
        terminals(value, {'posted': (4, 4, 5, 5)})
    elif name == 'fork-any-cancel':
        terminals(value, {'posted': (2, 2, 3, 3)})
    elif name == 'f2-wait-not-data':
        terminals(value, {'posted': (4, 4, 4, 4), 'returned': (2, 2, 2, 2)})
    elif name == 'same-capability-distinct-bindings':
        terminals(value, {'posted': (5, 5, 5, 5), 'returned': (2, 2, 2, 2)})
    else:
        ordinary_m0(value)


def response_times(name, value):
    rows = value['case_outcomes'][0]['terminals']
    if name in ('unknown', 'resend-delivered'):
        assert len(rows) == 1 and rows[0]['kind'] == 'receipt_unknown', rows
        end = 2 if name == 'unknown' else 4
        assert rows[0]['elapsed'] is None and rows[0]['completed_at'] is None
        assert rows[0]['observed_elapsed'] == {'min': end, 'max': end}, rows
    else:
        got = {r['end']: (r['elapsed']['min'], r['elapsed']['max'], r['work']['min'], r['work']['max']) for r in rows}
        expected = {'posted': (4, 4, 4, 4), 'returned': (2, 2, 2, 2)} if name == 'delivered' else (
            {'posted': (6, 6, 5, 5), 'returned': (4, 4, 3, 3)} if name == 'resend-delivered' else
            {'posted': (4, 6, 4, 5), 'returned': (4, 4, 3, 3)})
        assert got == expected, (got, expected)


class Behavior(unittest.TestCase):
    def test_response_loss_and_finite_retry_match_authority_scope(self):
        for name, source, _ in hand.response_cases():
            with self.subTest(name=name):
                out, value = invoke(source)
                verdict(out, value, 'inconclusive' if name in ('unknown', 'resend-delivered') else 'satisfied')
                contract(value, 'satisfied', case_count=1)
                response_times(name, value)

    def test_snapshot_definite_violation_takes_precedence(self):
        source = hand.edit(hand.M5, 'actor PurchaseOrder', 'actor Missing { source record }\n  actor PurchaseOrder')
        source = hand.edit(source, 'include [PurchaseOrder', 'include [Missing,PurchaseOrder')
        out, value = invoke(hand.basis(source, 'Approver.approve', 'PurchaseOrder,Missing'))
        verdict(out, value, 'violated')
        contract(value, 'violated')

    def test_snapshot_direct_alternative_does_not_need_conditional_edge(self):
        source = hand.edit(hand.M5, 'reads [po]', 'reads [po,PurchaseOrder]')
        out, value = invoke(hand.basis(source, 'Approver.approve'))
        verdict(out, value, 'satisfied')
        contract(value, 'satisfied')

    def test_clock_does_not_turn_unfinished_call_into_success_or_vacuity(self):
        source = hand.edit(hand.CASES['a1-direct-read'][0], 'clock 10min', 'clock 1min')
        out, value = invoke(source)
        verdict(out, value, 'inconclusive')
        assert property_row(value)['verdict'] == 'inconclusive'
        assert value['bounds']['truncated'] is True

    def test_a5_conditional_snapshot_is_unsupported_not_may_satisfied(self):
        refuse(hand.basis(hand.M5, 'Approver.approve'), 'U-SEM-0', 'evidence_basis_conditional_snapshot')

    def test_a3_vacuous_mutate_stops_before_candidates(self):
        source = hand.CASES['a3-unused'][0]
        out, value = invoke(source, 'mutate', ('--kinds', 'evidence_missing'))
        verdict(out, value, 'vacuous')
        contract(value['original'], 'vacuous')
        assert not any(k['mutants'] for k in value['mutations']), value['mutations']

    def test_check_no_evidence_field_and_explain_verified_finish_prefix(self):
        source = hand.CASES['a1-no-read'][0]
        out, check = invoke(source)
        verdict(out, check, 'violated')
        contract(check, 'violated')
        assert 'evidence' not in check
        out, value = invoke(source, 'explain')
        verdict(out, value, 'violated')
        traces = [t for t in value['explanation']['counterexamples'] if t['id'] == BASIS_ID]
        assert len(traces) == 1, traces
        trace = traces[0]
        assert trace['status'] == 'verified' and trace['replay']['status'] == 'verified', trace
        evidence = trace['evidence']
        assert set(evidence) == {'binding', 'actor', 'capability', 'required', 'sources', 'missing', 'excluded_edges'}
        assert evidence['binding'] == 'p'
        assert evidence['actor'] == 'Clerk'
        assert evidence['capability'] == 'match_po'
        assert set(evidence['required']) == {'PurchaseOrder'}
        assert set(evidence['sources']) == {'Invoice'}
        assert set(evidence['missing']) == {'PurchaseOrder'}
        assert evidence['excluded_edges'] == []
        for key in ('required', 'sources', 'missing', 'excluded_edges'):
            assert evidence[key] == sorted(evidence[key]), key
        finishes = [(s['time'], e['actor'], e['capability']) for s in trace['steps'] for e in s['events'] if e['kind'] == 'finish']
        assert finishes == [(1, 'Clerk', 'enter'), (2, 'Clerk', 'match_po')], finishes
        assert trace['steps'][-1]['kind'] == 'finish' and trace['steps'][-1]['time'] == 2
        assert trace['steps'][-1]['violation']['id'] == BASIS_ID

    def test_guard_counterexample_pass_before_approve_at_three(self):
        out, value = invoke(hand.CASES['a4-control-not-data'][0], 'explain')
        verdict(out, value, 'violated')
        trace = next(t for t in value['explanation']['counterexamples'] if t['id'] == BASIS_ID)
        assert trace['replay']['status'] == 'verified'
        events = [(s['time'], e['actor'], e['capability'], e['result']) for s in trace['steps'] for e in s['events'] if e['kind'] == 'finish']
        assert events == [(1, 'Clerk', 'enter', 'pass'), (2, 'Clerk', 'match_po', 'pass'), (3, 'Approver', 'approve', 'pass')], events
        assert set(trace['evidence']['missing']) == {'PurchaseOrder'}

    def test_f6_contract_never_feeds_derivation_or_responsibilities(self):
        for name in ('conjunction', 'conjunction-upstream-removed', 'f7-no-lineage-row'):
            with self.subTest(name=name):
                source, _, expected = hand.CASES[name]
                source = hand.add(source, 'responsible a1 for Ledger.maintenance')
                out, base = invoke(hand.remove_basis(source))
                verdict(out, base, 'satisfied')
                assert base['responsibilities'], 'exercise a real responsibility, not an absent table'
                out, value = invoke(source)
                verdict(out, value, expected)
                for key in ('agency', 'responsibilities'):
                    # Compare serialized projections byte-for-byte, including
                    # origin, lineage_groups, viewpoints and unchecked marks.
                    assert json.dumps(base.get(key), separators=(',', ':')) == json.dumps(value.get(key), separators=(',', ':')), key
                warnings = lambda v: [d for d in v['diagnostics'] if d['code'] == 'W-LIN-2']
                assert warnings(base) == warnings(value)
                if name == 'f7-no-lineage-row':
                    assert base['agency']['lineage'] == [], base['agency']['lineage']

    def test_mutate_conjunction_kills_both_reads(self):
        out, value = invoke(hand.CASES['conjunction'][0], 'mutate', ('--kinds', 'evidence_missing'))
        verdict(out, value, 'satisfied')
        contract(value['original'], 'satisfied')
        mutants = mutation_kind(value, 'evidence_missing')['mutants']
        assert len(mutants) == 2
        assert {m['id'] for m in mutants} == {'evidence_missing:Clerk.enter:reads:Invoice', 'evidence_missing:Clerk.match_po:reads:PurchaseOrder'}
        for m in mutants:
            assert m['result'] == 'detected', m
            contract(m['report'], 'violated')
            assert any(c['key'] == BASIS_ID and c['direction'] == 'worsened' for c in m['assigned_changes']), m['assigned_changes']

    def test_u7_direct_only_original_access_assignment(self):
        source = hand.basis(hand.M0, direct=True)
        out, value = invoke(source, 'mutate', ('--kinds', 'original_access_removed'))
        verdict(out, value, 'violated')
        mutants = {f'{m["site"]["actor"]}.{m["site"]["capability"]}:reads:{m["site"]["source"]}': m for m in mutation_kind(value, 'original_access_removed')['mutants']}
        assert len(mutants) == 2
        upstream = mutants['Clerk.enter:reads:Invoice']
        downstream = mutants['Clerk.match_po:reads:PurchaseOrder']
        assert upstream['result'] == 'undetected' and upstream['reason'] == 'no_worsening', upstream
        contract(upstream['report'], 'satisfied')
        assert downstream['result'] == 'detected', downstream
        contract(downstream['report'], 'violated')
        out, plain = invoke(hand.basis(hand.M0), 'mutate', ('--kinds', 'original_access_removed'))
        verdict(out, plain, 'violated')
        assert all(m['result'] == 'undetected' and m['reason'] == 'no_assigned_check' for m in mutation_kind(plain, 'original_access_removed')['mutants'])

    def test_two_cases_keep_individual_verdicts(self):
        source = hand.CASES['a1-no-read'][0]
        source = hand.edit(source, 'cases 1', 'cases 2')
        source = hand.add(source, 'arrivals fixed [0min,0min]')
        out, value = invoke(source)
        verdict(out, value, 'violated')
        contract(value, 'violated', case_count=2)

    def test_source_names_and_metadata_do_not_supply_meaning(self):
        source = hand.CASES['a4-control-not-data'][0]
        source = source.replace('PurchaseOrder', 'judge').replace('Invoice', 'observer').replace('source record', 'source tacit').replace('source document', 'source record')
        out, value = invoke(source)
        verdict(out, value, 'violated')
        contract(value, 'violated')

    def test_simulation_does_not_claim_universal_contract_safety(self):
        for name, expected in [('a1-direct-read', 'inconclusive'), ('a1-no-read', 'violated')]:
            with self.subTest(name=name):
                source = hand.add(hand.deterministic(hand.CASES[name][0]), 'simulation replications 4 seed 7 confidence 0.5')
                out, value = invoke(source, 'simulate')
                verdict(out, value, expected)
                assert property_row(value)['verdict'] == expected
                assert value['simulation']['status'] == 'completed'
                assert value['case_outcomes'][0]['unfinished_count'] == 0
                assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}]


def behavior_test(name):
    def test(self):
        source, _, expected = hand.CASES[name]
        out, value = invoke(source)
        verdict(out, value, expected)
        contract(value, expected, case_count=1 if name == 'f2-wait-not-data' else None)
        if name.startswith(('a1-', 'a2-', 'a3-', 'a4-', 'conjunction')):
            ordinary_m0(value)
    return test


for _name in hand.CASES:
    setattr(Behavior, 'test_' + _name.replace('-', '_'), behavior_test(_name))


class Static(unittest.TestCase):
    pass


def static_test(source, code, reason):
    def test(self):
        refuse(source, code, reason)
    return test


for _name, _source, _code, _reason in hand.static_cases():
    setattr(Static, 'test_' + _name.replace('.', '_').replace('-', '_'), static_test(_source, _code, _reason))
for _name, _source, _reason in hand.combination_cases():
    setattr(Static, 'test_combination_' + _name, static_test(_source, 'U-SEM-0', _reason))


class Gates(unittest.TestCase):
    def test_verify_retains_external_backend_refusal(self):
        out, value = invoke(hand.basis(hand.M0), 'verify')
        verdict(out, value, 'unsupported')
        assert 'external backend is not connected' in out.stderr, out.stderr
        backend = value['ledger']['backends']['external']
        assert backend['status'] == 'not_connected' and backend['reason'] == 'external_backend_not_connected', backend

    def test_compare(self):
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                source = hand.add(hand.configured(hand.deterministic(hand.basis(hand.M0))), 'simulation replications 4 seed 7 confidence 0.5')
                refuse(source, 'U-SEM-0', 'evidence_basis_unsupported_command', 'compare', ('--plans', 'A,B', '--mode', mode))

    def test_diff(self):
        refuse(hand.configured(hand.basis(hand.M0)), 'U-SEM-0', 'evidence_basis_unsupported_command', 'diff', ('--from', 'A', '--to', 'B'))

    def test_sensitivity(self):
        source = hand.add(hand.deterministic(hand.basis(hand.M0)), 'verify probability reached end.posted >= 1\nsimulation replications 4 seed 7 confidence 0.5')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                refuse(source, 'U-SEM-0', 'evidence_basis_unsupported_command', 'sensitivity', ('--mode', mode), manifest=hand.sensitivity_manifest(mode))

    def test_conform_unchecked_never_satisfied_from_no_read_log(self):
        source = hand.basis(hand.M0.replace('  verify reachable end.posted\n', ''))
        out, value = invoke(source, 'conform', ('--log', '@events.jsonl', '--scope', 'event-contracts'), {'events.jsonl': hand.EMPTY_LOG})
        verdict(out, value, 'unsupported')
        errors = [d for d in value['diagnostics'] if d['severity'] == 'error']
        assert errors and all(d['code'] == 'U-LOG-1' and d.get('reason') == 'unsupported_contract' for d in errors), errors
        assert 'U-LOG-1' in out.stderr
        assert 'evidence_basis' in value.get('unchecked', []), value.get('unchecked')
        assert any(r['item'] == BASIS_ID and r['reason'] == 'unsupported_request' for r in value['not_checked']), value.get('not_checked')
        assert not any(c.get('verdict') == 'satisfied' and BASIS_ID in c.get('id', '') for c in value['checks'])


class RecordedResult(unittest.TextTestResult):
    """Observation ledger for this invocation, not a golden-output generator."""
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
        self.records.append({'test': test.id(), 'subtest': str(subtest),
                             'status': 'RED' if err else 'GREEN',
                             **({'failure': str(err[1])} if err else {})})

    def stopTestRun(self):
        super().stopTestRun()
        if RECORD:
            (RECORD / 'summary.json').write_text(json.dumps(self.records, indent=2) + '\n')


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--record-dir', type=Path)
    parser.add_argument('--inject-time-fault', action='store_true')
    parser.add_argument('--inject-verdict-fault', action='store_true')
    args, rest = parser.parse_known_args()
    BINARY = str(Path(args.binary).resolve())
    RECORD = args.record_dir
    if RECORD:
        assert RECORD.resolve().is_relative_to(ROOT.resolve()), 'observations must remain in worktree'
        RECORD.mkdir(parents=True, exist_ok=True)
    INJECT = args.inject_time_fault
    INJECT_VERDICT = args.inject_verdict_fault
    (ROOT / '.tmp').mkdir(exist_ok=True)
    unittest.main(argv=[__file__, *rest], testRunner=unittest.TextTestRunner(resultclass=RecordedResult, verbosity=2))
