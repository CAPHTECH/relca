"""Record-provenance stage-1 acceptance, with literal hand expectations.

--record-dir retains observations only. --inject time|wire calibrates initially
GREEN tests without editing production, fixtures, snapshots, or stored inputs.
"""
import argparse
from fractions import Fraction as F
import gzip
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import inputs as hand

ROOT = hand.ROOT
BINARY = ''
RECORD = None
INJECT = None
SERIAL = 0
EXITS = dict(satisfied=0, violated=1, inconclusive=2, invalid=3, unsupported=4, vacuous=5)


def invoke(source, command='check', args=(), files=None, manifest=None):
    global SERIAL
    SERIAL += 1
    if INJECT == 'time':
        source = source.replace('duration fixed 1min', 'duration fixed 2min')
    with tempfile.TemporaryDirectory(prefix='record-provenance-', dir=ROOT / '.tmp') as tmp:
        folder = Path(tmp)
        (folder / 'model.relca').write_text(source)
        for name, text in (files or {}).items():
            (folder / name).write_text(text)
        path = folder / 'model.relca'
        if manifest is not None:
            path = folder / 'manifest.json'
            path.write_text(json.dumps(manifest))
        argv = [BINARY, command, str(path), *[
            str(folder / a[1:]) if a.startswith('@') else a for a in args]]
        out = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, check=False)
        if RECORD:
            stem = RECORD / f'{SERIAL:04d}-{command}'
            stem.with_suffix('.relca').write_text(source)
            stem.with_suffix('.stdout.json').write_text(out.stdout)
            stem.with_suffix('.stderr').write_text(out.stderr)
            stem.with_suffix('.exit').write_text(str(out.returncode) + '\n')
        assert out.stdout.endswith('\n') and out.stdout.count('\n') == 1, (argv, out)
        value = json.loads(out.stdout)
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={value.get("verdict")} '
              f'errors={[(d["code"], d["message"]) for d in diagnostics(value) if d["severity"] == "error"]}', flush=True)
        if INJECT == 'wire':
            value['verdict'] = 'injected_unrelated_verdict'
        return out, value


def diagnostics(node):
    if isinstance(node, dict):
        yield from node.get('diagnostics', [])
        for key, value in node.items():
            if key != 'diagnostics':
                yield from diagnostics(value)
    elif isinstance(node, list):
        for value in node:
            yield from diagnostics(value)


def verdict(out, value, expected):
    assert out.returncode == EXITS[expected], (expected, out.returncode, list(diagnostics(value)), out.stderr)
    assert value['verdict'] == expected, (expected, value['verdict'])


def one(rows, key, expected):
    matches = [r for r in rows if r[key] == expected]
    assert len(matches) == 1, (key, expected, rows)
    return matches[0]


def basis(value, expected):
    assert one(value['properties'], 'id', hand.ID)['verdict'] == expected


def evidence_shape(trace):
    assert set(trace['evidence']) == {'binding', 'actor', 'capability', 'required', 'sources', 'missing', 'excluded_edges'}, trace['evidence']


def refuse(source, code, reason, command='check', args=(), manifest=None):
    out, value = invoke(source, command, args, manifest=manifest)
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    assert any(d['code'] == code and d['severity'] == 'error' and reason in d['message']
               for d in diagnostics(value)), (code, reason, list(diagnostics(value)))
    assert code in out.stderr and reason in out.stderr, out.stderr
    return value


def times(value, review_time, deterministic=False):
    rows = value['terminals']
    if not rows and value.get('case_outcomes'):
        assert len(value['case_outcomes']) == 1
        rows = value['case_outcomes'][0]['terminals']
    lo, hi = (review_time, review_time) if isinstance(review_time, int) else review_time
    expected = {'posted': (lo + 1, hi + 1) * 2}
    if not deterministic:
        expected['returned'] = (lo, hi) * 2
    got = {r['end']: (r['elapsed']['min'], r['elapsed']['max'], r['work']['min'], r['work']['max']) for r in rows}
    assert got == expected, (got, expected)
    assert value['bounds']['truncated'] is False


def via(record='Notes', call='r', people=None, source='Know', route='registrar_direct'):
    return {'record': record, 'registrars': [{'call': call, 'people': ['p1'] if people is None else people}],
            'sources': [{'source': source, 'route': route}]}


