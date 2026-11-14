"""Scope candidate control with a lexical call on BOTH sides.

Desk.pay and Screen.approve each take one minute, so Old completes at two.
Controls applies only to New: expiring Mgr cannot be a candidate in Old,
even though Old calls Screen.approve. New has one authority and one target,
so exactly one expiry candidate exists and violates its authority contract.
This is a declaration/hand-time oracle, not a captured output expectation.
"""
import argparse
from pathlib import Path

import cli as c
import inputs as h


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    args = parser.parse_args()
    c.BINARY = str(Path(args.binary).resolve())
    source = h.scoped(h.both_call())
    out, value = c.invoke(source, 'mutate', ('--configuration', 'Old', '--kinds', 'authority_expired'))
    c.verdict(out, value, 'vacuous')
    assert value['run']['reason'] == 'no_selected_mutant'
    assert value['original']['verdict'] == 'satisfied'
    c.terminal_time(value['original'], 2)
    c.scope(value['original'], masks=True)
    c.diagnostic(value['original'], 'W-CONF-2')
    assert c.mutation(value, 'authority_expired')['mutants'] == []
    out, value = c.invoke(source, 'mutate', ('--configuration', 'New', '--kinds', 'authority_expired'))
    c.verdict(out, value, 'satisfied')
    c.terminal_time(value['original'], 2)
    mutants = c.mutation(value, 'authority_expired')['mutants']
    assert len(mutants) == 1 and mutants[0]['result'] == 'detected'
    print('scope candidate lexical-call control: GREEN')


if __name__ == '__main__':
    main()
