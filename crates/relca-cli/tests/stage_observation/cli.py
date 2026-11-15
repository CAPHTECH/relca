"""ADR 0074 stage-1 acceptance; expectations are from calculations.md.

python3 -B cli.py --binary target/debug/relca [Class.test_name]
--inject calibrates GREEN tests using temporary input/observation faults only.
--record-dir retains actual input/stdout/stderr/exit observations, never oracles.
"""
import argparse
from copy import deepcopy
from fractions import Fraction as F
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from typing import Any

import inputs as hand

ROOT = Path(__file__).resolve().parents[4]
BINARY = ''
INJECT = False
RECORD = None
SERIAL = 0
EXITS = dict(satisfied=0, violated=1, inconclusive=2, invalid=3, unsupported=4, vacuous=5)


def invoke(source, command='check', args=(), files=None, manifest=None):
    global SERIAL
    SERIAL += 1
    if INJECT:
        source = source.replace('duration fixed 1min', 'duration fixed 2min')
    with tempfile.TemporaryDirectory(prefix='stage-observation-', dir=ROOT / '.tmp') as d:
        folder = Path(d)
        path = folder / 'model.relca'
        path.write_text(source)
        for name, content in (files or {}).items():
            (folder / name).write_text(content)
        if manifest is not None:
            path = folder / 'manifest.json'
            path.write_text(json.dumps(manifest))
        argv = [BINARY, command, str(path), *[
            str(folder / a[1:]) if a.startswith('@') else a for a in args]]
        out = subprocess.run(argv, cwd=ROOT, capture_output=True, text=True, check=False)
        if RECORD:
            p = RECORD / f'{SERIAL:04d}-{command}'
            p.with_suffix('.relca').write_text(source)
            p.with_suffix('.stdout.json').write_text(out.stdout)
            p.with_suffix('.stderr').write_text(out.stderr)
            p.with_suffix('.exit').write_text(f'{out.returncode}\n')
        assert out.stdout.count('\n') == 1, (command, out.returncode, out.stdout, out.stderr)
        value = json.loads(out.stdout)
        # Static rejection cannot observe a duration fault: corrupt the reason
        # at this explicit seam. Recorded output remains actual raw CLI bytes.
        if INJECT:
            for diag in value.get('diagnostics', []):
                if diag['severity'] == 'error':
                    diag['message'] = 'injected unrelated rejection reason'
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={value.get("verdict")}', flush=True)
        return out, value


def verdict(out, value, expected):
    assert out.returncode == EXITS[expected], (expected, out.returncode, value.get('diagnostics'), out.stderr)
    assert value['verdict'] == expected, (expected, value.get('verdict'))


def one(rows, key, name):
    found = [r for r in rows if r[key] == name]
    assert len(found) == 1, (key, name, rows)
    return found[0]


def observation(value) -> dict[str, Any]:
    return value['agency']['viewpoints']['observation']


def responsibility(value):
    assert len(value['responsibilities']) == 1
    return value['responsibilities'][0]


def warning_count(value, code):
    return sum(d['code'] == code for d in value.get('diagnostics', []))


def stage(call, actor, capability, people, direct, via) -> dict[str, Any]:
    return dict(call=call, actor=actor, capability=f'{actor}.{capability}',
                people=people, unattributed=not people, direct=direct,
                via_outputs=via, via_outputs_status='derived')


def assert_stage(actual, expected):
    # Open to future independent via_records fields, not to a missing expected
    # field, a reason on derived, wrong people or an incorrectly empty via.
    for k, v in expected.items():
        assert actual.get(k, object()) == v, (k, v, actual)
    if expected['via_outputs_status'] == 'derived':
        assert 'via_outputs_reason' not in actual, actual
    else:
        assert actual['via_outputs'] is None, actual


def stages_expected(name):
    actor = 'Clerk' if name.startswith('s1-') else 'Checker'
    reread = name.endswith('reread')
    return [stage('e', 'Clerk', 'enter', ['c2'] if name == 's3-other' or name == 's3-reread' else ['c1'], ['Invoice'], []),
            stage('v', actor, 'confirm', ['c1'], ['Invoice'] if reread else [], [] if reread else ['Invoice']),
            stage('x', 'Ledger', 'post', [], [], ['Invoice'])]


