"""ADR 0073 hand acceptance. No current output is an expected-value generator.

--record-dir records actual stdout/stderr/exit, including expected RED. --fault
injects at the test's input/wire seam, never changes production or saved inputs.
Calibration is not a claim that an unimplemented production mutation ran.
"""
import argparse
import copy
from fractions import Fraction
import gzip
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


def invoke(source, command='diff', args=None):
    global SERIAL
    SERIAL += 1
    if FAULT == 'timing':
        source = source.replace('duration fixed 1min', 'duration fixed 4min')
    with tempfile.TemporaryDirectory(prefix='intent-pairs-', dir=ROOT / '.tmp') as tmp:
        d = Path(tmp)
        (d / 'model.relca').write_text(source)
        entry = d / 'model.relca'
        if args is None:
            args = ['--from', 'A', '--to', 'B'] if command == 'diff' else ['--configuration', 'B']
        if command == 'sensitivity':
            entry = d / 'manifest.json'
            entry.write_text(json.dumps(h.manifest()))
            args = ['--mode', 'exhaustive']
        if command == 'conform':
            (d / 'events.jsonl').write_text(h.log())
            args = ['--configuration', 'B', '--log', str(d / 'events.jsonl'), '--scope', 'event-contracts']
        if command == 'verify':
            args = []
        out = subprocess.run([BINARY, command, str(entry), *args], cwd=ROOT, text=True, capture_output=True)
        assert out.stdout.endswith('\n') and out.stdout.count('\n') == 1, (command, out)
        v = json.loads(out.stdout)
        if RECORD:
            prefix = RECORD / f'{SERIAL:04d}-{command}'
            prefix.with_suffix('.relca').write_text(source)
            prefix.with_suffix('.stdout.json').write_text(out.stdout)
            prefix.with_suffix('.stderr').write_text(out.stderr)
            prefix.with_suffix('.exit').write_text(str(out.returncode) + '\n')
        if FAULT == 'drop-difference' and v.get('differences'):
            v['differences'] = []
        if FAULT == 'new-field':
            v['intent_block'] = None
        if FAULT == 'drop-diagnostics':
            v['diagnostics'] = []
        if FAULT == 'verify-backend' and command == 'verify':
            v['ledger']['backends']['external']['reason'] = 'connected'
        errors = [(d['code'], d['message']) for d in diagnostics(v) if d['severity'] == 'error']
        print(f'OBS {SERIAL:04d} {command} exit={out.returncode} verdict={v.get("verdict")} errors={errors}', flush=True)
        return out, v


def diagnostics(v):
    if isinstance(v, dict):
        yield from v.get('diagnostics', [])
        for key, child in v.items():
            if key != 'diagnostics':
                yield from diagnostics(child)
    elif isinstance(v, list):
        for child in v:
            yield from diagnostics(child)


def verdict(out, v, expected='satisfied'):
    assert out.returncode == EXITS[expected], (expected, out.returncode, out.stderr)
    assert v['verdict'] == expected, (expected, v['verdict'])


def diag(v, code, reason=None, recursive=False):
    matches = [d for d in (diagnostics(v) if recursive else v['diagnostics']) if d['code'] == code]
    assert matches, (code, list(diagnostics(v)))
    assert all(d['severity'] == ('warning' if code.startswith('W-') else 'error') for d in matches)
    if reason:
        assert any(re.search(reason, d['message'], re.I) for d in matches), (reason, matches)
    return matches


def warnings(v, w1, w2, w3):
    assert [sum(d['code'] == f'W-DIFF-{n}' for d in v['diagnostics']) for n in (1, 2, 3)] == [w1, w2, w3], v['diagnostics']


def key(row):
    return row['kind'], row['target'], row['change']


def rows(v):
    result = {key(r): r for r in v['differences']}
    assert len(result) == len(v['differences']), 'duplicate difference'
    return result


def swap_keys(old='Clerk', new='Bot', partial=False):
    answer = {('relation', new + '.check', 'added'), ('relation', old + '.check', 'removed'),
              ('authority', new + '.check', 'added'), ('authority', old + '.check', 'removed'),
              ('resource', new, 'added'), ('actor', new, 'added'),
              ('visibility', 'Doc', 'modified'), ('relation', 'v', 'modified')}
    if not partial:
        answer |= {('actor', old, 'removed'), ('resource', old, 'removed')}
    return answer


AUDIT = {('actor', 'Audit', 'added'), ('relation', 'Audit.log', 'added'),
         ('authority', 'Audit.log', 'added'), ('resource', 'Audit', 'added')}


def geometry(v, old='Clerk', partial=False, audit_reads=False):
    actual = rows(v)
    expected = swap_keys(old, partial=partial) | (AUDIT if partial else set())
    assert set(actual) == expected, (set(actual), expected)
    visibility = actual[('visibility', 'Doc', 'modified')]
    assert visibility['from'] == [old] and visibility['to'] == (['Audit', 'Bot'] if audit_reads else ['Bot'])
    for actor, person in [('Bot', 'op'), (old, 'clerk' if old == 'Clerk' else 'boss')]:
        direction = 'to' if actor == 'Bot' else 'from'
        change = 'added' if actor == 'Bot' else 'removed'
        a = actual[('authority', actor + '.check', change)]
        assert a[direction] == {'permission': [person], 'grants': [], 'authority_basis': [], 'responsibilities': []}
        r = actual.get(('resource', actor, change))
        if r:
            assert r[direction] == {'capacity': 1, 'executors': [person]}, r
    # Producer identity is observable in addition to the Capability set.
    producer = actual[('relation', 'v', 'modified')]
    assert producer['from'] == {'producer': old + '.check'}
    assert producer['to'] == {'producer': 'Bot.check'}