def observation(value):
    return value['agency']['viewpoints']['observation']


def qnormal(value, person='q1'):
    return one(observation(value)['people'], 'person', person)['normal']


def stage(value, call='b'):
    return one(observation(value)['stages'], 'call', call)


def records(actual, expected):
    assert 'via_records' in actual, ('missing via_records', actual)
    assert actual['via_records'] == expected, (actual['via_records'], expected)


def warning_count(value, code):
    return sum(d['code'] == code for d in value['diagnostics'])


def assert_lineage(value, copy=False):
    lineages = value['agency']['lineage']
    a = one(lineages, 'call', 'a')
    assert a['subject_sources'] == ['Invoice'] and a['independent_sources'] == ['Know'], a
    b = one(lineages, 'call', 'b')
    assert b['subject_sources'] == ['Invoice', 'Know'] and b['independent_sources'] == [], b
    groups = value['agency']['lineage_groups']
    assert {s for g in groups for s in g['independent_sources']} == {'Know'}, groups
    for row in lineages + groups:
        assert not {'Notes', 'Notes2'}.intersection(row['subject_sources'] + row['independent_sources']), row
    if not copy:
        assert {tuple(g['subject_sources']): g['independent_sources'] for g in groups} == {
            ('Invoice',): ['Know'], ('Invoice', 'Know'): []}, groups


def hand_case(name):
    def test(self):
        source, expected, review_time = hand.CASES[name]
        out, value = invoke(source)
        verdict(out, value, expected)
        basis(value, expected)
        if expected == 'unsupported':
            assert any(d['code'] == 'U-SEM-0' and 'evidence_basis_conditional_snapshot' in d['message']
                       for d in diagnostics(value)), list(diagnostics(value))
        else:
            times(value, review_time)
            assert one(value['properties'], 'id', 'reachable end.posted')['verdict'] == 'satisfied'
        if name in ('a1_r0', 'a2_dominating', 'a3_single_result', 'a4_overwrite_before', 'a5_copy_of_copy', 'u2_multiple_targets'):
            assert_lineage(value, copy=name == 'a5_copy_of_copy')
    return test


class Behavior(unittest.TestCase):
    pass


for case_name in hand.CASES:
    setattr(Behavior, 'test_' + case_name, hand_case(case_name))


class Boundaries(unittest.TestCase):
    pass


def static_test(source, code, reason):
    def test(self):
        for command in ('check', 'explain', 'simulate', 'mutate'):
            with self.subTest(command=command):
                refuse(hand.simulated(source) if command == 'simulate' else source, code, reason, command)
    return test


for case_name, src, code, reason in hand.static_cases():
    setattr(Boundaries, 'test_' + case_name, static_test(src, code, reason))
for case_name, src, reason in hand.combination_cases():
    setattr(Boundaries, 'test_' + case_name, static_test(src, 'U-SEM-0', reason))