def assert_stage_table(value, expected):
    o = observation(value)
    assert o['status'] == 'derived', o
    assert 'stages' in o, 'ADR0074 observation.stages is missing'
    assert isinstance(o['stages'], list), o
    assert [r['call'] for r in o['stages']] == [r['call'] for r in expected], o['stages']
    for actual, want in zip(o['stages'], expected):
        assert_stage(actual, want)


def assert_resp_stages(o, expected=None, status='derived', reason=None):
    assert o.get('stages_status') == status, ('ADR0074 responsibility stages_status', o)
    if status == 'derived':
        assert expected is not None
        assert 'stages_reason' not in o, o
        assert isinstance(o['stages'], list), o
        assert len(o['stages']) == len(expected), o
        for actual, want in zip(o['stages'], expected):
            for k, v in want.items():
                assert actual.get(k, object()) == v, (k, v, actual)
    else:
        assert o['stages'] is None, o
        assert o['stages_reason'] == reason, o


def resp_stage(call='v', source='Invoice', direct=False):
    return dict(call=call, required=[source], direct=[source] if direct else [],
                missing=[] if direct else [source])


def assert_responsibility(name, value):
    row = responsibility(value)
    o = row['observation']
    reread = name.endswith('reread')
    if name == 's4-variable':
        assert (o['status'], o['reason']) == ('not_derived', 'no_source_identified'), o
        assert_resp_stages(o, status='not_derived', reason='no_source_identified')
        assert any('responsibility_observation' in m for m in value['agency']['unchecked']), value['agency']['unchecked']
    else:
        expected = 'missing' if name == 's3-other' else 'present'
        assert o['status'] == expected and o['reason'] is None, o
        assert o['required_sources'] == ['Invoice'], o
        assert o['direct'] == ([] if name == 's3-other' else ['Invoice']), o
        assert o['missing_sources'] == (['Invoice'] if name == 's3-other' else []), o
        assert_resp_stages(o, [resp_stage(direct=reread)])
    w1 = int(name == 's3-other')
    w3 = int(not reread and name not in ('s3-other', 's4-variable'))
    assert warning_count(value, 'W-RESP-1') == w1, value['diagnostics']
    assert warning_count(value, 'W-RESP-3') == w3, value['diagnostics']
    assert row['warnings'].count('W-RESP-1') == w1, row
    assert row['warnings'].count('W-RESP-3') == w3, row
    for d in value['diagnostics']:
        if d['code'] == 'W-RESP-3':
            assert 'v' in d['message'] and 'Invoice' in d['message'], d


def assert_times(value, repeat=False, deterministic=False):
    rows = value['terminals']
    expected = {'posted': (3, 4) if repeat and not deterministic else (3, 3)}
    if not deterministic:
        expected['returned'] = (3, 3) if repeat else (2, 2)
    assert {r['end'] for r in rows} == set(expected), rows
    for r in rows:
        lo, hi = expected[r['end']]
        assert r['elapsed'] == {'min': lo, 'max': hi}, r
        assert r['work'] == {'min': lo, 'max': hi}, r
    assert value['bounds']['truncated'] is False


def assert_lineage(name, value):
    actor = 'Clerk' if name.startswith('s1-') else 'Checker'
    calls = ['@loop:rework:1:v', '@loop:rework:2:v'] if name.startswith('s5-') else ['v']
    expected = [dict(call=c, capability=f'{actor}.confirm', subject_sources=['Invoice'], independent_sources=[]) for c in calls]
    assert value['agency']['lineage'] == expected, value['agency']['lineage']


def refusal(out, value, code, reason):
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    assert any(d['code'] == code and d['severity'] == 'error' and reason in d['message']
               for d in value.get('diagnostics', [])), (code, reason, value.get('diagnostics'))
    assert code in out.stderr and reason in out.stderr, out.stderr


class Behavior(unittest.TestCase):
    pass