def coverage(v, expected, old='Clerk', reason='自動化', loses: str | None = '人の目視'):
    """Exact bidirectional correspondence, without trusting production Serde."""
    if FAULT == 'oracle-blind':
        return
    actual = rows(v)
    replacements = v['replacements']
    assert len(replacements) == 1, replacements
    r = replacements[0]
    assert set(r) == {'from', 'to', 'reason', 'loses', 'covers', 'span'}, r
    assert (r['from'], r['to'], r['reason'], r['loses']) == (old + '.check', 'Bot.check', reason, loses)
    assert isinstance(r['span'], dict) and r['span']['start'] < r['span']['end'], r
    covers = {key(c): c['via'] for c in r['covers']}
    assert len(covers) == len(r['covers']), 'duplicate cover'
    assert set(covers) == set(expected), (covers, expected)
    marked = {k for k, row in actual.items() if 'covered_by' in row}
    assert marked == set(covers), (marked, covers)
    for k, row in actual.items():
        if k not in covers:
            continue
        via = 'direct' if k in {('relation', old + '.check', 'removed'), ('relation', 'Bot.check', 'added')} else 'derived'
        assert covers[k] == via
        assert row['covered_by'] == [{'intent': 'replace', 'from': old + '.check', 'to': 'Bot.check', 'via': via}]
        # The adopted addendum keeps prose once, on the declaration, not rows.
        assert row['reason'] is None and row['loses'] is None


def no_new_fields(v):
    assert 'intent_block' not in v and 'replacements' not in v
    assert all('covered_by' not in row for row in v.get('differences', []))
    def check_unchecked(node):
        if isinstance(node, dict):
            assert 'intent' not in node.get('unchecked', [])
            for child in node.values():
                check_unchecked(child)
        elif isinstance(node, list):
            for child in node:
                check_unchecked(child)
    check_unchecked(v)


def terminal(v, time):
    found = [r for r in v['terminals'] if r['end'] == 'done']
    assert len(found) == 1
    assert found[0]['elapsed'] == {'min': time, 'max': time}, found
    assert found[0]['work'] == {'min': time, 'max': time}, found


def refuse(source, code, reason, command='check', args=None):
    out, v = invoke(source, command, args)
    verdict(out, v, 'invalid' if code.startswith('E-') else 'unsupported')
    diag(v, code, reason, recursive=True)
    assert code in out.stderr and re.search(reason, out.stderr, re.I), out.stderr
    return v


