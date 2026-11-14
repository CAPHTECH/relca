"""ADR 0071 real-CLI acceptance, independent of snapshots and production DTOs.

Run with --record-dir for observations. --fault is a temporary calibration at
the input/wire seam, not a claim that unimplemented production code was tested.
The inventory-fault class is deliberately opt-in for an internal
report-deletion injection; it is not part of the ordinary positive suite.
"""
import argparse
import copy
from fractions import Fraction
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

import inputs as h

ROOT = h.ROOT
BINARY = ''
RECORD = None
FAULT = ''
SERIAL = 0
EXITS = dict(satisfied=0, violated=1, inconclusive=2, invalid=3, unsupported=4, vacuous=5)


def invoke(source, command='check', args=(), files=None, manifest=None):
    global SERIAL
    SERIAL += 1
    if FAULT == 'timing':
        source = source.replace('duration fixed 1min', 'duration fixed 2min')
    with tempfile.TemporaryDirectory(prefix='contract-scope-', dir=ROOT / '.tmp') as tmp:
        d = Path(tmp)
        (d / 'model.relca').write_text(source)
        for name, contents in (files or {}).items():
            (d / name).write_text(contents)
        entry = 'model.relca'
        if manifest is not None:
            entry = 'manifest.json'
            (d / entry).write_text(json.dumps(manifest))
        argv = [BINARY, command, str(d / entry), *[
            str(d / arg[1:]) if arg.startswith('@') else arg for arg in args]]
        out = subprocess.run(argv, capture_output=True, cwd=ROOT, text=True)
        assert out.stdout.endswith('\n') and out.stdout.count('\n') == 1, (argv, out)
        value = json.loads(out.stdout)
        if RECORD:
            prefix = RECORD / f'{SERIAL:04d}-{command}'
            prefix.with_suffix('.relca').write_text(source)
            prefix.with_suffix('.stdout.json').write_text(out.stdout)
            prefix.with_suffix('.stderr').write_text(out.stderr)
            prefix.with_suffix('.exit').write_text(f'{out.returncode}\n')
        if FAULT == 'verdict':
            value['verdict'] = 'satisfied'
        if FAULT == 'diagnostics':
            def clear(v):
                if isinstance(v, dict):
                    if 'diagnostics' in v:
                        v['diagnostics'] = []
                    for child in v.values():
                        clear(child)
                elif isinstance(v, list):
                    for child in v:
                        clear(child)
            clear(value)
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={value.get("verdict")} '
              f'errors={[(d["code"], d["message"]) for d in diagnostics(value) if d["severity"] == "error"]}', flush=True)
        return out, value


def diagnostics(value):
    if isinstance(value, dict):
        yield from value.get('diagnostics', [])
        for key, child in value.items():
            if key != 'diagnostics':
                yield from diagnostics(child)
    elif isinstance(value, list):
        for child in value:
            yield from diagnostics(child)


def verdict(out, value, expected):
    assert out.returncode == EXITS[expected], (expected, out.returncode, out.stderr)
    assert value['verdict'] == expected, (expected, value['verdict'])


def diagnostic(value, code, reason=None):
    matches = [d for d in diagnostics(value) if d['code'] == code]
    assert matches, (code, list(diagnostics(value)))
    assert all(d['severity'] == ('warning' if code.startswith('W-') else 'error') for d in matches)
    if reason is not None:
        assert any(reason in d['message'] for d in matches), (reason, matches)
    return matches


def no_diagnostic(value, code):
    assert not any(d['code'] == code for d in diagnostics(value)), (code, list(diagnostics(value)))


def row(value, id=h.ID):
    rows = [r for r in value['properties'] if r['id'] == id]
    assert len(rows) == 1, (id, value.get('properties'))
    return rows[0]


def terminal_time(value, time, work=None):
    rows = value.get('terminals') or [r for c in value['case_outcomes'] for r in c['terminals'] if r['kind'] == 'end']
    assert len(rows) == 1 and rows[0]['end'] == 'done', rows
    assert rows[0]['elapsed'] == {'min': time, 'max': time}, rows
    assert rows[0]['work'] == {'min': work or time, 'max': work or time}, rows


def scope(value, id=h.ID, contract='Controls', plans=('New',), reason=h.REASON, masks=False):
    rows = value['contract_scope']
    found = [r for r in rows if r['id'] == id]
    assert len(found) == 1, (id, rows)
    assert type(found[0]['masks_call']) is bool, found[0]
    assert found[0] == {'id': id, 'contract': contract, 'applies_to': list(plans),
                        'reason': reason, 'status': 'not_applicable', 'masks_call': masks}, found[0]
    assert value['unchecked'].count('contract.scope.' + id) == 1, value['unchecked']
    assert not any(r['id'] == id for r in value.get('properties', [])), value.get('properties')


def no_scope(value):
    assert not value.get('contract_scope'), value.get('contract_scope')
    assert not any(s.startswith('contract.scope.') for s in value.get('unchecked', []))


def refuse(source, code, reason, command='check', args=(), **kwargs):
    out, value = invoke(source, command, args, **kwargs)
    verdict(out, value, 'invalid' if code.startswith('E-') else 'unsupported')
    diagnostic(value, code, reason)
    assert code in out.stderr and reason in out.stderr, out.stderr
    return value


def mutation(value, kind):
    return next(r for r in value['mutations'] if r['kind'] == kind)


def old_time_control():
    out, value = invoke(h.BASE, args=('--configuration', 'New'))
    verdict(out, value, 'satisfied')
    terminal_time(value, 2)