def behavior(name):
    def test(self):
        out, value = invoke(hand.MODELS[name])
        verdict(out, value, 'satisfied')
        assert_times(value, name.startswith('s5-'))
        assert_stage_table(value, stages_expected(name))
        assert_responsibility(name, value)
        assert_lineage(name, value)
    return test


for model_name in hand.MODELS:
    setattr(Behavior, 'test_' + model_name.replace('-', '_'), behavior(model_name))


def responsibility_behavior(name):
    def test(self):
        out, value = invoke(hand.MODELS[name])
        verdict(out, value, 'satisfied')
        assert_responsibility(name, value)
    return test


for model_name in hand.MODELS:
    setattr(Behavior, 'test_responsibility_' + model_name.replace('-', '_'), responsibility_behavior(model_name))


def warning_behavior(self):
    out, value = invoke(hand.S1)
    verdict(out, value, 'satisfied')
    assert warning_count(value, 'W-RESP-3') == 1, 'ADR0074 W-RESP-3 is missing'
    assert 'W-RESP-3:' in out.stderr


setattr(Behavior, 'test_wresp3_is_a_warning_with_exit_zero', warning_behavior)


class Substrate(unittest.TestCase):
    def test_each_original_dsl_input_parses_lowers_and_matches_hand_times(self):
        for name, source in hand.MODELS.items():
            with self.subTest(model=name):
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                assert_times(value, name.startswith('s5-'))
                assert_lineage(name, value)
                assert not any(d['severity'] == 'error' for d in value['diagnostics'])

    def test_fraction_recalculation(self):
        minute = F(2 if INJECT else 1, 1)
        assert minute + minute == F(2, 1)
        assert 3 * minute == F(3, 1)
        assert 4 * minute == F(4, 1)
        assert F(2, 2) == 1 and F(1, 1) == 1
        # Count ratios, not a measure on nondeterministic choices.
        assert F(1, 1) - F(1, 1) == 0

    def test_static_unknown_source_is_not_an_empty_direct_observation(self):
        for replacement in ('Missing', 'Clerk'):
            with self.subTest(read=replacement):
                source = hand.edit(hand.S1, 'reads [Invoice]', f'reads [{replacement}]')
                for command in ('check', 'explain', 'simulate'):
                    out, value = invoke(hand.simulated(source) if command == 'simulate' else source, command)
                    refusal(out, value, 'E-NAME-0', 'reads requires a declared source Actor')

    def test_static_unknown_responsible_person(self):
        out, value = invoke(hand.edit(hand.S1, 'responsible c1', 'responsible missing'))
        refusal(out, value, 'E-NAME-0', 'undefined person')

    def test_hidden_latent_read_stays_a_static_error_not_observation(self):
        source = hand.add(hand.S1, 'latent case { secret: bool = false }')
        source = hand.edit(source, 'if v == pass', 'if secret == true')
        for command in ('check', 'explain', 'simulate'):
            with self.subTest(command=command):
                out, value = invoke(hand.simulated(source) if command == 'simulate' else source, command)
                refusal(out, value, 'E-POL-LATENT', 'policy cannot read latent state')

    def test_extra_boundary_models_parse_and_lower_before_stage_assertions(self):
        for name, source in [('rule', hand.rule_source()), ('contest', hand.contest_source()),
                             ('fallback', hand.FALLBACK), ('merge', hand.MERGE), ('per_site', hand.PER_SITE)]:
            with self.subTest(model=name):
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                if name in ('rule', 'contest'):
                    assert_times(value)
                else:
                    times = {'fallback': {'done': (2, 2)}, 'merge': {'posted': (3, 3), 'returned': (3, 3)},
                             'per_site': {'posted': (4, 4)}}[name]
                    rows = value['case_outcomes'][0]['terminals'] if name == 'fallback' else value['terminals']
                    assert {r['end']: (r['elapsed']['min'], r['elapsed']['max']) for r in rows} == times
                    assert value['bounds']['truncated'] is False

    def test_existing_argument_availability_unsupported_reason_is_preserved(self):
        source = hand.edit(hand.S2, 'v = Checker.confirm(e)', 'if e == pass { f = Checker.confirm(e) } else {}\n    v = Checker.confirm(f)')
        # Both branches must actually be possible; a singleton enter would
        # make f definitely available and rightly avoid the rejection.
        source = hand.edit(source, 'reads [Invoice]; duration fixed 1min; result nondet { pass }',
                           'reads [Invoice]; duration fixed 1min; result nondet { pass, fail }')
        out, value = invoke(source)
        refusal(out, value, 'U-SEM-0', 'argument result is not definitely available')

    def test_compare_and_diff_scaffolds_close_without_new_stage_assertions(self):
        source = hand.simulated(hand.configured(hand.S1))
        for command, args in [('diff', ('--from', 'A', '--to', 'B')),
                              ('compare', ('--plans', 'A,B', '--mode', 'exhaustive')),
                              ('compare', ('--plans', 'A,B', '--mode', 'simulation'))]:
            with self.subTest(command=command, args=args):
                out, value = invoke(source, command, args)
                verdict(out, value, 'satisfied')
                reports = [p['report'] for p in value['plans']] if command == 'compare' else [value[k]['report'] for k in ('from', 'to')]
                if command == 'diff' or args[-1] == 'exhaustive':
                    for report in reports:
                        assert_times(report, deterministic=True)
                else:
                    # Calibration time was checked exhaustively above.
                    assert all(r['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}] for r in reports)

    def test_explain_and_simulate_use_existing_output_envelopes(self):
        out, value = invoke(hand.S1, 'explain')
        verdict(out, value, 'satisfied')
        assert_times(value)
        out, value = invoke(hand.simulated(hand.S1), 'simulate')
        verdict(out, value, 'satisfied')
        assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'posted', 'count': 4}]
        # Also pin time with the exhaustive same deterministic input, making the
        # temporary duration fault observable without guessing sample fields.
        out, value = invoke(hand.deterministic(hand.S1))
        verdict(out, value, 'satisfied')
        assert_times(value, deterministic=True)