class Legacy(unittest.TestCase):
    def test_a1_old_geometry_and_counterfactual(self):
        source = h.model()
        out, v = invoke(source)
        verdict(out, v)
        geometry(v)
        warnings(v, 10, 0, 10)
        no_new_fields(v)
        for configuration, time in [('A', Fraction(2, 1) + Fraction(1, 1)), ('B', Fraction(1, 1) + Fraction(1, 1))]:
            out, value = invoke(source, 'check', ['--configuration', configuration])
            verdict(out, value)
            terminal(value, int(time))
        out, v = invoke(h.substrates()['a1-prime'])
        verdict(out, v)
        assert v['differences'] == []

    def test_a2_a3_old_partial_and_prime(self):
        for reads in (False, True):
            with self.subTest(audit_reads=reads):
                out, v = invoke(h.model(partial=True, audit_reads=reads))
                verdict(out, v)
                geometry(v, partial=True, audit_reads=reads)
                warnings(v, 12, 0, 12)
                source = h.substrates()['a3-prime' if reads else 'a2-prime']
                out, prime = invoke(source)
                verdict(out, prime)
                assert set(rows(prime)) == AUDIT | ({('visibility', 'Doc', 'modified')} if reads else set())

    def test_expanded_old_claims_and_wrong_partner(self):
        out, v = invoke(h.model(h.claims(), three=True))
        verdict(out, v)
        geometry(v)
        warnings(v, 0, 0, 8)
        no_new_fields(v)
        out, v = invoke(h.model(h.claims(), three=True), args=['--from', 'C', '--to', 'B'])
        verdict(out, v, 'invalid')
        assert len(diag(v, 'E-DIFF-0', 'no matching computed change')) == 4
        warnings(v, 4, 0, 10)
        out, v = invoke(h.model(h.claims('Robot', '移行'), three=True), args=['--from', 'C', '--to', 'B'])
        verdict(out, v)
        geometry(v, 'Robot')
        warnings(v, 0, 0, 8)
        assert {r['reason'] for r in v['differences']} == {'移行'}

    def test_many_to_one_substrate_and_simultaneous_prime(self):
        out, v = invoke(h.merged())
        verdict(out, v)
        assert set(rows(v)) == swap_keys() | swap_keys('Robot')
        warnings(v, 14, 0, 14)
        assert rows(v)[('visibility', 'Doc', 'modified')]['from'] == ['Clerk', 'Robot']
        out, v = invoke(h.substrates()['merged-prime'])
        verdict(out, v)
        assert v['differences'] == []

    def test_preserve_substrate_and_default_fallback(self):
        out, v = invoke(h.substrates()['selected-preserve'])
        verdict(out, v)
        assert [p['id'] for p in v['preservation']['properties']] == ['contract Selected #2 permitted']
        warnings(v, 10, 0, 0)
        body = 'retire Clerk.check from normal_path keep fallback'
        out, v = invoke(h.model(body), 'check')
        verdict(out, v)
        terminal(v, 2)
        still_called = h.model(body).replace('v = Bot.check();', 'extra = Clerk.check(); v = Bot.check();')
        refuse(still_called, 'E-DIFF-0', 'still called on the normal path')

    def test_from_values_leave_modified_authority_in_prime(self):
        out, v = invoke(h.scoped_authority())
        verdict(out, v)
        assert set(rows(v)) == swap_keys() | {('contract_scope', 'Scoped', 'added')}
        out, v = invoke(h.scoped_authority(prime=True))
        verdict(out, v)
        assert set(rows(v)) == {('authority', 'Bot.check', 'modified'), ('contract_scope', 'Scoped', 'added')}
        r = rows(v)[('authority', 'Bot.check', 'modified')]
        assert r['from']['authority_basis'] == []
        assert r['to']['authority_basis'] == [{'id': 'contract Scoped #1 authority_basis', 'authority': 'Grant'}]

    def test_context_words_remain_ordinary_names(self):
        for word in ['replace', 'with', 'intent', 'from', 'to']:
            for place in ['actor', 'capability', 'binding', 'configuration', 'contract', 'process', 'cut', 'person']:
                with self.subTest(word=word, place=place):
                    source, configuration = h.model(), 'B'
                    if place == 'actor':
                        source = source.replace('Bot', word)
                    elif place == 'capability':
                        source = source.replace('check', word)
                    elif place == 'binding':
                        source = source.replace('v = ', word + ' = ').replace('post(v)', 'post(' + word + ')')
                    elif place == 'configuration':
                        source = source.replace('configuration B ', 'configuration ' + word + ' ')
                        configuration = word
                    elif place == 'contract':
                        source = h.extra(source, f'contract {word} {{ permitted Desk.post }}')
                    elif place == 'process':
                        source = source.replace('PB', word)
                    elif place == 'cut':
                        source = source.replace('cut K', 'cut ' + word)
                    else:
                        source = re.sub(r'\bop\b', word, source)
                    out, v = invoke(source, 'check', ['--configuration', configuration])
                    verdict(out, v)
                    terminal(v, 2)

    def test_global_preserve_keeps_warning_one(self):
        source = h.extra(h.model('preserve Ctl'), 'contract Ctl { permitted Desk.post }')
        out, v = invoke(source)
        verdict(out, v)
        warnings(v, 10, 0, 0)
        assert v['preservation']['summary']['satisfied'] == 1
        no_new_fields(v)

    def test_old_diff_golden_bytes(self):
        # Reuse existing frozen bytes only for compatibility, never for meaning.
        spec = importlib.util.spec_from_file_location('snapshots', ROOT / 'scripts/snapshots.py')
        assert spec is not None and spec.loader is not None
        snapshots = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(snapshots)
        count = 0
        for directory in sorted((ROOT / 'fixtures').iterdir()):
            path = directory / 'invocation.json'
            if not path.is_file():
                continue
            invocation = json.loads(path.read_text())
            if invocation.get('command') != 'diff':
                continue
            job = {'id': 'fixture-' + directory.name,
                   'args': snapshots.invocation((directory / 'model.relca').relative_to(ROOT).as_posix(), invocation)}
            saved = ROOT / 'crates/relca-cli/tests/snapshots' / (job['id'] + '.snap.gz')
            expected = snapshots.unpack(gzip.decompress(saved.read_bytes()))
            actual = snapshots.capture(Path(BINARY), job)
            if FAULT == 'golden':
                actual = (actual[0] + b' ', actual[1], actual[2])
            assert expected == actual, '\n'.join(snapshots.compare(job, expected, actual))
            count += 1
        assert count > 0
        print(f'COMPAT {count} existing diff goldens exact stdout/stderr/exit', flush=True)

    def test_legacy_entrypoint_values(self):
        for command in ['check', 'explain', 'simulate', 'mutate', 'sensitivity', 'conform', 'verify']:
            with self.subTest(command=command):
                out, v = invoke(h.command_model(command, 'change visibility Doc reason "既定"'), command,
                                ['--configuration', 'B', '--kinds', 'authority_expired'] if command == 'mutate' else None)
                entry_values(command, out, v)
                no_new_fields(v)


def entry_values(command, out, v):
    if command == 'verify':
        verdict(out, v, 'unsupported')
        assert 'external backend is not connected' in out.stderr
        assert v['ledger']['backends']['external']['reason'] == 'external_backend_not_connected'
    elif command == 'mutate':
        verdict(out, v, 'vacuous')
        assert v['run']['reason'] == 'no_selected_mutant'
        terminal(v['original'], 2)
    elif command == 'simulate':
        verdict(out, v)
        assert v['case_outcomes'][0]['terminals'] == [{'kind': 'end', 'end': 'done', 'count': 4}]
    elif command == 'sensitivity':
        verdict(out, v)
        assert len(v['points']) == 2
        for point in v['points']:
            p = next(p for p in point['report']['properties'] if p['id'] == 'probability reached #1')
            assert p['probability'] == {'min': '1', 'max': '1'}
            terminal(point['report'], 2)
    elif command == 'conform':
        verdict(out, v)
        assert any(c['conformance'] == 'conformant' for c in v['checks'])
    else:
        verdict(out, v)
        terminal(v, 2)
        if command == 'explain':
            assert 'explanation' in v


