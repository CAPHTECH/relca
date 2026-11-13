"""Read fixed, independently authored OLD DSL witnesses; never parse repeat.

The path and fractional derivations are in calculations.md. This module is
only test input plumbing, not a source-to-source adapter or an interpreter.
"""
from pathlib import Path
import sys

NAMES = ('L1', 'L2', 'L3', 'L4', 'P1', 'L1-FFF', 'P1-r1', 'P1-r2',
         'P1-r3', 'P1-never', 'L1-32', 'L2-32', 'L3-32', 'L4-32',
         'P1-32', 'L2-clock', 'lineage')


def source(name):
    assert name in NAMES, name
    return (Path(__file__).parent / 'witnesses' / (name + '.relca')).read_text()


if __name__ == '__main__':
    assert sys.argv[1] == '--print'
    print(source(sys.argv[2]), end='')