class Observation(unittest.TestCase):
    def test_must_path_in_origin_people_and_stage_not_direct(self):
        out, value = invoke(hand.R0)
        verdict(out, value, 'satisfied')
        expected = [via()]
        for row in (one(value['agency']['origin'], 'actor', 'Reviewer'), qnormal(value), stage(value)):
            records(row, expected)
            assert row['direct'] == [], row
        assert stage(value)['via_outputs'] == ['Invoice'], stage(value)
        assert stage(value)['via_outputs_status'] == 'derived'

    def test_blind_never_and_latent_registrars_cannot_claim_knowledge(self):
        for name in ('a1_registrar_blind', 'a2_never', 'f5_latent_not_observation'):
            with self.subTest(model=name):
                out, value = invoke(hand.CASES[name][0])
                verdict(out, value, 'violated')
                records(qnormal(value), [])
                records(stage(value), [])
                assert qnormal(value)['direct'] == []

    def test_may_only_is_indirect_not_a_must_record_in_people_or_stage(self):
        for name in ('a2_conditional', 'a4_overwrite_maybe', 'f8_requires'):
            with self.subTest(model=name):
                out, value = invoke(hand.plain(hand.CASES[name][0]))
                verdict(out, value, 'satisfied')
                records(qnormal(value), [])
                records(stage(value), [])
                assert qnormal(value)['direct'] == []
                assert 'Know' in qnormal(value)['indirect'], qnormal(value)
                assert not any(key.startswith('may_') for key in stage(value))

    def test_last_writer_is_visible_not_the_overwritten_registrar(self):
        for name, expected in [('a4_overwrite_before', via()),
                               ('a4_overwrite_after', via(call='o', source='Invoice'))]:
            with self.subTest(model=name):
                out, value = invoke(hand.CASES[name][0])
                verdict(out, value, hand.CASES[name][1])
                records(qnormal(value), [expected])
                records(stage(value), [expected])

    def test_must_is_per_source_when_last_registrar_depends_on_path(self):
        out, value = invoke(hand.SAME_SOURCE_OVERWRITE)
        verdict(out, value, 'satisfied')
        basis(value, 'satisfied')
        times(value, (4, 5))
        expected = via()
        expected['registrars'].append({'call': 'r2', 'people': ['p1']})
        records(qnormal(value), [expected])
        records(stage(value), [expected])
        assert stage(value)['direct'] == []

    def test_relayed_sources_are_distinct_from_registrar_direct(self):
        for name in ('u4_relayed_argument', 'u4_relayed_snapshot'):
            with self.subTest(model=name):
                out, value = invoke(hand.CASES[name][0])
                verdict(out, value, 'satisfied')
                expected = via(route='relayed')
                expected['sources'] = [{'source': 'Invoice', 'route': 'relayed'}, {'source': 'Know', 'route': 'relayed'}]
                records(qnormal(value), [expected])
                records(stage(value), [expected])

    def test_copy_of_copy_keeps_immediate_record_and_deduplicates_sources(self):
        out, value = invoke(hand.COPY)
        verdict(out, value, 'satisfied')
        records(qnormal(value), [via('Notes2', 'c', route='relayed')])
        records(stage(value), [via('Notes2', 'c', route='relayed')])
        assert_lineage(value, copy=True)

    def test_multiple_targets_sorted_by_record_not_declaration_order(self):
        out, value = invoke(hand.MULTIPLE)
        verdict(out, value, 'satisfied')
        expected = [via('Notes'), via('Notes2')]
        records(qnormal(value), expected)
        records(stage(value), expected)
        assert_lineage(value)

    def test_unattributed_registrar_people_are_not_invented(self):
        source = hand.edit(hand.R0, 'actor Clerk { executors [p1]', 'actor Clerk { capacity 1')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        records(stage(value), [via(people=[])])

    def test_person_row_present_via_record_but_stage_missing_is_direct_only(self):
        for same_person in (False, True):
            source = hand.responsibility_source(same_person=same_person)
            out, value = invoke(source)
            verdict(out, value, 'satisfied')
            row = value['responsibilities'][0]
            obs = row['observation']
            assert obs['status'] == ('present' if same_person else 'present_via_record'), obs
            assert obs['required_sources'] == ['Know'] and obs['missing_sources'] == [], obs
            assert obs['direct'] == (['Know'] if same_person else []), obs
            records(obs, [via()])
            assert obs['stages_status'] == 'derived'
            assert len(obs['stages']) == 1
            s = obs['stages'][0]
            assert s['call'] == 'b' and s['required'] == ['Know'] and s['direct'] == [] and s['missing'] == ['Know'], s
            records(s, [via()])
            assert warning_count(value, 'W-RESP-1') == 0
            assert warning_count(value, 'W-RESP-3') == int(same_person)

    def test_responsibility_may_does_not_fill_person_missing(self):
        source = hand.responsibility_source(conditional=True)
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        obs = value['responsibilities'][0]['observation']
        assert obs['status'] == 'missing' and obs['missing_sources'] == ['Know'], obs
        records(obs, [])
        records(obs['stages'][0], [])
        assert obs['stages'][0]['missing'] == ['Know']
        assert warning_count(value, 'W-RESP-1') == 1 and warning_count(value, 'W-RESP-3') == 0

    def test_wlin2_may_registration_never_silences_common_error_warning(self):
        out, value = invoke(hand.wlin(conditional=True))
        verdict(out, value, 'satisfied')
        assert warning_count(value, 'W-LIN-2') == 1, value['diagnostics']
        assert 'Know' in qnormal(value)['indirect']
        records(qnormal(value), [])
        records(stage(value, 'bV'), [])
        assert not any(row['kind'] == 'wlin2_tacit' for row in value['agency'].get('model_uncertainties', {}).get('items', []))

    def test_wlin2_must_tacit_record_keeps_uncertainty_disclosure(self):
        out, value = invoke(hand.wlin())
        verdict(out, value, 'satisfied')
        # A must record can suppress B just like Know, but never without the
        # existing tacit-source disclosure. It is not a new explicit source.
        assert warning_count(value, 'W-LIN-2') == 0
        assert any(row['kind'] == 'wlin2_tacit' and row['sources'] == ['Know']
                   for row in value['agency'].get('model_uncertainties', {}).get('items', [])), value['agency']
        records(qnormal(value), [via()])

    def test_per_case_records_accept_two_cases_without_cross_case_provenance(self):
        out, value = invoke(hand.per_case())
        verdict(out, value, 'satisfied')
        row = one(value['properties'], 'id', hand.ID)
        assert [r['case_id'] for r in row['cases']] == [1, 2]
        assert [r['verdict'] for r in row['cases']] == ['satisfied', 'satisfied']