class Pairs(unittest.TestCase):
    def test_a5_explicit_old_items_are_partner_specific(self):
        source = h.model(pairs=h.pair(h.claims()) + '\n' + h.pair(h.claims('Robot', '移行'), 'C'), three=True)
        for from_, reason in [('A', '自動化'), ('C', '移行')]:
            with self.subTest(from_=from_):
                out, v = invoke(source, args=['--from', from_, '--to', 'B'])
                verdict(out, v)
                geometry(v, 'Clerk' if from_ == 'A' else 'Robot')
                warnings(v, 0, 0, 8)
                assert v['intent_block'] == {'from': from_, 'to': 'B'}
                assert {r['reason'] for r in v['differences']} == {reason}
                assert 'replacements' not in v and all('covered_by' not in r for r in v['differences'])

    def test_a5_no_block_and_reverse_never_reuse_reason(self):
        source = h.model(pairs=h.pair(h.claims()), three=True)
        for from_, to in [('A', 'C'), ('B', 'A'), ('C', 'B')]:
            with self.subTest(from_=from_, to=to):
                out, v = invoke(source, args=['--from', from_, '--to', to])
                verdict(out, v)
                warnings(v, 10, 0, 10)
                assert v['intent_block'] is None
                assert all(r['reason'] is None for r in v['differences'])

    def test_empty_pair_excludes_default_and_fallback_uses_default(self):
        source = h.model('change visibility Doc reason "既定"', h.pair(''), three=True)
        for from_, block, count, reason in [('A', {'from': 'A', 'to': 'B'}, 10, None),
                                            ('C', {'from': None, 'to': 'B'}, 9, '既定')]:
            with self.subTest(from_=from_):
                out, v = invoke(source, args=['--from', from_, '--to', 'B'])
                verdict(out, v)
                assert v['intent_block'] == block
                warnings(v, count, 0, 10)
                assert rows(v)[('visibility', 'Doc', 'modified')]['reason'] == reason

    def test_selected_preserve_only_and_destination_binding(self):
        source = h.extra(h.model('preserve Default', h.pair('preserve Selected'), three=True),
                         'contract Default { permitted Desk.post }\ncontract Selected { permitted Desk.post }')
        out, v = invoke(source)
        verdict(out, v)
        assert [p['id'] for p in v['preservation']['properties']] == ['contract Selected #2 permitted']
        warnings(v, 10, 0, 0)
        assert all([a['selector'] for a in r['annotations']] == ['Selected'] for r in v['differences'])
        source = h.model(pairs=h.pair('change relation fresh reason "new binding"')).replace(
            'v = Bot.check(); p = Desk.post(v)', 'fresh = Bot.check(); p = Desk.post(fresh)')
        out, v = invoke(source, 'check', ['--configuration', 'A'])
        verdict(out, v)

    def test_compare_discloses_default_and_pair_but_not_absence(self):
        for default, pair, present in [('change visibility Doc reason "既定"', '', True),
                                       ('', h.pair(''), True), ('', '', False)]:
            with self.subTest(default=default, pair=pair):
                out, v = invoke(h.model(default, pair, three=True), 'compare', ['--plans', 'A,B,C', '--mode', 'exhaustive'])
                verdict(out, v)
                assert v['unchecked'].count('intent') == int(present)
                assert 'differences' in v['unchecked']

    def test_default_unmatched_guidance_only_three_or_more_configurations(self):
        for three in (False, True):
            with self.subTest(three=three):
                out, v = invoke(h.model('add Clerk reason "not added"', three=three))
                verdict(out, v, 'invalid')
                found = diag(v, 'E-DIFF-0', 'no matching computed change')
                assert any('intent from A to B' in d['message'] for d in found) == three