class Boundaries(unittest.TestCase):
    def test_source_names_are_sorted_only_in_the_new_arrays(self):
        source = hand.PER_SITE.replace('reads [Z]', 'reads [Z,A]')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        rows = observation(value)['stages']
        assert one(rows, 'call', 'zed')['direct'] == ['A', 'Z']
        assert one(rows, 'call', 'one')['via_outputs'] == ['A', 'Z']
        assert one(rows, 'call', 'zed')['people'] == ['a', 'z']

    def test_required_is_per_position_not_per_capability_and_warning_is_per_row(self):
        out, value = invoke(hand.PER_SITE)
        verdict(out, value, 'satisfied')
        expected = [stage('zed', 'Intake', 'first', ['a', 'z'], ['Z'], []),
                    stage('alpha', 'Intake', 'second', ['a', 'z'], ['A'], []),
                    stage('one', 'Checker', 'confirm', ['a', 'z'], [], ['Z']),
                    stage('two', 'Checker', 'confirm', ['a', 'z'], [], ['A'])]
        assert_stage_table(value, expected)
        assert_resp_stages(responsibility(value)['observation'], [resp_stage('one', 'Z'), resp_stage('two', 'A')])
        assert warning_count(value, 'W-RESP-3') == 1
        assert warning_count(value, 'W-RESP-1') == 0

    def test_parameter_read_is_not_a_direct_source(self):
        source = hand.edit(hand.S1, 'capability confirm(e:V) -> V {', 'capability confirm(e:V) -> V { reads [e];')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert_stage_table(value, stages_expected('s1-artifact'))
        assert_responsibility('s1-artifact', value)

    def test_control_dependency_is_not_output_dependency(self):
        source = hand.edit(hand.S1, 'capability post(v:V)', 'capability post()')
        source = hand.edit(source, 'x = Ledger.post(v)', 'x = Ledger.post()')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        expected = stages_expected('s1-artifact')
        expected[-1]['via_outputs'] = []
        assert_stage_table(value, expected)

    def test_source_record_is_a_source_without_inventing_record_provenance(self):
        out, value = invoke(hand.S1.replace('source document', 'source record'))
        verdict(out, value, 'satisfied')
        assert_stage_table(value, stages_expected('s1-artifact'))

    def test_people_are_possible_executors_not_permission_or_qualification(self):
        source = hand.edit(hand.S2, 'actor Checker { executors [c1]', 'actor Checker { executors [c2,c1]')
        source = hand.edit(source, 'capability confirm(e:V) -> V {', 'capability confirm(e:V) -> V { qualified_executors [c2];')
        source = hand.add(source, 'permit c2 to Checker.confirm')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        expected = stages_expected('s2-artifact')
        expected[1]['people'] = ['c1', 'c2']
        assert_stage_table(value, expected)

    def test_merge_via_and_responsibility_are_not_derived_not_empty(self):
        out, value = invoke(hand.MERGE)
        verdict(out, value, 'satisfied')
        v = one(observation(value)['stages'], 'call', 'v')
        expected = stage('v', 'Clerk', 'confirm', ['c1'], [], None)
        expected.update(via_outputs_status='not_derived', via_outputs_reason='merge_control')
        assert_stage(v, expected)
        assert_resp_stages(responsibility(value)['observation'], status='not_derived', reason='merge_control')

    def test_actual_merge_if_example_has_unresolved_send_via(self):
        source = (ROOT / 'examples/merge-if/model.relca').read_text()
        out, value = invoke(source)
        verdict(out, value, 'violated')
        v = one(observation(value)['stages'], 'call', 's')
        assert v['direct'] == []
        assert v['via_outputs'] is None
        assert v['via_outputs_status'] == 'not_derived' and v['via_outputs_reason'] == 'merge_control'

    def test_maintenance_and_nonexecutor_are_not_applicable(self):
        for target, person, reason in [('Clerk.maintenance', 'c1', 'maintenance'), ('Clerk.confirm', 'c2', 'person_not_executor')]:
            with self.subTest(reason=reason):
                source = hand.edit(hand.S1, 'responsible c1 for Clerk.confirm', f'responsible {person} for {target}')
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                assert_resp_stages(responsibility(value)['observation'], status='not_applicable', reason=reason)

    def test_state_condition_selects_only_persons_writer_stage(self):
        for person in ('c1', 'c2'):
            with self.subTest(person=person):
                source = hand.edit(hand.S4, 'responsible c1 for Checker.confirm', f'responsible {person} for (entry == pass)')
                out, value = invoke(source)
                verdict(out, value, 'satisfied')
                o = responsibility(value)['observation']
                if person == 'c1':
                    assert_resp_stages(o, [resp_stage('e', direct=True)])
                else:
                    assert_resp_stages(o, status='not_derived', reason='no_person_stage')

    def test_unreachable_branch_calls_are_still_structural_stages(self):
        source = hand.edit(hand.S1, 'v = Clerk.confirm(e)', 'if e == fail { skipped = Clerk.confirm(e) } else {}\n    v = Clerk.confirm(e)')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        expected = stages_expected('s1-artifact')
        expected.insert(1, stage('skipped', 'Clerk', 'confirm', ['c1'], [], ['Invoice']))
        assert_stage_table(value, expected)
        assert_resp_stages(responsibility(value)['observation'], [resp_stage('skipped'), resp_stage()])
        assert warning_count(value, 'W-RESP-3') == 1

    def test_rules_do_not_become_observation_stages(self):
        source = hand.rule_source()
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert_stage_table(value, stages_expected('s1-artifact'))
        assert any('stage_observation(rule)' in mark for mark in value['agency']['unchecked']), value['agency']['unchecked']

    def test_contest_routes_do_not_become_normal_process_stages(self):
        source = hand.contest_source()
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert_stage_table(value, stages_expected('s1-artifact'))
        assert any('responsibility_stage(contest_path)' in mark for mark in value['agency']['unchecked']), value['agency']['unchecked']

    def test_fallback_path_is_excluded_with_reason_not_silently_successful(self):
        out, value = invoke(hand.FALLBACK)
        verdict(out, value, 'satisfied')
        assert_stage_table(value, [stage('p', 'Probe', 'run', ['c1'], [], []),
                                   stage('n', 'Worker', 'read', ['c1'], ['Doc'], [])])
        assert any('stage_observation(fallback_path)' in mark for mark in value['agency']['unchecked']), value['agency']['unchecked']