class Commands(unittest.TestCase):
    def test_computational_cut_before_reader_finish_does_not_certify_registration(self):
        out, value = invoke(hand.edit(hand.R0, 'clock 10min', 'clock 3min'))
        verdict(out, value, 'inconclusive')
        basis(value, 'inconclusive')
        assert value['bounds']['truncated'] is True
        assert value['terminals'] == []

    def test_explain_missing_finish_prefix_and_missing_set(self):
        for name, expected_events in [
            ('a1_registrar_blind', [(1, 'Clerk', 'enter'), (2, 'Clerk', 'register'), (3, 'Clerk', 'judge'), (4, 'Reviewer', 'review')]),
            ('a1_direct', [(1, 'Clerk', 'enter'), (2, 'Clerk', 'register'), (3, 'Clerk', 'judge'), (4, 'Reviewer', 'review')]),
            ('a4_overwrite_after', [(1, 'Clerk', 'enter'), (2, 'Clerk', 'register'), (3, 'Auditor', 'overwrite'), (4, 'Clerk', 'judge'), (5, 'Reviewer', 'review')]),
        ]:
            with self.subTest(model=name):
                out, value = invoke(hand.CASES[name][0], 'explain')
                verdict(out, value, 'violated')
                trace = one(value['explanation']['counterexamples'], 'id', hand.ID)
                evidence_shape(trace)
                assert trace['status'] == trace['replay']['status'] == 'verified', trace
                assert trace['evidence']['missing'] == ['Know'], trace
                assert trace['evidence']['required'] == ['Know'], trace
                assert trace['evidence']['sources'] == ([] if name == 'a1_direct' else ['Invoice']), trace
                finishes = [(s['time'], e['actor'], e['capability']) for s in trace['steps'] for e in s['events'] if e['kind'] == 'finish']
                assert finishes == expected_events, finishes

    def test_explain_definite_missing_keeps_conditional_record_edge(self):
        source = hand.add(hand.CONDITIONAL, 'actor Missing { source document }')
        source = hand.edit(source, 'include [Invoice,', 'include [Missing, Invoice,')
        source = hand.edit(source, 'evidence_basis Reviewer.review [Know]', 'evidence_basis Reviewer.review [Know, Missing]')
        out, value = invoke(source, 'explain')
        verdict(out, value, 'violated')
        trace = one(value['explanation']['counterexamples'], 'id', hand.ID)
        evidence_shape(trace)
        assert trace['replay']['status'] == 'verified'
        assert trace['evidence']['missing'] == ['Know', 'Missing']
        assert trace['evidence']['sources'] == ['Invoice']
        assert trace['evidence']['excluded_edges'] == ['r -> b']

    def test_simulate_is_sample_not_proof_and_cases_are_terminal_counts(self):
        for name, expected in [('a1_r0', 'inconclusive'), ('a1_registrar_blind', 'violated'), ('a4_overwrite_after', 'violated')]:
            with self.subTest(model=name):
                out, value = invoke(hand.simulated(hand.CASES[name][0]), 'simulate')
                verdict(out, value, expected)
                basis(value, expected)
                case = value['case_outcomes'][0]
                assert case['unfinished_count'] == 0
                assert case['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}]

    def test_mutate_evidence_missing_removes_the_registrar_read(self):
        out, value = invoke(hand.R0, 'mutate', ('--kinds', 'evidence_missing'))
        # The judge read can survive, so overall mutation status is violated.
        verdict(out, value, 'violated')
        kind = one(value['mutations'], 'kind', 'evidence_missing')
        mutant = one(kind['mutants'], 'id', 'evidence_missing:Clerk.register:reads:Know')
        assert mutant['result'] == 'detected', mutant
        assert any(change['key'] == hand.ID and change['direction'] == 'worsened'
                   for change in mutant['assigned_changes']), mutant

    def test_compare_and_diff_close_with_specific_command_reason(self):
        source = hand.simulated(hand.configured(hand.R0))
        for command, args in [('compare', ('--plans', 'A,B', '--mode', 'exhaustive')),
                              ('compare', ('--plans', 'A,B', '--mode', 'simulation')),
                              ('diff', ('--from', 'A', '--to', 'B'))]:
            with self.subTest(command=command, args=args):
                refuse(source, 'U-SEM-0', 'registration_unsupported_command', command, args)

    def test_sensitivity_closes_in_both_analysis_modes(self):
        source = hand.add(hand.simulated(hand.plain(hand.R0)), 'verify probability reached end.posted >= 1')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                refuse(source, 'U-SEM-0', 'registration_unsupported_command', 'sensitivity', ('--mode', mode), hand.manifest(mode))

    def test_conform_does_not_certify_unobserved_registration_provenance(self):
        source = hand.add(hand.plain(hand.R0).replace('  verify reachable end.posted\n', ''),
                          'contract Event { at_most_once Ledger.post.commit per case.id }')
        out, value = invoke(source, 'conform', ('--log', '@events.jsonl', '--scope', 'event-contracts'), {'events.jsonl': hand.EMPTY_LOG})
        verdict(out, value, 'inconclusive')
        assert 'registration_provenance' in value['unchecked'], value


class Substrate(unittest.TestCase):
    def test_responsibility_and_per_case_inputs_lower_with_old_snapshot_edges(self):
        for same_person, conditional in [(False, False), (True, False), (False, True)]:
            with self.subTest(same_person=same_person, conditional=conditional):
                out, value = invoke(hand.legacy(hand.responsibility_source(same_person, conditional)))
                verdict(out, value, 'satisfied')
                obs = value['responsibilities'][0]['observation']
                assert obs['required_sources'] == ['Know'], obs
                assert obs['status'] == ('present' if same_person else 'missing'), obs
                assert obs['stages'][0]['missing'] == ['Know'], obs
                assert warning_count(value, 'W-RESP-1') == int(not same_person)
                assert warning_count(value, 'W-RESP-3') == int(same_person)
        out, value = invoke(hand.legacy(hand.per_case()))
        verdict(out, value, 'satisfied')
        assert len(one(value['properties'], 'id', hand.ID)['cases']) == 2
        out, value = invoke(hand.legacy(hand.R0))
        times(value, 4)

    def test_latent_condition_and_basis_not_source_preserve_existing_reasons(self):
        cases = {name: (source, code, reason) for name, source, code, reason in hand.static_cases()}
        for name in ('latent_condition', 'basis_target_not_source'):
            source, code, reason = cases[name]
            refuse(hand.legacy(source), code, reason)

    def test_clock_cut_before_reader_is_inconclusive_in_existing_dsl(self):
        out, value = invoke(hand.legacy(hand.edit(hand.R0, 'clock 10min', 'clock 3min')))
        verdict(out, value, 'inconclusive')
        basis(value, 'inconclusive')
        assert value['bounds']['truncated'] is True
        assert value['terminals'] == []
        # Pin the hand clock so the temporary duration fault also fails here.
        out, value = invoke(hand.legacy(hand.R0))
        times(value, 4)

    def test_common_error_substrates_and_tacit_disclosure_shape(self):
        for direct, blind, warning in [(True, False, 0), (False, False, 0), (False, True, 1)]:
            with self.subTest(direct=direct, blind=blind):
                out, value = invoke(hand.legacy(hand.wlin(direct=direct, blind=blind)))
                verdict(out, value, 'satisfied')
                assert warning_count(value, 'W-LIN-2') == warning, value['diagnostics']
                items = value['agency'].get('model_uncertainties', {}).get('items', [])
                assert any(row['kind'] == 'wlin2_tacit' and row['sources'] == ['Know'] for row in items) == (not blind), items
                assert value['terminals'][0]['elapsed'] == {'min': 4, 'max': 4}

    def test_fraction_recalculation(self):
        minute = F(2 if INJECT == 'time' else 1, 1)
        assert minute + minute + minute + minute == F(4, 1)
        assert 5 * minute == F(5, 1)
        assert F(1, 4) + F(3, 4) == F(1, 1)  # count ratio, not nondeterministic probability
        assert F(2 * 2 * 2, 1) == F(8, 1)

    def test_each_legacy_equivalent_matches_hand_verdict_and_times(self):
        for name, (source, expected, time) in hand.CASES.items():
            with self.subTest(model=name):
                out, value = invoke(hand.legacy(source))
                verdict(out, value, expected)
                basis(value, expected)
                if expected == 'unsupported':
                    assert any(d['code'] == 'U-SEM-0' and 'evidence_basis_conditional_snapshot' in d['message'] for d in diagnostics(value))
                else:
                    times(value, time)

    def test_static_target_scaffolds_have_no_unrelated_parse_errors(self):
        for name, source, _, _ in hand.static_cases():
            if name in ('basis_target_not_source', 'latent_condition'):
                continue
            with self.subTest(model=name):
                out, value = invoke(hand.legacy(hand.plain(source)))
                verdict(out, value, 'satisfied')
                if name != 'shared_cases':
                    times(value, 4)
                assert not any(d['severity'] == 'error' for d in diagnostics(value))

    def test_combination_scaffolds_are_valid_or_own_known_snapshot_refusal(self):
        for name, source, reason in hand.combination_cases():
            with self.subTest(model=name):
                old = hand.legacy(source)
                if name == 'fork_sibling':
                    refuse(old, 'U-SEM-0', reason)
                    continue
                out, value = invoke(old)
                expected = 'inconclusive' if name.startswith(('retry_', 'recovery_')) else 'satisfied'
                verdict(out, value, expected)
                assert not any(d['severity'] == 'error' for d in diagnostics(value))
        # Time seam also calibrates this envelope-only test.
        out, value = invoke(hand.legacy(hand.R0))
        verdict(out, value, 'satisfied')
        times(value, 4)

    def test_command_scaffolds_use_real_existing_envelopes(self):
        source = hand.simulated(hand.configured(hand.legacy(hand.plain(hand.R0))))
        for command, args in [('compare', ('--plans', 'A,B', '--mode', 'exhaustive')),
                              ('compare', ('--plans', 'A,B', '--mode', 'simulation')),
                              ('diff', ('--from', 'A', '--to', 'B'))]:
            with self.subTest(command=command, args=args):
                out, value = invoke(source, command, args)
                verdict(out, value, 'satisfied')
        source = hand.add(hand.simulated(hand.legacy(hand.plain(hand.R0))), 'verify probability reached end.posted >= 1')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                out, value = invoke(source, 'sensitivity', ('--mode', mode), manifest=hand.manifest(mode))
                verdict(out, value, 'satisfied' if mode == 'exhaustive' else 'inconclusive')
        out, value = invoke(hand.legacy(hand.R0))
        verdict(out, value, 'satisfied')
        times(value, 4)

    def test_explain_existing_evidence_and_trace_fields(self):
        out, value = invoke(hand.legacy(hand.BLIND), 'explain')
        verdict(out, value, 'violated')
        trace = one(value['explanation']['counterexamples'], 'id', hand.ID)
        evidence_shape(trace)
        assert trace['status'] == trace['replay']['status'] == 'verified'
        assert trace['evidence']['missing'] == ['Know']
        assert trace['evidence']['sources'] == ['Invoice']
        assert [(s['time'], e['actor'], e['capability']) for s in trace['steps'] for e in s['events'] if e['kind'] == 'finish'] == [
            (1, 'Clerk', 'enter'), (2, 'Clerk', 'register'), (3, 'Clerk', 'judge'), (4, 'Reviewer', 'review')]

    def test_simulate_existing_sample_contract_shape(self):
        for source, expected in [(hand.R0, 'inconclusive'), (hand.BLIND, 'violated')]:
            out, value = invoke(hand.simulated(hand.legacy(source)), 'simulate')
            verdict(out, value, expected)
            basis(value, expected)
            assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}]
        out, value = invoke(hand.legacy(hand.R0))
        times(value, 4)