class Legacy(unittest.TestCase):
    def test_m1_global_contract_keeps_vacuity_and_hand_times(self):
        for configuration, expected, time, contract_verdict in [
                ('Old', 'vacuous', 1, 'vacuous'), ('New', 'satisfied', 2, 'satisfied')]:
            with self.subTest(configuration=configuration):
                out, value = invoke(h.BASE, args=('--configuration', configuration))
                verdict(out, value, expected)
                assert row(value)['verdict'] == contract_verdict
                no_scope(value)
                assert 'contract_scope' not in value
                terminal_time(value, time)

    def test_bad_grant_and_dead_branch_keep_their_old_meanings(self):
        for source, expected, time in [(h.bad_grant(), 'violated', 2), (h.dead_branch(), 'vacuous', 1)]:
            with self.subTest(expected=expected):
                out, value = invoke(source, args=('--configuration', 'New'))
                verdict(out, value, expected)
                assert row(value)['verdict'] == expected
                terminal_time(value, time)

    def test_guidance_absent_for_called_and_single_configuration(self):
        source = h.edit(h.unused(), '  configuration Old { cut K; process AsIs }\n', '')
        source = h.edit(source, '  process AsIs { p = Desk.pay(); end done }\n', '')
        out, value = invoke(source, args=('--configuration', 'New'))
        verdict(out, value, 'vacuous')
        no_diagnostic(value, 'W-CONF-1')
        terminal_time(value, 1)
        out, value = invoke(h.BASE, args=('--configuration', 'New'))
        verdict(out, value, 'satisfied')
        no_diagnostic(value, 'W-CONF-1')

    def test_no_configuration_model_keeps_global_contract(self):
        source = h.edit(h.BASE, '  configuration Old { cut K; process AsIs }\n', '')
        source = h.edit(source, '  configuration New { cut K; process ToBe }\n', '')
        source = h.edit(source, '  process AsIs { p = Desk.pay(); end done }\n', '')
        out, value = invoke(source)
        verdict(out, value, 'satisfied')
        assert row(value)['verdict'] == 'satisfied'
        assert 'contract_scope' not in value
        terminal_time(value, 2)

    def test_context_words_are_existing_names_not_global_keywords(self):
        for word in ('applies_to', 'reason'):
            for place in ('actor', 'capability', 'binding', 'contract', 'configuration'):
                with self.subTest(word=word, place=place):
                    source, configuration = h.BASE, 'New'
                    if place == 'actor':
                        source = source.replace('Desk', word)
                    elif place == 'capability':
                        source = source.replace('pay', word)
                    elif place == 'binding':
                        source = source.replace('p = Desk.pay()', f'{word} = Desk.pay()')
                    elif place == 'contract':
                        source = source.replace('Controls', word)
                    else:
                        source = source.replace('configuration New', f'configuration {word}')
                        configuration = word
                    out, value = invoke(source, args=('--configuration', configuration))
                    verdict(out, value, 'satisfied')
                    terminal_time(value, 2)
                    no_scope(value)

    def test_waive_substrate_has_five_minute_completion(self):
        out, value = invoke(h.WAIVE, args=('--configuration', 'New'))
        verdict(out, value, 'satisfied')
        terminal_time(value, 5)
        assert row(value, 'contract Delivery #1 response')['verdict'] == 'satisfied'

    def test_evidence_existing_read_and_missing_read(self):
        for read, expected in [(True, 'satisfied'), (False, 'violated')]:
            with self.subTest(read=read):
                out, value = invoke(h.evidence(read=read), args=('--configuration', 'New'))
                verdict(out, value, expected)
                assert row(value, 'contract Evidence #1 evidence_basis')['verdict'] == expected
                terminal_time(value, 2)

    def test_legacy_compare_diff_and_explain_envelopes(self):
        source = h.without_control()
        out, value = invoke(source, 'compare', ('--plans', 'Old,New', '--mode', 'exhaustive'))
        verdict(out, value, 'satisfied')
        for cell in value['tables']['properties'][0]['values']:
            assert 'status' not in cell
        for p, time in zip(value['plans'], [1, 2]):
            terminal_time(p['report'], time)
        out, value = invoke(source, 'diff', ('--from', 'Old', '--to', 'New'))
        verdict(out, value, 'satisfied')
        assert all('from_status' not in r and 'to_status' not in r for r in value['properties'])
        terminal_time(value['from']['report'], 1)
        terminal_time(value['to']['report'], 2)
        out, value = invoke(source, 'explain', ('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        terminal_time(value, 1)

    def test_legacy_simulation_is_not_a_safety_proof(self):
        source = h.edit(h.simulation(h.without_control()), 'verify reachable end.done', 'verify safety no_resource_overcommit')
        out, value = invoke(source, 'simulate', ('--configuration', 'Old'))
        verdict(out, value, 'inconclusive')
        assert row(value, 'safety no_resource_overcommit')['verdict'] == 'inconclusive'
        assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'done', 'count': 4}]
        old_time_control()

    def test_legacy_mutate_authority_is_one_detected_candidate(self):
        out, value = invoke(h.BASE, 'mutate', ('--configuration', 'New', '--kinds', 'authority_expired'))
        verdict(out, value, 'satisfied')
        terminal_time(value['original'], 2)
        mutants = mutation(value, 'authority_expired')['mutants']
        assert len(mutants) == 1 and mutants[0]['result'] == 'detected', mutants

    def test_excluded_authority_old_dsl_projection_has_no_mutant(self):
        out, value = invoke(h.without_control(), 'mutate', ('--configuration', 'Old', '--kinds', 'authority_expired'))
        verdict(out, value, 'vacuous')
        assert value['original']['verdict'] == 'satisfied'
        terminal_time(value['original'], 1)
        assert mutation(value, 'authority_expired')['mutants'] == []
        assert value['run']['reason'] == 'no_selected_mutant'

    def test_legacy_sensitivity_same_question_has_exact_unit_probability(self):
        source = h.probability_model(h.without_control())
        out, value = invoke(source, 'sensitivity', ('--mode', 'exhaustive'), manifest=h.manifest())
        verdict(out, value, 'satisfied')
        assert len(value['points']) == 2
        for p in value['points']:
            assert row(p['report'], 'probability reached #1')['probability'] == {'min': '1', 'max': '1'}
            terminal_time(p['report'], 1)

    def test_zero_request_lower_refusal_precedes_target_resolution(self):
        source = h.edit(h.without_control(), '  verify reachable end.done\n', '')
        for mode in ('exhaustive', 'simulation'):
            with self.subTest(mode=mode):
                model = source if mode == 'exhaustive' else h.simulation(source)
                out, value = invoke(model, 'sensitivity', ('--mode', mode), manifest=h.manifest(mode, ('Old',)))
                verdict(out, value, 'unsupported')
                zero_sensitivity_diagnostics(value, model)

    def test_legacy_declared_evaluation_has_unit_paid_per_arrival(self):
        out, value = invoke(h.evaluation_model(h.without_control()), 'compare',
                            ('--plans', 'Old,New', '--mode', 'exhaustive'))
        verdict(out, value, 'satisfied')
        assert [p['status'] for p in value['evaluation']['plans']] == ['admissible', 'admissible']
        for p, time in zip(value['plans'], [1, 2]):
            terminal_time(p['report'], time)

    def test_legacy_conform_single_and_double_commit(self):
        for duplicate, expected in [(False, 'satisfied'), (True, 'violated')]:
            with self.subTest(duplicate=duplicate):
                out, value = invoke(h.conform_model(), 'conform', ('--configuration', 'New', '--log', '@events.jsonl', '--scope', 'event-contracts'),
                                    files={'events.jsonl': h.log(duplicate)})
                verdict(out, value, expected)
                if duplicate:
                    assert any(c.get('reason') == 'at_most_once' and c['conformance'] == 'violating' for c in value['checks'])
        old_time_control()

    def test_conform_excluded_contract_old_dsl_projections(self):
        source = h.edit(h.conform_model(), 'contract Controls { at_most_once Desk.pay.commit per case.id }', '')
        evidence_source = h.edit(h.conform_evidence_model(), 'contract Evidence { evidence_basis Screen.approve [Document] }', '')
        for model in (source, evidence_source):
            with self.subTest(evidence='Document' in model):
                out, value = invoke(model, 'conform', ('--configuration', 'Old', '--log', '@events.jsonl', '--scope', 'event-contracts'),
                                    files={'events.jsonl': h.log(True)})
                verdict(out, value, 'satisfied')
                assert not any(c.get('reason') == 'at_most_once' for c in value['checks'])
        old_time_control()

    def test_legacy_verify_remains_unconnected(self):
        out, value = invoke(h.BASE, 'verify')
        verdict(out, value, 'unsupported')
        assert 'diagnostics' not in value
        assert out.stderr == 'relca verify: not implemented; outside Phase 1; external backend is not connected\n'

    def test_fraction_recalculation(self):
        minute = Fraction(2 if FAULT == 'timing' else 1, 1)
        self.assertEqual(minute, Fraction(1, 1))
        self.assertEqual(minute + minute, Fraction(2, 1))
        self.assertEqual(minute + 4 * minute, Fraction(5, 1))
        self.assertEqual(Fraction(1, 1) / Fraction(1, 1), 1)