def comparison_unchecked(value):
    rows = value['unchecked']
    assert any('stage_observation' in r and 'stage_correspondence_not_defined' in r for r in rows), rows


class Commands(unittest.TestCase):
    def test_kind_nine_detects_confirm_not_enter_and_kind_zero_assignment_is_unchanged(self):
        for kind in ('original_access_removed', 'evidence_missing'):
            with self.subTest(kind=kind):
                out, value = invoke(hand.reread(hand.S1), 'mutate', ('--kinds', kind))
                verdict(out, value, 'violated')
                group = one(value['mutations'], 'kind', kind)
                mutants = group['mutants']
                assert len(mutants) == 2
                enter = one(mutants, 'id', f'{kind}:Clerk.enter:reads:Invoice')
                confirm = one(mutants, 'id', f'{kind}:Clerk.confirm:reads:Invoice')
                assert enter['result'] == 'undetected' and enter['reason'] == 'no_worsening', enter
                if kind == 'original_access_removed':
                    assert confirm['result'] == 'detected', confirm
                    assert_responsibility('s1-artifact', confirm['report'])
                    assert any(c['check'] == 'responsibilities' and c['direction'] == 'worsened'
                               for c in confirm['assigned_changes']), confirm['assigned_changes']
                else:
                    assert confirm['result'] == 'undetected' and confirm['reason'] == 'no_worsening', confirm
                    assert not any(c['check'] == 'responsibilities' for c in confirm['assigned_changes'])

    def test_diff_discloses_correspondence_not_defined(self):
        out, value = invoke(hand.configured(hand.S1), 'diff', ('--from', 'A', '--to', 'B'))
        verdict(out, value, 'satisfied')
        comparison_unchecked(value)

    def test_compare_both_modes_disclose_correspondence_not_defined(self):
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                out, value = invoke(hand.simulated(hand.configured(hand.S1)), 'compare', ('--plans', 'A,B', '--mode', mode))
                verdict(out, value, 'satisfied')
                comparison_unchecked(value)

    def test_check_explain_simulate_all_publish_the_new_stages(self):
        for command in ('check', 'explain', 'simulate'):
            with self.subTest(command=command):
                out, value = invoke(hand.simulated(hand.S1) if command == 'simulate' else hand.S1, command)
                verdict(out, value, 'satisfied')
                assert_stage_table(value, stages_expected('s1-artifact'))
                assert_responsibility('s1-artifact', value)


