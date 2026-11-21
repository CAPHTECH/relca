"""Public compatibility witnesses for Agency's repeat expansion budget.

The digest covers exactly (stdout, stderr, exit), using the snapshot framing.
This is a current-output regression, not the semantic oracle in repeat_agency.md.
"""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from tempfile import TemporaryDirectory

import cli

HEADER = '''model X version "0.2" {
  type Entry = enum { entry }
  type Part = enum { part }
  type V = enum { pass, fail }
  observed case { reviewed: optional<V> }
  actor Invoice { source document }
  actor Policy { source document }
  actor Reader { capacity 1; capability enter() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
  actor DevA { capacity 1; capability work(e: Entry) -> Part { duration fixed 2min; result nondet { part } } }
  actor DevB { capacity 1; capability work(e: Entry) -> Part { reads [Policy]; duration fixed 3min; result nondet { part } } }
  actor Rev { capacity 1
    capability review(e: Entry) -> V { duration fixed 1min; result nondet { pass, fail }; writes reviewed }
    capability check(a: Part, b: Part) -> V { duration fixed 1min; result nondet { pass, fail } } }
  actor Sup { capacity 1; capability second(e: Entry) -> V { reads [Policy]; duration fixed 1min; result nondet { pass, fail } } }
  actor Mgr { capacity 1; capability override(e: Entry) -> V { duration fixed 1min; result nondet { pass, fail } } }
  actor Clerk { capacity 1; capability fix() -> V { duration fixed 2min; result nondet { pass } } }
  actor Ledger { capacity 1; capability post(e: Entry) -> V { commits; duration fixed 1min; result nondet { pass } } }
  process P {
entry = Reader.enter()
'''
FOOTER = '''end done
  }
  cut C { include [Invoice, Policy, Reader, DevA, DevB, Rev, Sup, Mgr, Clerk, Ledger] }
  bounds { cases 1; live_tasks 2; clock 200min }
  verify reachable end.done
}
'''
ROUND = 'r = Rev.review(entry); '
RETRY = 'if r == pass { leave } else { f = Clerk.fix(); again }'
POST = 'p = Ledger.post(entry)'
SECOND = 'q = Sup.second(entry); '
DENIED = 'if r == fail { end denied } else {}'
HELD = 'if q == fail { end held } else {}'


def repeat(body, n=2, limit='end escalated', name='rework'):
    return f'repeat {name} at_most {n} {{ {body} }} on_limit {{ {limit} }}'


def model(*steps, postv=False):
    head = HEADER
    if postv:
        head = head.replace('Ledger { capacity 1; capability post(', '''Ledger { capacity 1; capability postv(v: V) -> V { commits; duration fixed 1min; result nondet { pass } }
    capability post(''')
    return head + ''.join(step + '\n' for step in steps) + FOOTER


def cases():
    review = repeat(ROUND + RETRY)
    second = repeat(ROUND + SECOND + RETRY)
    return {
        'G': model(review, DENIED, POST),
        'G2': model(repeat(ROUND + 'if r == pass { leave } else { leave }', n=3), DENIED, POST),
        'G3': model(second, HELD, POST),
        'G4': model(review, 'p = Ledger.postv(r)', postv=True),
        'G5': model(POST, second, HELD),
        'G6': model(repeat(ROUND + SECOND + RETRY, limit='if q == fail { end escalated } else {}; leave'), POST),
        'G7': model(repeat(ROUND + 'leave', n=1, name='x'), DENIED, POST),
        'G8': model(second, HELD),
        'G9': model(second, 'if q == pass { p = Ledger.post(entry) } else {}', postv=True),
    }


def run(binary, source, cmd):
    with TemporaryDirectory(prefix='repeat-review-', dir=cli.ROOT / '.tmp') as tmp:
        path = Path(tmp) / 'model.relca'
        path.write_text(source)
        out = subprocess.run([binary, cmd, str(path)], cwd=cli.ROOT, capture_output=True)
    doc = json.loads(out.stdout)
    stdout, stderr = diagnostic_facts(out.stdout, out.stderr, doc)
    channels = (stdout, stderr, f'{out.returncode}\n'.encode())
    lengths = (' '.join(str(len(c)) for c in channels) + '\n').encode()
    return out.returncode, hashlib.sha256(lengths + b''.join(channels)).hexdigest(), doc


def diagnostic_facts(stdout, stderr, doc):
    """Byte-preserving counterpart of the Rust diagnostic facts projection."""
    def rows(obj):
        if isinstance(obj, dict):
            d = obj.get('detail')
            if isinstance(d, dict) and d.get('reason') == 'result_not_definitely_available':
                yield obj, d
            for value in obj.values():
                yield from rows(value)
        elif isinstance(obj, list):
            for value in obj:
                yield from rows(value)

    def at(loc):
        return f"{loc['line']}:{loc['column']}"

    for row, d in rows(doc):
        assert row['code'] == 'U-SEM-0'
        marker = b'"detail":' + json.dumps(d, ensure_ascii=False, separators=(',', ':')).encode()
        assert marker in stdout
        stdout = stdout.replace(marker, b'"detail":null', 1)
        block = f"{row['code']}: {row['message']}\n"
        header = block.encode()
        if row['location'] is not None:
            block += f"  at {at(row['location'])}\n"
        block += f"  kind: {d['kind']}\n  cause: {d['cause']}\n"
        for b in d['blockers']:
            line = f"  blocked by: {b['role']}"
            if b['name'] is not None:
                line += ' ' + b['name']
            if 'rounds' in b:
                line += ' (rounds ' + ', '.join(map(str, b['rounds'])) + ')'
            if b['location'] is not None:
                line += ' at ' + at(b['location'])
            block += line + '\n'
        block += f"  see: {d['see']}\n"
        assert block.encode() in stderr
        stderr = stderr.replace(block.encode(), header, 1)
    return stdout, stderr


