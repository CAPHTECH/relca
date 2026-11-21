"""Selection regression: compare whole CLI outputs, not selected fields.

Only model name/hash are normalized. The control replaces the unselected loop
body while preserving terminal declaration order and all source byte spans.
"""
import argparse
from copy import deepcopy
import hashlib
import json
import unittest

import cli

PAIR = (cli.DATA / 'models/L2-pair.relca').read_text()
LOOP = PAIR[PAIR.index('  process Looped'):PAIR.index('  process Unrolled')]
L4 = (cli.DATA / 'models/L4.relca').read_text()
assert sum('verify probability' in line for line in L4.splitlines()) == 4
L4 = '\n'.join(line for line in L4.splitlines() if 'verify probability' not in line)
L4 = cli.replace(L4, '  bounds', '''  process U { wait 1min; end done }
  configuration Loop { cut C; process P }
  configuration Hand { cut C; process U }
  bounds''')


def without_loop(source):
    lines = LOOP.splitlines(keepends=True)
    replacements = {0: '  process Looped {', 1: '    end escalated', 2: '    end done',
                    len(lines) - 1: '  }'}
    control = []
    for i, line in enumerate(lines):
        text = replacements.get(i, '')
        assert len(text) <= len(line.rstrip('\n'))
        control.append(text.ljust(len(line.rstrip('\n'))) + ('\n' if line.endswith('\n') else ''))
    return cli.replace(source, LOOP, ''.join(control))


def normalized(value):
    value = deepcopy(value)

    def walk(v):
        if isinstance(v, dict):
            if isinstance(v.get('model'), dict):
                for key in ('name', 'sha256'):
                    v['model'].pop(key, None)
            for child in v.values():
                walk(child)
        elif isinstance(v, list):
            for child in v:
                walk(child)

    walk(value)
    return value