class Preservation(unittest.TestCase):
    def test_new_output_words_remain_legal_existing_identifiers(self):
        for word in ('stages', 'via_outputs', 'via_records', 'stages_status', 'direct', 'missing', 'required', 'unattributed'):
            for place in ('actor', 'capability', 'source', 'binding'):
                with self.subTest(word=word, place=place):
                    s = hand.S1
                    if place == 'actor':
                        s = s.replace('Clerk', word)
                    elif place == 'capability':
                        s = s.replace('confirm', word)
                    elif place == 'source':
                        s = s.replace('Invoice', word)
                    else:
                        s = s.replace('v = Clerk.confirm(e)', f'{word} = Clerk.confirm(e)').replace('if v == pass', f'if {word} == pass').replace('Ledger.post(v)', f'Ledger.post({word})')
                    out, value = invoke(s)
                    verdict(out, value, 'satisfied')
                    assert_times(value)

    def test_s1_reread_does_not_increase_independent_sources(self):
        reports = []
        for name in ('s1-artifact', 's1-reread'):
            out, value = invoke(hand.MODELS[name])
            verdict(out, value, 'satisfied')
            assert_times(value)
            assert_lineage(name, value)
            reports.append(value)
        a, b = reports
        for key in ('lineage', 'lineage_groups', 'origin', 'common_causes', 'model_uncertainties'):
            assert json.dumps(a['agency'].get(key), ensure_ascii=False) == json.dumps(b['agency'].get(key), ensure_ascii=False), key
        assert observation(a)['people'] == observation(b)['people']
        assert warning_count(a, 'W-LIN-2') == warning_count(b, 'W-LIN-2')

    def test_wresp1_keeps_person_wide_meaning_and_does_not_double_warn(self):
        out, value = invoke(hand.S3)
        verdict(out, value, 'satisfied')
        assert_times(value)
        o = responsibility(value)['observation']
        assert o['status'] == 'missing' and o['direct'] == [] and o['missing_sources'] == ['Invoice']
        assert warning_count(value, 'W-RESP-1') == 1
        assert warning_count(value, 'W-RESP-3') == 0