class Preservation(unittest.TestCase):
    def test_old_non_source_actor_is_still_invalid_without_registration(self):
        source = hand.edit(hand.R0, 'registers [Notes]; ', '')
        refuse(source, 'E-NAME-0', 'reads requires a declared source Actor')

    def test_source_record_is_not_automatically_reinterpreted(self):
        source = hand.edit(hand.edit(hand.R0, 'registers [Notes]; ', ''), 'actor Notes { }', 'actor Notes { source record }')
        out, value = invoke(source)
        verdict(out, value, 'violated')
        basis(value, 'violated')
        times(value, 4)
        assert qnormal(value)['direct'] == ['Notes']
        assert {s for g in value['agency']['lineage_groups'] for s in g['independent_sources']} == {'Know', 'Notes'}

    def test_contextual_registers_remains_usable_in_each_old_name_position(self):
        base = hand.legacy(hand.R0)
        for place, source in {
            'actor': base.replace('Clerk', 'registers'),
            'source': base.replace('Invoice', 'registers'),
            'capability': base.replace('capability enter()', 'capability registers()').replace('Clerk.enter()', 'Clerk.registers()'),
            'parameter': base.replace('review(e: V)', 'review(registers: V)'),
            'binding': base.replace('e = Clerk.enter()', 'registers = Clerk.enter()').replace('judge(e)', 'judge(registers)').replace('review(e)', 'review(registers)'),
            'type': base.replace('type V =', 'type registers =').replace(': V', ': registers').replace('-> V', '-> registers'),
            'process': base.replace('process Main', 'process registers'),
            'model': base.replace('model R0', 'model registers'),
            'contract': base.replace('contract Basis', 'contract registers'),
            'person': base.replace('p1', 'registers'),
            'observed': base.replace('note', 'registers'),
            'enum_value': base.replace('pass', 'registers'),
        }.items():
            with self.subTest(place=place):
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                times(value, 4)