class Replace(unittest.TestCase):
    def test_a1_ten_rows_direct_and_derived(self):
        out, v = invoke(h.model(h.REPLACE))
        verdict(out, v)
        geometry(v)
        coverage(v, swap_keys())
        warnings(v, 0, 0, 0)
        assert 'intent_block' not in v

    def test_a1b_missing_loses_keeps_eight_derived_warnings(self):
        out, v = invoke(h.model(h.REPLACE.replace(' loses "人の目視"', '')))
        verdict(out, v)
        coverage(v, swap_keys(), loses=None)
        warnings(v, 0, 1, 8)
        diag(v, 'W-DIFF-2', r'replace Clerk\.check with Bot\.check does not record loses')
        messages = [d['message'] for d in v['diagnostics'] if d['code'] == 'W-DIFF-3']
        assert not any('relation Clerk.check ' in m or 'relation Bot.check ' in m for m in messages)

    def test_a2_unrelated_audit_stays_uncovered(self):
        out, v = invoke(h.model(h.REPLACE, partial=True))
        verdict(out, v)
        geometry(v, partial=True)
        coverage(v, swap_keys(partial=True))
        warnings(v, 4, 0, 4)
        assert all(rows(v)[k]['reason'] is None for k in AUDIT)

    def test_a3_extra_reader_stays_uncovered(self):
        out, v = invoke(h.model(h.REPLACE, partial=True, audit_reads=True))
        verdict(out, v)
        geometry(v, partial=True, audit_reads=True)
        coverage(v, swap_keys(partial=True) - {('visibility', 'Doc', 'modified')})
        warnings(v, 5, 0, 5)
        diag(v, 'W-DIFF-1', r'visibility Doc has no intent')
        assert rows(v)[('visibility', 'Doc', 'modified')]['reason'] is None

    def test_individual_claim_wins_independent_of_order(self):
        individual = 'change visibility Doc reason "個別" loses "個別の損失"'
        for body in [h.REPLACE + '; ' + individual, individual + '; ' + h.REPLACE]:
            with self.subTest(body=body):
                out, v = invoke(h.model(body))
                verdict(out, v)
                coverage(v, swap_keys() - {('visibility', 'Doc', 'modified')})
                row = rows(v)[('visibility', 'Doc', 'modified')]
                assert (row['reason'], row['loses']) == ('個別', '個別の損失')
                warnings(v, 0, 0, 0)

    def test_a4_mismatch_refused_and_loses_still_warned(self):
        v = refuse(h.model('replace Desk.post with Bot.check reason "x"'), 'E-DIFF-0', r'Desk\.post|matching|removed', 'diff')
        diag(v, 'W-DIFF-2', 'does not record loses')
        assert all('covered_by' not in r for r in v['differences'])

    def test_a5_replace_reasons_do_not_leak_or_merge_defaults(self):
        source = h.model('change visibility Doc reason "既定"',
                         h.pair(h.REPLACE) + '\n' + h.pair('replace Robot.check with Bot.check reason "移行" loses "保守契約"', 'C'),
                         three=True)
        for from_, old, reason, loss in [('A', 'Clerk', '自動化', '人の目視'), ('C', 'Robot', '移行', '保守契約')]:
            with self.subTest(from_=from_):
                out, v = invoke(source, args=['--from', from_, '--to', 'B'])
                verdict(out, v)
                coverage(v, swap_keys(old), old, reason, loss)
                warnings(v, 0, 0, 0)
                assert v['intent_block'] == {'from': from_, 'to': 'B'}
        out, v = invoke(source, args=['--from', 'A', '--to', 'C'])
        verdict(out, v)
        warnings(v, 10, 0, 10)
        assert v['intent_block'] is None and not v.get('replacements')
        assert all('covered_by' not in r and r['reason'] is None for r in v['differences'])

    def test_many_to_one_simultaneous_projection_and_order_independence(self):
        second = 'replace Robot.check with Bot.check reason "自動化" loses "人の目視"'
        # 14 distinct rows; 19 (row,declaration) pairs. Doc needs both changes.
        expected_keys = swap_keys() | swap_keys('Robot')
        expected_covers = {'Clerk.check': swap_keys(),
                           'Robot.check': swap_keys('Robot') - {('relation', 'v', 'modified')}}
        previous = None
        for body in [h.REPLACE + '; ' + second, second + '; ' + h.REPLACE]:
            with self.subTest(body=body):
                out, v = invoke(h.merged(body))
                verdict(out, v)
                assert set(rows(v)) == expected_keys
                warnings(v, 0, 0, 0)
                multi_coverage(v, expected_covers)
                current = {k: r['covered_by'] for k, r in rows(v).items()}
                if previous is not None:
                    assert current == previous
                previous = current

    def test_many_to_one_distinct_reasons_preserve_whole_output_order(self):
        second = 'replace Robot.check with Bot.check reason "移行" loses "保守契約"'
        expected = {'Clerk.check': swap_keys(),
                    'Robot.check': swap_keys('Robot') - {('relation', 'v', 'modified')}}
        reasons = {'Clerk.check': ('自動化', '人の目視'), 'Robot.check': ('移行', '保守契約')}
        previous = None
        for body in [h.REPLACE + '; ' + second, second + '; ' + h.REPLACE]:
            with self.subTest(body=body):
                source = h.merged(body)
                out, v = invoke(source)
                verdict(out, v)
                assert set(rows(v)) == swap_keys() | swap_keys('Robot')
                warnings(v, 0, 0, 0)
                multi_coverage(v, expected, reasons)
                sha256 = hashlib.sha256(source.encode()).hexdigest()
                assert v['from']['report']['model']['sha256'] == v['to']['report']['model']['sha256'] == sha256
                for r in v['replacements']:
                    text = source.encode()[r['span']['start']:r['span']['end']].decode()
                    assert text.startswith('replace ' + r['from'] + ' with ' + r['to'])
                    assert r['reason'] in text and r['loses'] in text
                normalized = without_order_provenance(v)
                if previous is not None:
                    assert normalized == previous
                previous = normalized

    def test_from_values_do_not_cover_added_to_modified_authority(self):
        out, v = invoke(h.scoped_authority(h.REPLACE))
        verdict(out, v)
        assert set(rows(v)) == swap_keys() | {('contract_scope', 'Scoped', 'added')}
        coverage(v, swap_keys() - {('authority', 'Bot.check', 'added')})
        warnings(v, 2, 0, 2)
        r = rows(v)[('authority', 'Bot.check', 'added')]
        assert r['from'] is None and r['reason'] is None
        assert r['to']['authority_basis'] == [{'id': 'contract Scoped #1 authority_basis', 'authority': 'Grant'}]
        diag(v, 'W-DIFF-1', r'authority Bot\.check has no intent')


def static_case(source, code, reason):
    def test(self):
        # Select A deliberately: unselected blocks must be checked as well.
        refuse(source(), code, reason, args=['--configuration', 'A'])
    return test


PAIR_STATIC = {
    'duplicate_pair': (lambda: h.model(pairs=h.pair('') + h.pair('')), 'E-SYN-0', r'duplicate|twice|already'),
    'self_pair': (lambda: h.model(pairs=h.pair('', 'B', 'B')), 'E-CON-0', r'same|self|distinct|different'),
    'unknown_from': (lambda: h.model(pairs=h.pair('', 'Z')), 'E-NAME-0', r'configuration|\bZ\b'),
    'unknown_to': (lambda: h.model(pairs=h.pair('', 'A', 'Z')), 'E-NAME-0', r'configuration|\bZ\b'),
    'keep_fallback': (lambda: h.model(pairs=h.pair('retire Clerk.check from normal_path keep fallback')), 'U-SYN-0', r'fallback'),
    'unknown_target': (lambda: h.model(pairs=h.pair('change relation Unknown.run reason "x"')), 'E-NAME-0', r'intent target|Unknown'),
    'wrong_namespace': (lambda: h.model(pairs=h.pair('change visibility Bot reason "x"')), 'E-CON-0', r'namespace|intent target|source'),
    'duplicate_target': (lambda: h.model(pairs=h.pair('add Bot reason "x"; add Bot reason "y"')), 'U-SEM-0', r'duplicate intent target'),
    'unknown_selector': (lambda: h.model(pairs=h.pair('preserve Missing')), 'E-NAME-0', r'preservation selector|Missing'),
    'destination_not_source_binding': (lambda: h.model(pairs=h.pair('change relation only_from reason "x"')).replace('v = Clerk.check(); p = Desk.post(v)', 'only_from = Clerk.check(); p = Desk.post(only_from)'), 'E-NAME-0', r'intent target|only_from'),
}
for name, values in PAIR_STATIC.items():
    setattr(Pairs, 'test_static_' + name, static_case(*values))