def legacy_clause_test(body, expected):
    def test(self):
        out, value = invoke(h.clause_model(body), args=('--configuration', 'New'))
        verdict(out, value, expected)
        terminal_time(value, 2)
    return test


for _i, (_kind, _body, _expected, _masks) in enumerate(h.clause_cases()):
    setattr(Legacy, f'test_clause_{_i}_{_kind}', legacy_clause_test(_body, _expected))


class Check(unittest.TestCase):
    def test_context_words_are_still_names_inside_scope_headers(self):
        source = h.BASE.replace('Controls', 'applies_to').replace('configuration New', 'configuration reason')
        source = h.scoped(source, 'applies_to', 'reason')
        out, value = invoke(source, args=('--configuration', 'reason'))
        verdict(out, value, 'satisfied')
        id = 'contract applies_to #1 authority_basis'
        assert row(value, id)['verdict'] == 'satisfied'
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, id, 'applies_to', ('reason',))

    def test_scope_declaration_list_is_preserved_in_report(self):
        source = h.add(h.BASE, 'configuration Third { cut K; process ToBe }')
        source = h.scoped(source, plans='Third,New')
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, plans=('Third', 'New'))

    def test_a1_old_is_not_vacuous_or_satisfied_contract(self):
        out, value = invoke(h.scoped(), args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        assert value['properties'] == [{'id': 'reachable end.done', 'verdict': 'satisfied'}]
        scope(value)
        assert len(value['contract_scope']) == 1
        no_diagnostic(value, 'W-CONF-2')
        terminal_time(value, 1)

    def test_a1b_excluded_call_is_warned_not_rejected(self):
        out, value = invoke(h.scoped(h.both_call()), args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, masks=True)
        diagnostic(value, 'W-CONF-2')
        assert 'W-CONF-2' in out.stderr
        terminal_time(value, 2)

    def test_excluded_violating_call_is_disclosed_but_not_folded(self):
        out, value = invoke(h.scoped(h.bad_grant(h.both_call())), args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, masks=True)
        diagnostic(value, 'W-CONF-2')
        terminal_time(value, 2)

    def test_a2_new_still_checks_the_control(self):
        out, value = invoke(h.scoped(), args=('--configuration', 'New'))
        verdict(out, value, 'satisfied')
        assert row(value)['verdict'] == 'satisfied'
        no_scope(value)
        no_diagnostic(value, 'W-CONF-2')
        terminal_time(value, 2)

    def test_a2b_bad_grant_still_violates_inside_scope(self):
        out, value = invoke(h.scoped(h.bad_grant()), args=('--configuration', 'New'))
        verdict(out, value, 'violated')
        assert row(value)['verdict'] == 'violated'
        no_scope(value)

    def test_a3_dead_branch_is_vacuous_not_automatically_exempt(self):
        out, value = invoke(h.scoped(h.only_new(h.dead_branch())), args=('--configuration', 'New'))
        verdict(out, value, 'vacuous')
        assert row(value)['verdict'] == 'vacuous'
        no_scope(value)
        diagnostic(value, 'W-UNREACH-0', 'never_started')
        no_diagnostic(value, 'W-CONF-1')
        terminal_time(value, 1)

    def test_excluded_dead_branch_still_masks_a_lexical_call(self):
        out, value = invoke(h.scoped(h.dead_branch(), plans='Old'), args=('--configuration', 'New'))
        verdict(out, value, 'satisfied')
        scope(value, plans=('Old',), masks=True)
        diagnostic(value, 'W-CONF-2')
        terminal_time(value, 1)

    def test_not_called_inside_scope_keeps_vacuity_and_guidance(self):
        out, value = invoke(h.scoped(h.unused()), args=('--configuration', 'New'))
        verdict(out, value, 'vacuous')
        diagnostic(value, 'W-UNREACH-0', 'not_called')
        diagnostic(value, 'W-CONF-1')
        assert 'Controls' in out.stderr and 'New' in out.stderr and 'applies_to' in out.stderr

    def test_old_global_not_called_also_gets_guidance(self):
        out, value = invoke(h.BASE, args=('--configuration', 'Old'))
        verdict(out, value, 'vacuous')
        diagnostic(value, 'W-CONF-1')
        assert 'Controls' in out.stderr and 'Old' in out.stderr

    def test_ordinal_counts_excluded_clauses(self):
        for configuration, ids in [('Old', ['reachable end.done', 'contract Later #2 permitted']),
                                   ('New', ['reachable end.done', h.ID, 'contract Later #2 permitted'])]:
            with self.subTest(configuration=configuration):
                out, value = invoke(h.ordinal_source(), args=('--configuration', configuration))
                verdict(out, value, 'satisfied')
                assert [r['id'] for r in value['properties']] == ids

    def test_multiple_clauses_keep_model_wide_ordinal(self):
        source = h.edit(h.scoped(), 'authority_basis Screen.approve Mgr }',
                        'authority_basis Screen.approve Mgr; permitted Desk.pay }')
        source = h.add(source, 'contract Later { permitted Desk.pay }')
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        assert [p['id'] for p in value['properties']] == ['reachable end.done', 'contract Later #3 permitted']
        scope(value)
        scope(value, 'contract Controls #2 permitted', masks=True)
        assert len(value['contract_scope']) == 2

    def test_all_configurations_enumerated_is_not_an_exemption(self):
        for configuration, expected in [('Old', 'vacuous'), ('New', 'satisfied')]:
            with self.subTest(configuration=configuration):
                out, value = invoke(h.scoped(plans='Old,New'), args=('--configuration', configuration))
                verdict(out, value, expected)
                assert row(value)['verdict'] == expected
                no_scope(value)
                no_diagnostic(value, 'W-CONF-2')

    def test_omitted_scope_contract_applies_even_beside_a_scoped_one(self):
        source = h.add(h.scoped(), 'contract Global { authority_basis Screen.approve Mgr }')
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'vacuous')
        scope(value)
        assert row(value, 'contract Global #2 authority_basis')['verdict'] == 'vacuous'

    def test_no_configuration_selection_still_requires_selection(self):
        value = refuse(h.scoped(), 'E-CONF-0', 'configuration')
        assert not value['properties']

    def test_waive_and_response_equal_scopes_are_accepted(self):
        source = h.scoped(h.scoped(h.WAIVE, 'Delivery'), 'Exception')
        for configuration in ('Old', 'New'):
            with self.subTest(configuration=configuration):
                out, value = invoke(source, args=('--configuration', configuration))
                verdict(out, value, 'satisfied')
                terminal_time(value, 5)
                if configuration == 'Old':
                    scope(value, 'contract Delivery #1 response', 'Delivery', masks=False)
                    scope(value, 'contract Exception #2 waive', 'Exception', masks=True)
                else:
                    assert row(value, 'contract Delivery #1 response')['verdict'] == 'satisfied'

    def test_waive_subset_does_not_waive_the_other_configuration(self):
        source = h.scoped(h.WAIVE, 'Exception')
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'violated')
        scope(value, 'contract Exception #2 waive', 'Exception', masks=True)
        assert row(value, 'contract Delivery #1 response')['verdict'] == 'violated'