def snapshots_module():
    spec = importlib.util.spec_from_file_location('record_current_snapshots', ROOT / 'scripts/snapshots.py')
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class Golden(unittest.TestCase):
    def test_all_old_current_stdout_stderr_and_exit_are_unchanged(self):
        module = snapshots_module()
        jobs = json.loads((module.SAVED / 'jobs.json').read_bytes())
        # Read the current golden, never regenerate or copy binary observations.
        # The manifest includes missing-path and no-positional-argument refusals,
        # not just model files. Pass every argv through unchanged.
        checked = 0
        for job in jobs:
            actual = module.capture(Path(BINARY), job)
            if INJECT == 'wire':
                actual = (actual[0] + b' ', *actual[1:])
            paths = [module.SAVED / (job['id'] + suffix) for suffix in ('.snap', '.snap.gz')]
            paths = [p for p in paths if p.exists()]
            assert len(paths) == 1, job
            raw = paths[0].read_bytes()
            saved = module.unpack(gzip.decompress(raw) if paths[0].suffix == '.gz' else raw)
            assert actual == saved, module.compare(job, saved, actual)
            checked += 1
        assert checked > 0
        print(f'PRESERVED jobs={checked} channels={3 * checked}', flush=True)


def main():
    global BINARY, RECORD, INJECT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', type=Path, required=True)
    parser.add_argument('--record-dir', type=Path)
    parser.add_argument('--inject', choices=('time', 'wire'))
    parser.add_argument('tests', nargs='*')
    args = parser.parse_args()
    BINARY = str(args.binary.resolve())
    INJECT = args.inject
    RECORD = args.record_dir
    (ROOT / '.tmp').mkdir(exist_ok=True)
    if RECORD:
        RECORD.mkdir(parents=True, exist_ok=True)
    unittest.main(argv=[sys.argv[0], *(args.tests or [])], verbosity=2)


if __name__ == '__main__':
    main()