REPLACE_STATIC = {
    'unknown_from_capability': ('replace Missing.check with Bot.check reason "x"', 'E-NAME-0', r'Missing|undeclared|unknown'),
    'unknown_to_capability': ('replace Clerk.check with Bot.missing reason "x"', 'E-NAME-0', r'missing|undeclared|unknown'),
    'same_capability': ('replace Clerk.check with Clerk.check reason "x"', 'E-CON-0', r'same|self|distinct|different'),
    'duplicate_source': (h.REPLACE + '; ' + h.REPLACE, 'U-SEM-0', r'duplicate intent target'),
    'overlapping_retire': (h.REPLACE + '; retire Clerk.check reason "x"', 'U-SEM-0', r'duplicate|overlap|claim'),
    'overlapping_add': (h.REPLACE + '; add Bot.check reason "x"', 'U-SEM-0', r'duplicate|overlap|claim'),
}
for name, (body, code, reason) in REPLACE_STATIC.items():
    setattr(Replace, 'test_static_' + name, static_case(lambda body=body: h.model(body), code, reason))


def partial_replacement_test(mode):
    def test(self):
        source = h.model(h.REPLACE)
        if mode == 'old_remains':
            source = source.replace('v = Bot.check();', 'left = Clerk.check(); v = Bot.check();')
        elif mode == 'new_already_active':
            source = source.replace('v = Clerk.check();', 'already = Bot.check(); v = Clerk.check();')
        else:
            source = h.model(h.REPLACE + '; replace Bot.check with Robot.check reason "chain" loses "chain loss"', three=True)
        refuse(source, 'E-DIFF-0', r'matching|added|removed|replace', 'diff')
    return test


for name in ['old_remains', 'new_already_active', 'chain']:
    setattr(Replace, 'test_partial_refusal_' + name, partial_replacement_test(name))


UNRELATED = {'contract_scope': [('contract_scope', 'Ctl', 'added')],
             'meaning': [('meaning', 'end.done', 'removed'), ('meaning', 'end.other', 'added')],
             'time': [('time', 'Desk.post', 'modified')],
             'contest': [('contest', 'end.other', 'added')],
             'rule': [('rule', 'Policy', 'modified')],
             'qualification': [('qualification', 'Clerk.check', 'removed'), ('qualification', 'Bot.check', 'added')],
             'performance': [('performance', 'Clerk.check', 'removed'), ('performance', 'Bot.check', 'added')],
             'degradation': [('degradation', 'Manual', 'modified')]}


def unrelated_test(kind, replace):
    def test(self):
        source = h.unrelated(kind, h.REPLACE if replace else '')
        out, v = invoke(source)
        verdict(out, v, 'violated' if kind == 'contest' else 'satisfied')
        actual = rows(v)
        extras = set(UNRELATED[kind])
        if kind == 'contest':
            extras |= {('meaning', 'end.done', 'removed'), ('meaning', 'end.other', 'added'),
                       ('authority', 'Review.review', 'added'), ('resource', 'Review', 'added')}
        assert set(actual) == swap_keys() | extras
        for k in UNRELATED[kind]:
            assert k in actual, (k, actual.keys())
            assert 'covered_by' not in actual[k] and actual[k]['reason'] is None
            target = k[1]
            diag(v, 'W-DIFF-1', re.escape(target) + r' has no intent')
        if replace:
            coverage(v, swap_keys())
            warnings(v, len(extras), 0, len(extras))
        else:
            warnings(v, 10 + len(extras), 0, 10 + len(extras))
    return test


for kind in UNRELATED:
    setattr(Legacy, 'test_unrelated_substrate_' + kind, unrelated_test(kind, False))
    setattr(Replace, 'test_never_covers_' + kind, unrelated_test(kind, True))


def entry_test(command, stage):
    def test(self):
        body = 'change visibility Doc reason "pair"' if stage == 'pairs' else h.REPLACE
        source = h.command_model(command, pairs=h.pair(body))
        out, v = invoke(source, command, ['--configuration', 'B', '--kinds', 'authority_expired'] if command == 'mutate' else None)
        entry_values(command, out, v)
        no_new_fields(v)
        if command == 'verify':
            old, _ = invoke(h.command_model(command, 'change visibility Doc reason "既定"'), command)
            assert (out.stdout, out.stderr, out.returncode) == (old.stdout, old.stderr, old.returncode)
    return test


def fallback_entry_test(command):
    def test(self):
        source = h.command_model(command, pairs=h.pair('retire Clerk.check from normal_path keep fallback'))
        args = ['--plans', 'A,B,C', '--mode', 'exhaustive'] if command == 'compare' else None
        if command == 'mutate':
            args = ['--configuration', 'B', '--kinds', 'authority_expired']
        refuse(source, 'U-SYN-0', 'fallback', command, args)
    return test


for command in ['check', 'explain', 'simulate', 'mutate', 'sensitivity', 'conform', 'verify']:
    setattr(Pairs, 'test_entry_' + command, entry_test(command, 'pairs'))
    setattr(Replace, 'test_entry_' + command, entry_test(command, 'replace'))