def scoped_clause_test(kind, body, expected, masks):
    def test(self):
        source = h.scoped(h.clause_model(body))
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, f'contract Controls #1 {kind}', masks=masks)
        if masks:
            diagnostic(value, 'W-CONF-2')
        else:
            no_diagnostic(value, 'W-CONF-2')
        out, value = invoke(source, args=('--configuration', 'New'))
        verdict(out, value, expected)
        assert row(value, f'contract Controls #1 {kind}')['verdict'] == expected
    return test


for _i, (_kind, _body, _expected, _masks) in enumerate(h.clause_cases()):
    setattr(Check, f'test_clause_{_i}_{_kind}', scoped_clause_test(_kind, _body, _expected, _masks))


class Static(unittest.TestCase):
    pass


def static_test(source, code, reason):
    def test(self):
        value = refuse(source, code, reason, args=('--configuration', 'Old'))
        assert not value['properties']
        # Missing reason and anonymous scope stop parsing: Cut is unresolved.
        if reason in ('contract_scope_reason_missing', 'contract_scope_anonymous'):
            assert value['cut'] == 'unresolved'
            no_diagnostic(value, 'W-CUT-0')
    return test


for _name, _source, _code, _reason in h.static_cases():
    setattr(Static, 'test_' + _name.replace('-', '_'), static_test(_source, _code, _reason))