class Page(unittest.TestCase):
    def render(self, source, value, out):
        with tempfile.TemporaryDirectory(prefix='stage-page-', dir=ROOT / '.tmp') as d:
            p = Path(d)
            (p / 'model.relca').write_text(source)
            (p / 'check.json').write_text(json.dumps(value, ensure_ascii=False))
            (p / 'check.err').write_text(out.stderr)
            result = subprocess.run(['python3', '-B', 'skills/relca-model/scripts/review_page.py',
                                     '--model', str(p / 'model.relca'), '--check', str(p / 'check.json'),
                                     '--check-exit', str(out.returncode), '--check-stderr', str(p / 'check.err'),
                                     '--no-mermaid', '-o', str(p / 'page.html')], cwd=ROOT, capture_output=True, text=True)
            assert result.returncode == 0, result.stderr
            return (p / 'page.html').read_text()

    def test_old_output_explicitly_says_stage_records_absent(self):
        out, value = invoke(hand.S1)
        observation(value).pop('stages', None)
        for key in ('stages', 'stages_status', 'stages_reason'):
            responsibility(value)['observation'].pop(key, None)
        page = self.render(hand.S1, value, out)
        assert '段ごとの記録なし' in page

    def test_stage_missing_source_is_displayed_next_to_responsibility(self):
        out, value = invoke(hand.S1)
        assert_stage_table(value, stages_expected('s1-artifact'))
        page = self.render(hand.S1, value, out)
        assert 'この段では生成物だけ' in page
        assert 'W-RESP-3' in page and 'Invoice' in page and 'Clerk.confirm' in page


def hand_report(name) -> dict[str, Any]:
    # Deliberately handwritten normal future observation, not patched live JSON.
    o: dict[str, Any] = dict(status='present', reason=None, required_sources=['Invoice'], direct=['Invoice'],
             missing_sources=[], stages_status='derived', stages=[resp_stage(direct=name.endswith('reread'))])
    w1 = int(name == 's3-other')
    w3 = int(not name.endswith('reread') and name not in ('s3-other', 's4-variable'))
    if name == 's3-other':
        o.update(status='missing', direct=[], missing_sources=['Invoice'])
    if name == 's4-variable':
        o.update(status='not_derived', reason='no_source_identified', required_sources=[], stages=None,
                 stages_status='not_derived', stages_reason='no_source_identified')
    codes = ['W-RESP-1'] * w1 + ['W-RESP-3'] * w3
    value = dict(agency=dict(viewpoints=dict(observation=dict(status='derived', stages=stages_expected(name))),
                             unchecked=['responsibility_observation'] if name == 's4-variable' else []),
                 responsibilities=[dict(observation=o, warnings=codes)],
                 diagnostics=[dict(code=c, message='v missing Invoice') for c in codes])
    if INJECT:
        # Corrupt the positive witness of each comparator self-test; the
        # ordinary validator must reject it before testing the negative pair.
        observation(value)['stages'][0]['direct'] = []
        o['stages_status'] = 'injected_unknown_status'
    return value


