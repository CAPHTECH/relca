"""Finite D06/quantile-review source inventory and hand read/write pairs.

Payloads come from the source's If/finish pair, not CLI output.
"""
from pathlib import Path
import re
from read_write_checks import require

ROOT = Path(__file__).resolve().parents[3]
HERE = Path(__file__).resolve().parent
FIELD = 'concurrency.read_write'
AFFECTS = '共有の観測stateの静的な読取り/書込み候補。到達や同時実行を確定しない。到達不能な枝や資源による直列化も含みうる。全順序を展開し、古い値を読む経路を残す。複数案件の共有Actor stateだけを対象とし、単一案件・案件局所・per_case・潜在は一覧の対象外。候補の不在は競合なしの証明ではない。'


def sources():
    text = (ROOT/'crates/relca-core/tests/support/quantile_review_inputs.rs').read_text()
    bases = {m[1]:m[2] for m in re.finditer(r'pub const (\w+): &str = r#"(.*?)"#;', text, re.S)}
    require(set(bases) == {'CLOSED', 'DESK', 'SELECTED'})
    out = []
    for desk in [False, True]:
        for positive in [False, True]:
            for quantiles in [False, True]:
                for reverse in [False, True]:
                    s = bases['DESK' if desk else 'CLOSED']
                    if positive:
                        before,after = ('reached(Desk.flag == true)','reached(Desk.seen == true)') if desk else ('Ledger.n == 2','Ledger.n == 1')
                        require(s.count(before) == 1)
                        s = s.replace(before,after)
                    s = s.replace('replications 1','replications 4')
                    if not quantiles:
                        s = '\n'.join(line for line in s.splitlines() if not line.strip().startswith('metric quantile.'))
                    body = s[s.index('  process P'):s.index('  cut C')]
                    second = body.replace('process P','process Q')
                    prop = 'Flag' if desk else 'Two'
                    addition = f'\n{second}\nconfiguration Left {{ cut C; process P }}\nconfiguration Right {{ cut C; process Q }}\nmetric cohort.Rate = expected(count(case.{prop})) / arrivals\ncompare [Left,Right] cut C hard {{ cohort.Rate <= 0.75 }} objectives {{ minimize cohort.Rate }}\n'
                    end = s.rfind('}')
                    s = s[:end] + addition + s[end:]
                    if reverse:
                        s = s.replace('compare [Left,Right]','compare [Right,Left]')
                    out.append((f'd06-{desk}-{positive}-{quantiles}-{reverse}', s, ['Left','Right']))
    out += [(name, bases[name], [None]) for name in ['CLOSED','DESK','SELECTED']]
    out.append(('SELECTED-fixed',bases['SELECTED'].replace('duration {2min:0.5, 4min:0.5}','duration fixed 4min'),[None]))
    atomic = (ROOT/'fixtures/cf-version-atomic/model.relca').read_text()
    addition = 'property case.Done = reached(end.done)\nmetric cohort.Rate = expected(count(case.Done)) / arrivals\nconfiguration Left { cut K; process Flow }\nconfiguration Right { cut K; process Flow }\ncompare [Left,Right] hard { cohort.Rate <= 1 } objectives { maximize cohort.Rate }'
    for removed in [False,True]:
        s = atomic.replace('version_check [keys] else conflict','') if removed else atomic
        end = s.rfind('}')
        s = s[:end]+'\n'+addition+'\n'+s[end:]
        out.append((f'atomic-quality-{removed}',s,['Left','Right']))
    for name in ['rec-approval','agy2-jue','cf-version-atomic']:
        s = (ROOT/f'fixtures/{name}/model.relca').read_text()
        require('metrics {' not in s)
        end = s.rfind('}')
        s = s[:end]+'\nmetrics { window 20min; currency "JPY"; breakdown waits }\n'+s[end:]
        out.append((f'waiting-{name}',s,[None]))
        out.append((f'waiting-{name}-masked',s.replace('breakdown waits','               '),[None]))
    require(len(out) == 28)
    return out


def inventory():
    out = []
    for name,source,configs in sources():
        for config in configs:
            rows = []
            if 'actor Desk' in source:
                process = 'Q' if config == 'Right' else 'P'
                start = source.encode().index(b'if Desk.seen',source.encode().index(f'process {process}'.encode()))
                # Hand relation: the If reads seen, only first writes seen;
                # second writes flag, and reached(property) is not a reader.
                rows = [dict(field=FIELD,value=f'Desk.seen: process.{process}@if:{start} -> Desk.first',affects=AFFECTS)]
            elif name.startswith('atomic-quality-') or name.startswith('waiting-cf-version-atomic'):
                # Hand atomic pair: post's membership test reads keys and the
                # same Capability inserts into keys. The quality property is not a reader.
                rows = [dict(field=FIELD,value='Ledger.keys: Ledger.post -> Ledger.post',affects=AFFECTS)]
            commands = ['check','simulate'] if 'simulation replications' in source else ['check']
            out.append(dict(name=name,source=source,configuration=config,rows=rows,commands=commands))
    require(len(out) == 46)
    require(sum(bool(row['rows']) for row in out) == 23)
    require(sum(not row['rows'] for row in out) == 23)
    return out


if __name__ == '__main__':
    import sys
    if sys.argv[1:]==['--json']:
        import json
        print(json.dumps(inventory(),ensure_ascii=False))
    else:
        require(not sys.argv[1:],'unknown hand-input export option')
        inventory()
        print('28 source variants;46 selected IR inputs;23 hand pairs and23 empty inputs')