class Comparison(unittest.TestCase):
    def test_a4_rows_cells_fold_and_warning(self):
        out, value = invoke(h.scoped(), 'compare', ('--plans', 'Old,New', '--mode', 'exhaustive'))
        verdict(out, value, 'satisfied')
        compare_scope(value)

    def test_u6_asymmetric_scope_does_not_make_admissible_plan_undetermined(self):
        out, value = invoke(h.evaluation_model(h.scoped()), 'compare', ('--plans', 'Old,New', '--mode', 'exhaustive'))
        verdict(out, value, 'satisfied')
        plans = value['evaluation']['plans']
        assert [(p['configuration'], p['verify_verdict'], p['hard_verdict'], p['status']) for p in plans] == [
            ('Old', 'satisfied', 'satisfied', 'admissible'), ('New', 'satisfied', 'satisfied', 'admissible')]
        assumptions = [a for a in value['assumptions'] if a['field'] == 'compare.contract_scope']
        assert len(assumptions) == 1 and assumptions[0]['value'] == 'asymmetric', assumptions
        assert all(name in assumptions[0]['affects'] for name in ('Controls', 'Old', 'New')), assumptions

    def test_a5_preserve_cannot_select_an_excluded_destination(self):
        source = h.edit(h.scoped(), 'configuration Old { cut K; process AsIs }',
                        'configuration Old { cut K; process AsIs; intent { preserve Controls } }')
        value = refuse(source, 'E-DIFF-0', 'Controls', 'diff', ('--from', 'New', '--to', 'Old'))
        assert value['verdict'] == 'invalid'

    def test_a5_removed_and_added_are_directional(self):
        for a, b, change in [('New', 'Old', 'removed'), ('Old', 'New', 'added')]:
            with self.subTest(change=change):
                out, value = invoke(h.scoped(), 'diff', ('--from', a, '--to', b))
                verdict(out, value, 'satisfied')
                diff_scope(value, change)
                diagnostic(value, 'W-DIFF-1')
                diagnostic(value, 'W-DIFF-3')


def compare_scope(value):
    assert len(value['tables']['properties']) == 2, value['tables']['properties']
    r = next(r for r in value['tables']['properties'] if r['id'] == h.ID)
    assert r['values'][0] == {'configuration': 'Old', 'status': 'not_applicable', 'verdict': None, 'reason': 'configuration_scope'}, r
    assert r['values'][1]['configuration'] == 'New' and r['values'][1]['status'] == 'applied'
    assert r['values'][1]['verdict'] == 'satisfied'
    reachable = next(r for r in value['tables']['properties'] if r['id'] == 'reachable end.done')
    assert [c['status'] for c in reachable['values']] == ['applied', 'applied']
    assert len([d for d in value['diagnostics'] if d['code'] == 'W-CMP-1']) == 1
    assert 'contract_scope' in value['checked']
    scope(value['plans'][0]['report'])
    assert row(value['plans'][1]['report'])['verdict'] == 'satisfied'


def diff_scope(value, change):
    rows = [r for r in value['differences'] if r['kind'] == 'contract_scope']
    assert len(rows) == 1 and rows[0]['target'] == 'Controls' and rows[0]['change'] == change, rows
    r = row(value)
    a, b = ('applied', 'not_applicable') if change == 'removed' else ('not_applicable', 'applied')
    assert (r['from_status'], r['to_status']) == (a, b), r
    assert r['from'] == ('satisfied' if a == 'applied' else None), r
    assert r['to'] == ('satisfied' if b == 'applied' else None), r
    # Preservation must not count not_applicable as a satisfied contract.
    assert 'not_applicable' not in (value.get('preservation') or {}).get('summary', {})