def hand_pairs(binary, shapes):
    def inspect(source):
        code, _, doc = run(binary, source, 'check')
        assert code == 0 and doc['verdict'] == 'satisfied', doc['diagnostics']
        return doc

    def terminals(doc):
        return {t['end']: (t['elapsed'], t['work']) for t in doc['terminals']}

    start = 'r1 = Rev.review(entry); q1 = Sup.second(entry)'
    body = 'if r1 == fail { f1 = Clerk.fix(); r2 = Rev.review(entry); q2 = Sup.second(entry); if r2 == fail { f2 = Clerk.fix(); end escalated } else { if q2 == fail { end held } else {} } } else { if q1 == fail { end held } else {} }'
    for name, hand in [
        ('G3', model(start, body, POST)),
        ('G4', model('r1 = Rev.review(entry)', 'if r1 == pass { p1 = Ledger.postv(r1) } else { f1 = Clerk.fix(); r2 = Rev.review(entry); if r2 == pass { p2 = Ledger.postv(r2) } else { f2 = Clerk.fix(); end escalated } }', postv=True)),
    ]:
        loop, written = inspect(shapes[name]), inspect(hand)
        assert terminals(loop) == terminals(written), name
        assert written['agency']['refusal'], name
        assert loop['agency']['refusal'] == [] and 'state_guards' in loop['agency']['unchecked'], name

    prefix = ('r0 = Rev.review(entry)', 'c0 = Sup.second(entry)',
              'if r0 == fail { if c0 == fail { end denied } else {} } else {}', 'p0 = Ledger.post(entry)')
    hand_tail = 'r1 = Rev.review(entry); if r1 == fail { f1 = Clerk.fix(); r2 = Rev.review(entry); if r2 == fail { f2 = Clerk.fix(); end escalated } else {} } else {}'
    loop = inspect(model(*prefix, repeat(ROUND + RETRY)))
    hand = inspect(model(*prefix, hand_tail))
    assert terminals(loop) == terminals(hand)
    for doc in (loop, hand):
        row, = [r for r in doc['agency']['viewpoints']['effects']['rows'] if r['call'] == 'p0']
        assert [(j['call'], j['stops'], j['bypasses']) for j in row['j']] == [('r0', 'conditional', 1), ('c0', 'conditional', 1)]
        assert sum(d['code'] == 'W-AGY-1' for d in doc['diagnostics']) == 2
    assert loop['agency']['refusal'] == [] and hand['agency']['refusal']

    duties = '''model Duties version "0.2" {
      type Entry = enum { entry }
      type V = enum { pass, fail }
      people [rd, rv, ck, lg]
      actor Invoice { source document }
      actor Reader { executors [rd]; capability enter() -> Entry { reads [Invoice]; duration fixed 1min; result nondet { entry } } }
      actor Rev { executors [rv]; capability review(e: Entry) -> V { duration fixed 1min; result nondet { pass, fail } } }
      actor Clerk { executors [ck]; capability fix() -> V { duration fixed 2min; result nondet { pass } } }
      actor Ledger { executors [lg]; capability post(e: Entry) -> V { commits; duration fixed 1min; result nondet { pass } } }
      responsible rv for Rev.review
      process P { entry = Reader.enter(); $BODY; p = Ledger.post(entry); end done }
      cut C { include [Invoice, Reader, Rev, Clerk, Ledger] }
      bounds { cases 1; live_tasks 1; clock 60min }
      verify reachable end.done
    }'''
    loop = inspect(duties.replace('$BODY', repeat(ROUND + 'if r == fail { f = Clerk.fix(); again } else {}; leave')))
    hand = inspect(duties.replace('$BODY', hand_tail))
    assert terminals(loop) == terminals(hand)
    assert 'responsibility_refusal' in loop['agency']['unchecked']
    assert 'responsibility_refusal' not in hand['agency']['unchecked'] and hand['agency']['refusal']
    assert loop['responsibilities'][0]['refusal']['reason'] == 'loop_control'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary')
    args = parser.parse_args()
    locked = json.loads(Path(__file__).with_suffix('.json').read_text())
    shapes = cases()
    assert set(shapes) == set(locked)
    for name, source in shapes.items():
        for cmd, expected in zip(('check', 'explain', 'mutate'), locked[name], strict=True):
            code, actual, doc = run(args.binary, source, cmd)
            assert (code, actual) == tuple(expected), (name, cmd, code, actual, doc['diagnostics'])
            if cmd != 'mutate':
                assert code == 0 and doc['verdict'] == 'satisfied', (name, cmd)
    hand_pairs(args.binary, shapes)


if __name__ == '__main__':
    main()