class Calibration(unittest.TestCase):
    def rejected(self, validator, broken):
        with self.assertRaises(AssertionError):
            validator(broken)

    def test_f1_via_is_not_direct_and_f2_actor_or_person_union_is_not_stage(self):
        for name in ('s1-artifact', 's2-artifact', 's4-variable'):
            with self.subTest(model=name):
                good = hand_report(name)
                assert_stage_table(good, stages_expected(name))
                for direct, via in [(['Invoice'], []), (['Invoice'], ['Invoice'])]:
                    bad = deepcopy(good)
                    bad['agency']['viewpoints']['observation']['stages'][1].update(direct=direct, via_outputs=via)
                    self.rejected(lambda v: assert_stage_table(v, stages_expected(name)), bad)

    def test_f3_reread_must_not_become_an_independent_source(self):
        good = {'agency': {'lineage': [dict(call='v', capability='Clerk.confirm', subject_sources=['Invoice'], independent_sources=[])]}}
        if INJECT:
            good['agency']['lineage'][0]['independent_sources'] = ['Invoice']
        assert_lineage('s1-reread', good)
        bad = deepcopy(good)
        bad['agency']['lineage'][0]['independent_sources'] = ['Invoice']
        self.rejected(lambda v: assert_lineage('s1-reread', v), bad)

    def test_f4_person_missing_source_does_not_duplicate_wresp1(self):
        good = hand_report('s3-other')
        assert_responsibility('s3-other', good)
        bad = deepcopy(good)
        bad['diagnostics'].append(dict(code='W-RESP-3', message='v Invoice'))
        bad['responsibilities'][0]['warnings'].append('W-RESP-3')
        self.rejected(lambda v: assert_responsibility('s3-other', v), bad)

    def test_f5_not_derived_must_not_become_empty_success(self):
        good = hand_report('s4-variable')
        assert_responsibility('s4-variable', good)
        for fault in ('success', 'empty', 'no_unchecked'):
            bad = deepcopy(good)
            o = responsibility(bad)['observation']
            if fault == 'success':
                o.update(status='present', reason=None)
            elif fault == 'empty':
                o['stages'] = []
            else:
                bad['agency']['unchecked'] = []
            self.rejected(lambda v: assert_responsibility('s4-variable', v), bad)

    def test_f6_repeat_is_one_lexical_stage_and_one_warning(self):
        good = hand_report('s5-repeat')
        assert_stage_table(good, stages_expected('s5-repeat'))
        assert_responsibility('s5-repeat', good)
        bad = deepcopy(good)
        rows = observation(bad)['stages']
        rows.insert(2, deepcopy(rows[1]))
        rows[1]['call'], rows[2]['call'] = '@loop:rework:1:v', '@loop:rework:2:v'
        self.rejected(lambda v: assert_stage_table(v, stages_expected('s5-repeat')), bad)
        bad = deepcopy(good)
        bad['diagnostics'].append(dict(code='W-RESP-3', message='v Invoice'))
        self.rejected(lambda v: assert_responsibility('s5-repeat', v), bad)

    def test_f7_merge_unknown_via_is_not_a_union_of_branches(self):
        expected = stage('v', 'Clerk', 'confirm', ['c1'], [], None)
        expected.update(via_outputs_status='not_derived', via_outputs_reason='merge_control')
        positive = deepcopy(expected)
        if INJECT:
            positive['via_outputs'] = []
        assert_stage(positive, expected)
        bad = deepcopy(expected)
        bad.update(via_outputs=['Invoice'], via_outputs_status='derived')
        bad.pop('via_outputs_reason')
        self.rejected(lambda v: assert_stage(v, expected), bad)

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True)
    parser.add_argument('--inject', action='store_true')
    parser.add_argument('--record-dir', type=Path)
    options, tests = parser.parse_known_args()
    BINARY = str(Path(options.binary).resolve())
    INJECT = options.inject
    RECORD = options.record_dir
    (ROOT / '.tmp').mkdir(exist_ok=True)
    if RECORD:
        assert RECORD.resolve().is_relative_to(ROOT)
        RECORD.mkdir(parents=True, exist_ok=True)
    unittest.main(argv=[sys.argv[0], *tests], verbosity=2)