class OtherCommands(unittest.TestCase):
    def test_explain_explicit_omission(self):
        out, value = invoke(h.scoped(), 'explain', ('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value)
        assert value['explanation']['counterexamples'] == []
        assert {'id': h.ID, 'verdict': None, 'reason': 'not_applicable_configuration_scope'} in value['explanation']['omitted']

    def test_explain_active_violation_has_verified_finish_witness(self):
        out, value = invoke(h.scoped(h.bad_grant()), 'explain', ('--configuration', 'New'))
        verdict(out, value, 'violated')
        t = next(t for t in value['explanation']['counterexamples'] if t['id'] == h.ID)
        assert t['replay']['status'] == 'verified'
        finishes = [(s['time'], e['actor'], e['capability']) for s in t['steps'] for e in s['events'] if e['kind'] == 'finish']
        assert finishes == [(1, 'Screen', 'approve')], finishes

    def test_simulate_scope_without_inventing_a_proof(self):
        source = h.edit(h.simulation(h.scoped()), 'verify reachable end.done', 'verify safety no_resource_overcommit')
        for configuration in ('Old', 'New'):
            with self.subTest(configuration=configuration):
                out, value = invoke(source, 'simulate', ('--configuration', configuration))
                verdict(out, value, 'inconclusive')
                assert row(value, 'safety no_resource_overcommit')['verdict'] == 'inconclusive'
                assert value['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'done', 'count': 4}]
                if configuration == 'Old':
                    scope(value)
                else:
                    assert row(value)['verdict'] == 'inconclusive'
        out, value = invoke(h.simulation(h.scoped(h.bad_grant())), 'simulate', ('--configuration', 'New'))
        verdict(out, value, 'violated')

    def test_compare_simulation_scope_is_not_an_inventory_error(self):
        source = h.edit(h.simulation(h.scoped()), 'verify reachable end.done', 'verify safety no_resource_overcommit')
        out, value = invoke(source, 'compare', ('--plans', 'Old,New', '--mode', 'simulation'))
        verdict(out, value, 'inconclusive')
        r = next(r for r in value['tables']['properties'] if r['id'] == h.ID)
        assert r['values'][0]['verdict'] is None and r['values'][0]['status'] == 'not_applicable'
        assert r['values'][1]['verdict'] == 'inconclusive'
        diagnostic(value, 'W-CMP-1')

    def test_mutate_excludes_authority_candidates_not_just_detectors(self):
        out, value = invoke(h.scoped(), 'mutate', ('--configuration', 'Old', '--kinds', 'authority_expired'))
        verdict(out, value, 'vacuous')
        assert value['original']['verdict'] == 'satisfied'
        scope(value['original'])
        assert mutation(value, 'authority_expired')['mutants'] == []
        assert value['run']['reason'] == 'no_selected_mutant'
        out, value = invoke(h.scoped(), 'mutate', ('--configuration', 'New', '--kinds', 'authority_expired'))
        verdict(out, value, 'satisfied')
        mutants = mutation(value, 'authority_expired')['mutants']
        assert len(mutants) == 1 and mutants[0]['result'] == 'detected', mutants

    def test_sensitivity_same_scope_and_changed_declaration_gate(self):
        source = h.probability_model(h.scoped())
        out, value = invoke(source, 'sensitivity', ('--mode', 'exhaustive'), manifest=h.manifest())
        verdict(out, value, 'satisfied')
        for p in value['points']:
            scope(p['report'])
            assert row(p['report'], 'probability reached #1')['probability'] == {'min': '1', 'max': '1'}
        # Both points select Old; neither includes Controls after selection.
        source = h.add(source, 'configuration Third { cut K; process ToBe }')
        changed = h.edit(source, 'applies_to [New]', 'applies_to [Third]')
        value = refuse(source, 'E-CMP-0', 'sensitivity_contract_mismatch', 'sensitivity', ('--mode', 'exhaustive'),
                       files={'changed.relca': changed}, manifest=h.manifest(models=['model.relca', 'changed.relca']))
        assert all(p['status'] != 'evaluated' for p in value['points'])

    def test_sensitivity_simulation_preserves_scope_metadata(self):
        source = h.simulation(h.probability_model(h.scoped()))
        out, value = invoke(source, 'sensitivity', ('--mode', 'simulation'), manifest=h.manifest('simulation'))
        verdict(out, value, 'inconclusive')
        for p in value['points']:
            scope(p['report'])

    def test_conform_does_not_run_excluded_monitor(self):
        source = h.scoped(h.conform_model())
        for configuration, expected in [('Old', 'satisfied'), ('New', 'violated')]:
            with self.subTest(configuration=configuration):
                out, value = invoke(source, 'conform', ('--configuration', configuration, '--log', '@events.jsonl', '--scope', 'event-contracts'),
                                    files={'events.jsonl': h.log(True)})
                verdict(out, value, expected)
                if configuration == 'Old':
                    scope(value, 'contract Controls #1 at_most_once', masks=True)
                    assert not any(c.get('reason') == 'at_most_once' for c in value['checks'])
                    assert any(c['id'] == 'contract:2' and c['conformance'] == 'conformant' for c in value['checks'])
                else:
                    assert any(c.get('reason') == 'at_most_once' for c in value['checks'])

    def test_verify_still_unconnected_with_new_syntax(self):
        out, value = invoke(h.scoped(), 'verify')
        verdict(out, value, 'unsupported')
        assert 'diagnostics' not in value
        assert out.stderr == 'relca verify: not implemented; outside Phase 1; external backend is not connected\n'


class Evidence(unittest.TestCase):
    def test_out_of_scope_and_in_scope_evidence_basis(self):
        id = 'contract Evidence #1 evidence_basis'
        for read in (False, True):
            source = h.scoped(h.evidence(read=read), 'Evidence')
            with self.subTest(read=read):
                out, value = invoke(source, args=('--configuration', 'Old'))
                verdict(out, value, 'satisfied')
                scope(value, id, 'Evidence')
                out, value = invoke(source, args=('--configuration', 'New'))
                verdict(out, value, 'satisfied' if read else 'violated')
                assert row(value, id)['verdict'] == ('satisfied' if read else 'violated')

    def test_excluded_evidence_does_not_trigger_command_refusal(self):
        source = h.add(h.evidence(read=False), 'configuration Third { cut K; process ToBe }')
        source = h.scoped(source, 'Evidence', 'Third')
        for command, args in [('compare', ('--plans', 'Old,New', '--mode', 'exhaustive')),
                               ('diff', ('--from', 'Old', '--to', 'New'))]:
            with self.subTest(command=command):
                out, value = invoke(source, command, args)
                verdict(out, value, 'satisfied')
                children = [p['report'] for p in value['plans']] if command == 'compare' else [value[k]['report'] for k in ('from', 'to')]
                for report, masks in zip(children, [False, True]):
                    scope(report, 'contract Evidence #1 evidence_basis', 'Evidence', ('Third',), masks=masks)

    def test_conform_excluded_evidence_is_scope_not_unsupported_request(self):
        source = h.scoped(h.conform_evidence_model(), 'Evidence')
        out, value = invoke(source, 'conform', ('--configuration', 'Old', '--log', '@events.jsonl', '--scope', 'event-contracts'),
                            files={'events.jsonl': h.log()})
        verdict(out, value, 'satisfied')
        scope(value, 'contract Evidence #1 evidence_basis', 'Evidence')
        assert not any('evidence_basis' in str(x) and x.get('reason') == 'unsupported_request' for x in value['not_checked'])

    def test_sensitivity_excluded_evidence_preserves_unchecked_scope(self):
        source = h.add(h.probability_model(h.evidence()), 'configuration Third { cut K; process ToBe }')
        source = h.scoped(source, 'Evidence', 'Third')
        out, value = invoke(source, 'sensitivity', ('--mode', 'exhaustive'), manifest=h.manifest())
        verdict(out, value, 'satisfied')
        for p in value['points']:
            scope(p['report'], 'contract Evidence #1 evidence_basis', 'Evidence', ('Third',))

    def test_simulate_and_explain_excluded_evidence(self):
        source = h.scoped(h.evidence(read=False), 'Evidence')
        out, value = invoke(source, 'explain', ('--configuration', 'Old'))
        verdict(out, value, 'satisfied')
        scope(value, 'contract Evidence #1 evidence_basis', 'Evidence')
        assert {'id': 'contract Evidence #1 evidence_basis', 'verdict': None,
                'reason': 'not_applicable_configuration_scope'} in value['explanation']['omitted']
        source = h.edit(h.simulation(source), 'verify reachable end.done', 'verify safety no_resource_overcommit')
        out, value = invoke(source, 'simulate', ('--configuration', 'Old'))
        verdict(out, value, 'inconclusive')
        scope(value, 'contract Evidence #1 evidence_basis', 'Evidence')

    def test_in_scope_evidence_keeps_its_existing_unsupported_command(self):
        source = h.scoped(h.evidence(), 'Evidence', 'Old,New')
        refuse(source, 'U-SEM-0', 'evidence_basis_unsupported_command', 'compare', ('--plans', 'Old,New', '--mode', 'exhaustive'))


class ClosedContest(unittest.TestCase):
    pass


NO_REQUEST = 'at least one verification request is required'


def zero_sensitivity_diagnostics(value, source):
    # Full arrays, including span, severity and every message. No unresolved
    # target diagnostic is permitted: lower failed before target resolution.
    expected = {'code': 'U-SEM-0', 'severity': 'error', 'message': NO_REQUEST,
                'span': {'start': 0, 'end': len(source.rstrip())}}
    wrapped = dict(expected, message='sensitivity_model_invalid: point "p0": ' + NO_REQUEST)
    assert value['diagnostics'] == [wrapped], value['diagnostics']
    assert value['points'][0]['report']['diagnostics'] == [expected], value['points'][0]['report']['diagnostics']
    assert list(diagnostics(value)) == [wrapped, expected], list(diagnostics(value))


class ZeroRequests(unittest.TestCase):
    def test_excluded_state_invariant_is_not_a_hidden_request(self):
        source = h.scoped(h.clause_model('invariant flag == true'))
        source = h.edit(source, '  verify reachable end.done\n', '')
        out, value = invoke(source, args=('--configuration', 'Old'))
        verdict(out, value, 'unsupported')
        diagnostic(value, 'U-SEM-0', NO_REQUEST)
        scope(value, 'contract Controls #1 invariant', masks=True)
        diagnostic(value, 'W-CONF-2')


def zero_test(command, args, simulation=False):
    def test(self):
        source = h.zero_requests()
        if simulation:
            source = h.simulation(source)
        out, value = invoke(source, command, args,
                            files={'events.jsonl': h.log()},
                            manifest=h.manifest('simulation' if simulation else 'exhaustive', ('Old',)) if command == 'sensitivity' else None)
        verdict(out, value, 'unsupported')
        diagnostic(value, 'U-SEM-0', NO_REQUEST)
        if command == 'mutate':
            report = value['original']
            assert not any(m['mutants'] for m in value['mutations'])
        elif command == 'sensitivity':
            report = value['points'][0]['report']
            zero_sensitivity_diagnostics(value, source)
            assert value['points'][0]['status'] == 'refused'
        else:
            report = value
        scope(report)
        assert report['verdict'] == 'unsupported'
        if 'properties' in report:
            assert report['properties'] == []
    return test


for _command, _args, _sim in [
        ('check', ('--configuration', 'Old'), False),
        ('explain', ('--configuration', 'Old'), False),
        ('simulate', ('--configuration', 'Old'), True),
        ('mutate', ('--configuration', 'Old', '--kinds', 'authority_expired'), False),
        ('conform', ('--configuration', 'Old', '--log', '@events.jsonl', '--scope', 'event-contracts'), False),
        ('sensitivity', ('--mode', 'exhaustive'), False),
        ('sensitivity', ('--mode', 'simulation'), True)]:
    setattr(ZeroRequests, 'test_' + _command + ('_simulation' if _command == 'sensitivity' and _sim else ''),
            zero_test(_command, _args, _sim))


def contest_test(command, args, source=None, manifest=None, files=None):
    def test(self):
        input_source = source or h.scoped(h.CONTEST)
        if command == 'simulate' or 'simulation' in args:
            input_source = h.add(input_source, 'simulation replications 4 seed 7 confidence 0.5')
        refuse(input_source, 'U-SEM-0', 'contract_scope_contest', command, args,
               manifest=manifest, files=files)
    return test


for _command, _args in [
        ('check', ('--configuration', 'Old')), ('explain', ('--configuration', 'Old')),
        ('simulate', ('--configuration', 'Old')), ('mutate', ('--configuration', 'Old', '--kinds', 'authority_expired')),
        ('compare', ('--plans', 'Old,New', '--mode', 'exhaustive')), ('diff', ('--from', 'Old', '--to', 'New')),
        ('conform', ('--configuration', 'Old', '--log', '@events.jsonl', '--scope', 'event-contracts'))]:
    setattr(ClosedContest, 'test_' + _command, contest_test(_command, _args, files={'events.jsonl': h.log()}))
setattr(ClosedContest, 'test_compare_simulation',
        contest_test('compare', ('--plans', 'Old,New', '--mode', 'simulation')))
for _mode in ('exhaustive', 'simulation'):
    setattr(ClosedContest, 'test_sensitivity_' + _mode,
            contest_test('sensitivity', ('--mode', _mode),
                         h.add(h.scoped(h.CONTEST), 'verify probability reached end.returned >= 1'), h.manifest(_mode)))


class PageAcceptance(unittest.TestCase):
    def test_scope_is_displayed_and_counted_as_unverified(self):
        # A hand wire makes the page test independent of the missing parser.
        source = h.scoped()
        report = {'model': {'name': 'Ctl', 'version': '0.2', 'sha256': hashlib.sha256(source.encode()).hexdigest()},
                  'mode': 'exhaustive', 'configuration': 'Old', 'cut': 'K', 'bounds': {'cases': 1, 'live_tasks': 1, 'clock': 5, 'truncated': False},
                  'verdict': 'satisfied', 'diagnostics': [], 'assumptions': [],
                  'properties': [{'id': 'reachable end.done', 'verdict': 'satisfied'}],
                  'unchecked': ['contract.scope.' + h.ID],
                  'contract_scope': [{'id': h.ID, 'contract': 'Controls', 'applies_to': ['New'], 'reason': h.REASON,
                                      'status': 'not_applicable', 'masks_call': False}]}
        with tempfile.TemporaryDirectory(prefix='contract-scope-page-', dir=ROOT / '.tmp') as tmp:
            d = Path(tmp)
            (d / 'model.relca').write_text(source)
            (d / 'check.json').write_text(json.dumps(report))
            spec = importlib.util.spec_from_file_location('scope_review_page', ROOT / 'skills/relca-model/scripts/review_page.py')
            assert spec is not None and spec.loader is not None
            page_module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(page_module)
            args = page_module.parse_args(['--model', str(d / 'model.relca'), '--check', str(d / 'check.json'),
                                          '--check-exit', '0', '--no-mermaid', '-o', str(d / 'page.html')])
            page = page_module.Page(args)
            html = page.render()
            embedded = re.search(r'id="review-data">(.*?)</script>', html)
            assert embedded is not None
            summary = json.loads(embedded.group(1))
            assert h.REASON in html and 'New' in html and 'Controls' in html
            assert any(h.ID in str(r) and 'not_applicable' in str(r) for r in summary['unverified']), summary['unverified']
            assert summary['unverified_count'] >= 1
            assert 'contract_scope' not in summary['unread_keys']
            assert 'src="https:' not in html


class OracleCalibration(unittest.TestCase):
    def test_hand_scope_positive_and_single_faults(self):
        good = {'contract_scope': [{'id': h.ID, 'contract': 'Controls', 'applies_to': ['New'], 'reason': h.REASON,
                                    'status': 'not_applicable', 'masks_call': False}],
                'unchecked': ['contract.scope.' + h.ID], 'properties': []}
        scope(good)
        for fault in ('missing', 'satisfied', 'mask', 'unchecked', 'property', 'ordinal'):
            bad = copy.deepcopy(good)
            if fault == 'missing':
                bad['contract_scope'] = []
            elif fault == 'satisfied':
                bad['contract_scope'][0]['status'] = 'satisfied'
            elif fault == 'mask':
                bad['contract_scope'][0]['masks_call'] = True
            elif fault == 'unchecked':
                bad['unchecked'] = []
            elif fault == 'property':
                bad['properties'] = [{'id': h.ID, 'verdict': 'satisfied'}]
            else:
                bad['contract_scope'][0]['id'] = 'contract Controls #2 authority_basis'
            with self.subTest(fault=fault), self.assertRaises(AssertionError):
                scope(good if FAULT == 'oracle' else bad)

    def test_hand_compare_catches_dropped_row_null_verdict_and_missing_warning(self):
        excluded = {'contract_scope': [{'id': h.ID, 'contract': 'Controls', 'applies_to': ['New'], 'reason': h.REASON,
                                        'status': 'not_applicable', 'masks_call': False}],
                    'unchecked': ['contract.scope.' + h.ID], 'properties': []}
        active = {'properties': [{'id': h.ID, 'verdict': 'satisfied'}]}
        good = {'tables': {'properties': [
            {'id': 'reachable end.done', 'values': [{'status': 'applied'}, {'status': 'applied'}]},
            {'id': h.ID, 'values': [
                {'configuration': 'Old', 'status': 'not_applicable', 'verdict': None, 'reason': 'configuration_scope'},
                {'configuration': 'New', 'status': 'applied', 'verdict': 'satisfied'}]}]},
                'plans': [{'report': excluded}, {'report': active}],
                'diagnostics': [{'code': 'W-CMP-1'}], 'checked': ['contract_scope']}
        compare_scope(good)
        for fault in ('row', 'verdict', 'warning'):
            bad = copy.deepcopy(good)
            if fault == 'row':
                bad['tables']['properties'].pop()
            elif fault == 'verdict':
                bad['tables']['properties'][1]['values'][0]['verdict'] = 'satisfied'
            else:
                bad['diagnostics'] = []
            with self.subTest(fault=fault), self.assertRaises(AssertionError):
                compare_scope(good if FAULT == 'oracle' else bad)

    def test_hand_diff_catches_missing_removed_and_false_satisfied(self):
        good = {'properties': [{'id': h.ID, 'from': 'satisfied', 'to': None,
                                'from_status': 'applied', 'to_status': 'not_applicable'}],
                'differences': [{'kind': 'contract_scope', 'target': 'Controls', 'change': 'removed'}]}
        diff_scope(good, 'removed')
        for fault in ('removed', 'verdict', 'status'):
            bad = copy.deepcopy(good)
            if fault == 'removed':
                bad['differences'] = []
            elif fault == 'verdict':
                bad['properties'][0]['to'] = 'satisfied'
            else:
                bad['properties'][0]['to_status'] = 'applied'
            with self.subTest(fault=fault), self.assertRaises(AssertionError):
                diff_scope(good if FAULT == 'oracle' else bad, 'removed')


class InventoryFault(unittest.TestCase):
    """Run only while production temporarily deletes an unexplained active row."""
    def test_unexplained_inventory_difference_is_rejected(self):
        out, value = invoke(h.scoped(), 'compare', ('--plans', 'Old,New', '--mode', 'exhaustive'))
        verdict(out, value, 'unsupported')
        diagnostic(value, 'U-SEM-0', 'report inventories differ')


class RecordedResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.cases = {}

    def startTest(self, test):
        self.cases[test.id()] = 'GREEN'
        super().startTest(test)

    def addFailure(self, test, err):
        self.cases[test.id()] = 'RED'
        super().addFailure(test, err)

    def addError(self, test, err):
        self.cases[test.id()] = 'ERROR'
        super().addError(test, err)

    def addSubTest(self, test, subtest, err):
        if err is not None:
            assert err[0] is not None
            self.cases[test.id()] = 'RED' if issubclass(err[0], test.failureException) else 'ERROR'
        super().addSubTest(test, subtest, err)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--record-dir', type=Path)
    parser.add_argument('--fault', choices=['timing', 'verdict', 'oracle', 'diagnostics'], default='')
    parser.add_argument('--inventory-fault', action='store_true')
    options, rest = parser.parse_known_args()
    BINARY = str(Path(options.binary).resolve())
    RECORD, FAULT = options.record_dir, options.fault
    if RECORD:
        RECORD.mkdir(parents=True, exist_ok=True)
    if options.inventory_fault:
        suite = unittest.defaultTestLoader.loadTestsFromTestCase(InventoryFault)
    elif rest:
        suite = unittest.defaultTestLoader.loadTestsFromNames(rest, sys.modules[__name__])
    else:
        suite = unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromTestCase(c) for c in
                                   (Legacy, Check, Static, Comparison, OtherCommands, Evidence, ClosedContest, ZeroRequests, PageAcceptance, OracleCalibration))
    result = unittest.TextTestRunner(verbosity=2, resultclass=RecordedResult).run(suite)
    assert isinstance(result, RecordedResult)
    for name, status in result.cases.items():
        print(f'CASE {name} {status}', flush=True)
    sys.exit(not result.wasSuccessful())
