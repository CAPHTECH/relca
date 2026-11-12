"""P07 exact wire alpha/grain control, with independent source-byte mapping.

Only scalar leaves justified by the source edits are rewritten. Object key order,
whitespace, unknown fields, all numbers not representing source locations, and
all property notes remain byte-exact. This is not recursive field deletion.
"""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT=Path(__file__).resolve().parents[4]

def spans(raw):
    """JSON byte locations for this current-output alpha test, without a gate."""
    locations = {}

    def whitespace(at):
        while at < len(raw) and raw[at] in b' \t\r\n':
            at += 1
        return at

    def string_end(at):
        assert raw[at] == 34
        at += 1
        while at < len(raw):
            if raw[at] == 92:
                at += 2
            elif raw[at] == 34:
                return at + 1
            else:
                at += 1
        raise AssertionError('unterminated JSON string')

    def value(at, path):
        at = whitespace(at)
        start = at
        if raw[at] == 123:
            at = whitespace(at + 1)
            keys = set()
            while raw[at] != 125:
                end = string_end(at)
                key = json.loads(raw[at:end])
                assert key not in keys, 'duplicate JSON key'
                keys.add(key)
                at = whitespace(end)
                assert raw[at] == 58
                at = whitespace(value(at + 1, path + '/' + key.replace('~', '~0').replace('/', '~1')))
                if raw[at] != 44:
                    break
                at = whitespace(at + 1)
            assert raw[at] == 125
            at += 1
        elif raw[at] == 91:
            at = whitespace(at + 1)
            index = 0
            while raw[at] != 93:
                at = whitespace(value(at, path + '/' + str(index)))
                index += 1
                if raw[at] != 44:
                    break
                at = whitespace(at + 1)
            assert raw[at] == 93
            at += 1
        elif raw[at] == 34:
            at = string_end(at)
        else:
            while at < len(raw) and raw[at] not in b',]} \t\r\n':
                at += 1
            json.loads(raw[start:at])
        locations[path] = (start, at)
        return at

    assert whitespace(value(0, '')) == len(raw), 'trailing JSON input'
    return locations, {}


def compact(v):
    return json.dumps(v,separators=(',',':'),ensure_ascii=False).encode()


def position(source,at):
    before=source[:at]
    return before.count(b'\n')+1,len(before.rsplit(b'\n',1)[-1].decode())+1


def source_edit(old,name,grain):
    edits=[(m.start(),m.end(),name.encode()) for m in re.finditer(b'Ledger',old)]
    if grain:
        at=old.index(b'actor Ledger {')+len(b'actor Ledger {')
        edits.append((at,at,(' grain '+grain+';').encode()))
    edits.sort()
    new=old
    for a,b,value in reversed(edits):
        new=new[:a]+value+new[b:]
    assert new!=old
    def offset(at):
        delta=0
        for a,b,value in edits:
            if at<a:
                break
            if a<at<b:
                raise AssertionError(('source endpoint inside identifier',at,a,b))
            if at>=b:
                delta+=len(value)-(b-a)
        return at+delta
    return new,offset


def expected_wire(raw,old_source,new_source,map_offset,name,grain):
    old=json.loads(raw)
    leaf_spans,_=spans(raw)
    assert old['model']['sha256']==hashlib.sha256(old_source).hexdigest()
    edits={}
    def put(path,value):
        a,b=leaf_spans[path]
        edits[path]=(a,b,compact(value))
    def walk(value,path=''):
        if isinstance(value,dict):
            if isinstance(value.get('span'),dict):
                start,end=value['span']['start'],value['span']['end']
                assert 0<=start<=end<=len(old_source)
                a,b=map_offset(start),map_offset(end)
                put(path+'/span/start',a)
                put(path+'/span/end',b)
                if 'line' in value:
                    assert (value['line'],value['column'])==position(old_source,start)
                    assert (value['end_line'],value['end_column'])==position(old_source,end)
                    line,col=position(new_source,a)
                    end_line,end_col=position(new_source,b)
                    for key,item in [('line',line),('column',col),('end_line',end_line),('end_column',end_col)]:
                        put(path+'/'+key,item)
            for key,item in value.items():
                walk(item,path+'/'+key.replace('~','~0').replace('/','~1'))
        elif isinstance(value,list):
            for i,item in enumerate(value):
                walk(item,path+'/'+str(i))
        elif isinstance(value,str) and 'Ledger' in value:
            put(path,value.replace('Ledger',name))
    walk(old)
    put('/model/sha256',hashlib.sha256(new_source).hexdigest())
    if grain:
        assert old['actors'][0]['name']=='Ledger'
        assert old['actors'][0]['grain']=={'value':None,'status':'default'}
        put('/actors/0/grain/value',grain)
        put('/actors/0/grain/status','declared')
    expected=raw
    for a,b,value in sorted(edits.values(),reverse=True):
        expected=expected[:a]+value+expected[b:]
    return expected


def run(binary,path):
    return subprocess.run([str(binary),'check',str(path)],cwd=ROOT,capture_output=True)


def main():
    binary=Path(sys.argv[1]).resolve()
    original=ROOT/'fixtures/aset-label/model.relca'
    old_source=original.read_bytes()
    before=run(binary,original)
    assert before.returncode==0,(before.returncode,before.stderr.decode())
    directory=ROOT/'target/aset-alpha-control'
    directory.mkdir(exist_ok=True)
    for grain in [None,'document','ledger','person']:
        new_source,offset=source_edit(old_source,'Book',grain)
        path=directory/('Book-'+str(grain)+'.relca')
        path.write_bytes(new_source)
        after=run(binary,path)
        assert after.returncode==0,(grain,after.returncode,after.stderr.decode())
        expected=expected_wire(before.stdout,old_source,new_source,offset,'Book',grain)
        assert expected==after.stdout,('unjustified wire difference',grain)
        assert before.stderr.replace(b'Ledger',b'Book')==after.stderr
    print('P07: four full stdout/stderr/exit alpha/grain controls, source-bound SHA/spans/coordinates')


def self_test():
    source=b'actor Ledger {'
    raw=(b'{"model":{"sha256":"'+hashlib.sha256(source).hexdigest().encode()+
         b'"},"actors":[{"name":"Ledger","grain":{"value":null,"status":"default"}}],'
         b'"diagnostics":[{"span":{"start":6,"end":12},"line":1,"column":7,"end_line":1,"end_column":13}],"states":7}\n')
    new,offset=source_edit(source,'Book','document')
    assert new==b'actor Book { grain document;'
    expected=expected_wire(raw,source,new,offset,'Book','document')
    value=json.loads(expected)
    assert value['diagnostics']==[{'span':{'start':6,'end':10},'line':1,'column':7,'end_line':1,'end_column':11}]
    assert value['actors']==[{'name':'Book','grain':{'value':'document','status':'declared'}}]
    assert value['states']==7 and expected.endswith(b',"states":7}\n')
    assert value['model']['sha256']==hashlib.sha256(new).hexdigest()
    assert b'"model":{"sha256":' in expected
    print('P07 source-map calibration: name length, grain insertion, SHA, both span endpoints and columns')


if __name__=='__main__':
    self_test() if sys.argv[1:]==['--self-test'] else main()