for command in ['check', 'explain', 'simulate', 'mutate', 'sensitivity', 'conform', 'compare', 'diff']:
    setattr(Pairs, 'test_fallback_refused_' + command, fallback_entry_test(command))


def multi_coverage(v, expected_covers, reasons=None):
    if FAULT == 'oracle-blind':
        return
    actual_rows = rows(v)
    assert [r['from'] for r in v['replacements']] == sorted(expected_covers)
    replacements = {r['from']: r for r in v['replacements']}
    assert len(replacements) == len(v['replacements']) == len(expected_covers)
    wanted_memberships = set()
    seen_memberships = set()
    for old, expected in expected_covers.items():
        r = replacements[old]
        reason, loses = (reasons or {}).get(old, ('自動化', '人の目視'))
        assert (r['to'], r['reason'], r['loses']) == ('Bot.check', reason, loses)
        covers = {key(c): c['via'] for c in r['covers']}
        assert len(covers) == len(r['covers']) and set(covers) == expected
        for k in expected:
            via = 'direct' if k in {('relation', old, 'removed'), ('relation', 'Bot.check', 'added')} else 'derived'
            assert covers[k] == via
            wanted_memberships.add((k, old, via))
    for k, row in actual_rows.items():
        marks = row.get('covered_by', [])
        assert [m['from'] for m in marks] == sorted({m['from'] for m in marks})
        for mark in marks:
            assert set(mark) == {'intent', 'from', 'to', 'via'}
            assert (mark['intent'], mark['to']) == ('replace', 'Bot.check')
            seen_memberships.add((k, mark['from'], mark['via']))
        if marks:
            assert row['reason'] is None and row['loses'] is None
    assert seen_memberships == wanted_memberships


# Exact JSON pointers excluded ONLY from declaration-order comparison. No
# ledger pointer is excluded: the current ledger links use stable table paths,
# not source positions. New location-dependent links require a reviewed change.
ORDER_PROVENANCE = (
    '/from/report/model/sha256',
    '/to/report/model/sha256',
    '/replacements/0/span',
    '/replacements/1/span',
)


def without_order_provenance(v):
    if FAULT == 'oracle-blind':
        return {}
    result = copy.deepcopy(v)
    for pointer in ORDER_PROVENANCE:
        parts = pointer.strip('/').split('/')
        node = result
        for part in parts[:-1]:
            node = node[int(part)] if isinstance(node, list) else node[part]
        assert parts[-1] in node, pointer
        del node[parts[-1]]
    return result


def hand_report(partial=False, audit_reads=False):
    """Hand wire for testing the oracle, not a surrogate production pass."""
    all_keys = swap_keys(partial=partial) | (AUDIT if partial else set())
    covered = swap_keys(partial=partial) - ({('visibility', 'Doc', 'modified')} if audit_reads else set())
    report = {'differences': [], 'replacements': [{'from': 'Clerk.check', 'to': 'Bot.check', 'reason': '自動化',
               'loses': '人の目視', 'span': {'start': 1, 'end': 2}, 'covers': []}]}
    for k in sorted(all_keys):
        row: dict = dict(zip(('kind', 'target', 'change'), k), reason=None, loses=None)
        if k in covered:
            via = 'direct' if k in {('relation', 'Clerk.check', 'removed'), ('relation', 'Bot.check', 'added')} else 'derived'
            row['covered_by'] = [{'intent': 'replace', 'from': 'Clerk.check', 'to': 'Bot.check', 'via': via}]
            report['replacements'][0]['covers'].append(dict(zip(('kind', 'target', 'change'), k), via=via))
        report['differences'].append(row)
    return report, covered