class Selection(unittest.TestCase):
    def test_contest_round_consumers_fail_closed(self):
        source = cli.model('scope-contest')
        atomic = cli.replace(source, '  kernel K', '  observed case { flag: bool = false }\n  kernel K')
        atomic = cli.replace(atomic, 'outcomes kernel K',
                             'writes flag; atomic { if rework.round >= 1 { flag = true; result ok } else { result denied } }')
        for source in (source, cli.replace(source, 'outcomes kernel K',
                       'requires rework.round >= 1 else denied; result nondet { ok }'), atomic):
            with self.subTest(source=source):
                out, value = cli.invoke(source, args=['--configuration', 'Hand'])
                cli.verdict(out, value, 'unsupported')
                self.assertTrue(any(d['code'] == 'U-SEM-0' and 'loop_feature_combination' in d['message']
                                    for d in value['diagnostics']))
        safe = cli.replace(cli.model('scope-contest'), 'outcomes kernel K', 'result nondet { ok }')
        out, value = cli.invoke(safe, args=['--configuration', 'Hand'])
        cli.verdict(out, value, 'violated')  # The independent reversal-authority defect remains.

    def test_l4_unused_global_round_kernel(self):
        out, value = cli.invoke(L4, args=['--configuration', 'Hand'])
        # Calibrated against model-wide round declarations before the fix.
        cli.verdict(out, value, 'satisfied')
        self.assertNotIn('repetition', value)
        self.assertNotIn('loop_control', out.stdout)
        self.assertNotIn('@loop:', out.stdout)

    def test_l4_round_kernel_consumed_outside_loop(self):
        source = cli.replace(L4, 'process U { wait 1min;', 'process U { outside = Rev.review();')
        out, value = cli.invoke(source, args=['--configuration', 'Hand'])
        cli.verdict(out, value, 'unsupported')
        errors = [d for d in value['diagnostics'] if d['severity'] == 'error']
        self.assertEqual([(d['code'], d['message']) for d in errors],
                         [('U-SEM-0', 'loop_round_out_of_scope')])

    def equal_to_control(self, source, command, args, verdict='satisfied', files=None):
        actual, value = cli.invoke(source, command, args, files)
        expected, control = cli.invoke(without_loop(source), command, args, files)
        for out, result in ((actual, value), (expected, control)):
            cli.verdict(out, result, verdict)
        self.assertEqual(actual.returncode, expected.returncode, value.get('diagnostics'))
        self.assertEqual(actual.stderr, expected.stderr)
        self.assertEqual(normalized(value), normalized(control))
        return value

    def test_check_hand(self):
        out, value = cli.invoke(PAIR, args=['--configuration', 'Hand'])
        cli.verdict(out, value, 'satisfied')
        self.assertNotIn('"rework"', out.stdout)
        self.assertNotIn('loop_control', out.stdout)
        value = self.equal_to_control(PAIR, 'check', ['--configuration', 'Hand'])
        self.assertEqual([r['end'] for r in value['agency']['viewpoints']['framing']['ends']],
                         ['escalated', 'done'])

    def test_only_selected_loop_survives_when_two_processes_repeat(self):
        start = PAIR.index('  process Unrolled')
        end = PAIR.index('  cut C')
        selected_loop = cli.replace(cli.replace(LOOP, 'process Looped', 'process Unrolled'),
                                    'repeat rework ', 'repeat selected ')
        source = cli.replace(PAIR, PAIR[start:end], selected_loop)
        value = self.equal_to_control(source, 'check', ['--configuration', 'Hand'])
        rows = value['repetition']['cases']
        self.assertEqual(len(rows), 1)
        self.assertEqual(rows[0]['loop_name'], 'selected')
        self.assertEqual(len(value['repetition']['sites']), 6)

    def test_nonloop_commands(self):
        triple = cli.replace(PAIR, '  metrics { window 60min; currency "JPY" }',
                              '  configuration Hand2 { cut C; process Unrolled }')
        for command, source, args, verdict in [
            ('diff', triple, ['--from', 'Hand', '--to', 'Hand2'], 'satisfied'),
            ('compare', triple, ['--plans', 'Hand,Hand2', '--mode', 'exhaustive'], 'satisfied'),
            ('mutate', PAIR, ['--configuration', 'Hand'], 'unsupported'),
            ('conform', PAIR, ['--configuration', 'Hand', '--log', '@events.jsonl', '--scope', 'event-contracts'], 'invalid'),
        ]:
            with self.subTest(command=command):
                self.equal_to_control(source, command, args, verdict,
                                      {'events.jsonl': ''} if command == 'conform' else None)

    def test_loop_mutation_counts_lexical_guards(self):
        out, value = cli.invoke(PAIR, 'mutate', ['--configuration', 'Loop', '--kinds', 'hold_delayed'])
        cli.verdict(out, value, 'violated')
        row, = (r for r in value['mutations'] if r['kind'] == 'hold_delayed')
        # Only retaining the fix branch has a call or a later call.
        self.assertEqual(row['candidates'], 1)
        mutant, = row['mutants']
        self.assertEqual(mutant['site']['retained_branch'], 'else')
        self.assertEqual(mutant['result'], 'undetected')

    def test_loop_diff_in_both_directions(self):
        for a, b in [('Loop', 'Hand'), ('Hand', 'Loop')]:
            out, value = cli.invoke(PAIR, 'diff', ['--from', a, '--to', b])
            cli.verdict(out, value, 'satisfied')
            row, = (r for r in value['differences'] if r['kind'] == 'repetition')
            self.assertEqual((row['target'], row['change']), ('rework', 'removed' if a == 'Loop' else 'added'))

    def test_loop_compare_in_both_pair_orders(self):
        for order in ('Loop,Hand', 'Hand,Loop'):
            with self.subTest(order=order):
                out, value = cli.invoke(PAIR, 'compare', ['--plans', order, '--mode', 'exhaustive'])
                cli.verdict(out, value, 'satisfied')
                row, = value['tables']['repetition']
                self.assertEqual((row['case_id'], row['loop']), (1, 'rework'))
                self.assertEqual([(c['configuration'], c['status']) for c in row['values']],
                                 [(p, 'reported' if p == 'Loop' else 'not_applicable') for p in order.split(',')])

    def test_loop_check_unchanged(self):
        out, value = cli.invoke(PAIR, args=['--configuration', 'Loop'])
        cli.verdict(out, value, 'satisfied')
        # Full current stdout+stderr+exit regression hash.
        observed = hashlib.sha256((out.stdout + out.stderr + str(out.returncode)).encode()).hexdigest()
        self.assertEqual(observed, 'b913dd8307cfbdbe6259051a59b01a12e07049283eb6fd05d28f5b401316f64d')

    def test_sensitivity_selection(self):
        # Keep the same manifest and relative filenames; only the model changes.
        manifest = json.dumps({
            'schema': 'relca-sensitivity/v1',
            'scope': {'kind': 'enumerated_models', 'rationale': 'selection regression'},
            'points': [{'id': 'base', 'model': 'point.relca', 'configuration': 'Hand', 'scenario': None}],
            'targets': [{'kind': 'probability_query', 'id': 'probability reached #1'}],
            'confidence': None,
        })
        source = cli.replace(PAIR, '  metrics { window 60min; currency "JPY" }', '')
        a, av = cli.invoke(manifest, 'sensitivity', ['--mode', 'exhaustive'], {'point.relca': source})
        b, bv = cli.invoke(manifest, 'sensitivity', ['--mode', 'exhaustive'], {'point.relca': without_loop(source)})
        for out, value in ((a, av), (b, bv)):
            cli.verdict(out, value, 'satisfied')
            self.assertEqual(len(value['points']), 1)
        self.assertEqual(a.returncode, b.returncode)
        self.assertEqual(a.stderr, b.stderr)
        # Sensitivity exposes the input hash in addition to model.sha256.
        for value in (av, bv):
            for point in value['points']:
                point.pop('source_sha256', None)
        self.assertEqual(normalized(av), normalized(bv))

        out, value = cli.invoke(manifest, 'sensitivity', ['--mode', 'exhaustive'], {'point.relca': L4})
        cli.verdict(out, value, 'unsupported')
        self.assertIn('loop_feature_combination', out.stdout)


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    args, rest = parser.parse_known_args()
    cli.BINARY = str(cli.Path(args.binary).resolve())
    unittest.main(argv=[__file__, *rest])
