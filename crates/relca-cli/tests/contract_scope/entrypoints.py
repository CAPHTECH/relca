"""Scoped static refusals must not depend on declaring simulation settings."""
import argparse
from pathlib import Path

import cli as c
import inputs as h


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--binary', required=True)
    args = parser.parse_args()
    c.BINARY = str(Path(args.binary).resolve())
    c.refuse(h.scoped(h.CONTEST), 'U-SEM-0', 'contract_scope_contest',
             'simulate', ('--configuration', 'Old'))
    source = h.edit(h.scoped(), '  verify reachable end.done\n', '')
    out, value = c.invoke(source, 'simulate', ('--configuration', 'Old'))
    c.verdict(out, value, 'unsupported')
    c.diagnostic(value, 'U-SEM-0', 'at least one verification request is required')
    c.scope(value)
    print('scoped entrypoint static gate: GREEN')


if __name__ == '__main__':
    main()