class Calibration(unittest.TestCase):
    def test_order_comparison_has_only_four_explicit_exclusions(self):
        good = {'from': {'report': {'model': {'sha256': 'a', 'name': 'Swap'}}},
                'to': {'report': {'model': {'sha256': 'a', 'name': 'Swap'}}},
                'replacements': [{'span': {'start': 1, 'end': 2}, 'reason': 'first'},
                                 {'span': {'start': 3, 'end': 4}, 'reason': 'second'}],
                'ledger': {'subjects': []}, 'span': 'must not be erased'}
        changed = copy.deepcopy(good)
        changed['from']['report']['model']['sha256'] = 'b'
        changed['to']['report']['model']['sha256'] = 'b'
        changed['replacements'][0]['span'] = {'start': 10, 'end': 20}
        changed['replacements'][1]['span'] = {'start': 30, 'end': 40}
        assert without_order_provenance(good) == without_order_provenance(changed)
        for field in ['reason', 'name', 'ledger', 'span', 'order']:
            bad = copy.deepcopy(good)
            if field == 'reason':
                bad['replacements'][0]['reason'] = 'lost reason'
            elif field == 'name':
                bad['from']['report']['model']['name'] = 'Other'
            elif field == 'ledger':
                bad['ledger']['subjects'] = ['lost evidence']
            elif field == 'order':
                bad['replacements'].reverse()
            else:
                bad['span'] = 'unexpected change'
            assert without_order_provenance(good) != without_order_provenance(bad), field

    def test_many_to_one_membership_bijection_and_sort(self):
        expected = {'Clerk.check': swap_keys(),
                    'Robot.check': swap_keys('Robot') - {('relation', 'v', 'modified')}}
        good = {'differences': [], 'replacements': []}
        for old, keys in expected.items():
            covers = []
            for k in sorted(keys):
                via = 'direct' if k in {('relation', old, 'removed'), ('relation', 'Bot.check', 'added')} else 'derived'
                covers.append(dict(zip(('kind', 'target', 'change'), k), via=via))
            good['replacements'].append({'from': old, 'to': 'Bot.check', 'reason': '自動化', 'loses': '人の目視', 'covers': covers})
        for k in sorted(swap_keys() | swap_keys('Robot')):
            marks = []
            for r in good['replacements']:
                for c in r['covers']:
                    if key(c) == k:
                        marks.append({'intent': 'replace', 'from': r['from'], 'to': 'Bot.check', 'via': c['via']})
            good['differences'].append(dict(zip(('kind', 'target', 'change'), k), reason=None, loses=None, covered_by=marks))
        multi_coverage(good, expected)
        for fault in ['missing-member', 'missing-cover', 'sort', 'via']:
            with self.subTest(fault=fault):
                broken = copy.deepcopy(good)
                shared = next(r for r in broken['differences'] if key(r) == ('relation', 'Bot.check', 'added'))
                if fault == 'missing-member':
                    shared['covered_by'].pop()
                elif fault == 'missing-cover':
                    broken['replacements'][0]['covers'].pop()
                elif fault == 'sort':
                    shared['covered_by'].reverse()
                else:
                    shared['covered_by'][0]['via'] = 'derived'
                with self.assertRaises(AssertionError):
                    multi_coverage(broken, expected)

    def test_a1b_a4b_warning_and_matching_faults_are_rejected(self):
        if FAULT == 'oracle-blind':
            return self.fail('calibration oracle disabled')
        warnings({'diagnostics': [{'code': 'W-DIFF-2'}] + [{'code': 'W-DIFF-3'}] * 8}, 0, 1, 8)
        with self.assertRaises(AssertionError):
            warnings({'diagnostics': [{'code': 'W-DIFF-2'}]}, 0, 1, 8)
        broken = subprocess.CompletedProcess([], 0, '', '')
        with self.assertRaises(AssertionError):
            verdict(broken, {'verdict': 'satisfied'}, 'invalid')

    def test_a5b_wrong_partner_and_a5c_default_reason_are_rejected(self):
        good, expected = hand_report()
        coverage(good, expected)
        for fault in ['wrong-partner', 'merged-default']:
            with self.subTest(fault=fault):
                broken = copy.deepcopy(good)
                if fault == 'wrong-partner':
                    broken['replacements'][0]['from'] = 'Robot.check'
                else:
                    next(r for r in broken['differences'] if r['kind'] == 'visibility')['reason'] = '既定'
                with self.assertRaises(AssertionError):
                    coverage(broken, expected)

    def test_a2b_wildcard_extra_audit_is_rejected(self):
        good, expected = hand_report(partial=True)
        coverage(good, expected)
        broken = copy.deepcopy(good)
        row = next(r for r in broken['differences'] if key(r) == ('actor', 'Audit', 'added'))
        row['covered_by'] = [{'intent': 'replace', 'from': 'Clerk.check', 'to': 'Bot.check', 'via': 'derived'}]
        with self.assertRaises(AssertionError):
            coverage(broken, expected)

    def test_a3b_partial_reader_overcoverage_is_rejected(self):
        good, expected = hand_report(partial=True, audit_reads=True)
        coverage(good, expected)
        broken = copy.deepcopy(good)
        k = ('visibility', 'Doc', 'modified')
        broken['replacements'][0]['covers'].append(dict(zip(('kind', 'target', 'change'), k), via='derived'))
        with self.assertRaises(AssertionError):
            coverage(broken, expected)

    def test_cover_bijection_via_and_individual_priority(self):
        good, expected = hand_report()
        coverage(good, expected)
        for fault in ['missing-cover', 'missing-mark', 'via', 'wrong-reason', 'duplicate', 'individual']:
            with self.subTest(fault=fault):
                broken = copy.deepcopy(good)
                if fault == 'missing-cover':
                    broken['replacements'][0]['covers'].pop()
                elif fault == 'missing-mark':
                    broken['differences'][0].pop('covered_by')
                elif fault == 'via':
                    broken['differences'][0]['covered_by'][0]['via'] = 'direct'
                elif fault == 'wrong-reason':
                    broken['differences'][0]['reason'] = '既定'
                elif fault == 'duplicate':
                    broken['replacements'][0]['covers'].append(broken['replacements'][0]['covers'][0])
                else:
                    k = ('visibility', 'Doc', 'modified')
                    expected = expected - {k}
                with self.assertRaises(AssertionError):
                    coverage(broken, expected)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    parser.add_argument('--stage', choices=['all', 'legacy', 'pairs', 'replace', 'calibration'], default='all')
    parser.add_argument('--record-dir', type=Path)
    parser.add_argument('--fault', default='', choices=['', 'timing', 'drop-difference', 'new-field', 'drop-diagnostics', 'golden', 'verify-backend', 'oracle-blind'])
    parser.add_argument('--test', help='unittest method substring')
    args = parser.parse_args()
    BINARY, RECORD, FAULT = str(Path(args.binary).resolve()), args.record_dir, args.fault
    if RECORD:
        RECORD.mkdir(parents=True, exist_ok=True)
    classes = {'legacy': Legacy, 'pairs': Pairs, 'replace': Replace, 'calibration': Calibration}
    suite = unittest.TestSuite()
    for stage, cls in classes.items():
        if args.stage not in ('all', stage):
            continue
        for name in unittest.defaultTestLoader.getTestCaseNames(cls):
            if args.test is None or args.test in name:
                suite.addTest(cls(name))
    assert suite.countTestCases() > 0
    result = unittest.TextTestRunner(verbosity=2).run(suite)
    sys.exit(not result.wasSuccessful())
