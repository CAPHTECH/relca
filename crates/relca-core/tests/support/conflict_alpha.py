"""Independent, source-bound name/position projection for ADR0041 A12.

Only an explicitly supplied lexical edit is authorized. No semantic subtree is
discarded; diagnostic spans and full Agency payload remain in the comparison.
"""
import hashlib
import copy
import json
import re
import sys


def actor_starts(source):
    """Independent lexical boundaries at model-body depth, excluding strings/comments."""
    tokens = [m for m in re.finditer(
        rb'//[^\n]*|/\*[\s\S]*?\*/|"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|[{}]', source)
        if not m[0].startswith((b'//', b'/*', b'"'))]
    depth = 0
    starts = []
    for index, token in enumerate(tokens):
        if token[0] == b'actor' and depth == 1:
            assert index + 2 < len(tokens) and tokens[index + 2][0] == b'{'
            starts.append((token.start(), tokens[index + 1][0]))
        if token[0] == b'{':
            depth += 1
        elif token[0] == b'}':
            depth -= 1
    return starts


def compare(row):
    old=row['old_source'].encode();new=row['new_source'].encode()
    a=row['from'].encode();b=row['to'].encode()
    assert old.replace(a,b)==new and old!=new
    mapping={};old_at=new_at=0
    for match in re.finditer(re.escape(a),old):
        for p in range(old_at,match.start()+1):mapping[new_at+p-old_at]=p
        new_at+=match.start()-old_at
        mapping[new_at]=match.start();mapping[new_at+len(b)]=match.end()
        if len(a)==len(b):
            for p in range(len(a)+1):mapping[new_at+p]=match.start()+p
        old_at=match.end();new_at+=len(b)
    for p in range(old_at,len(old)+1):mapping[new_at+p-old_at]=p
    # ADR 0054 "意味": W-RESP-0 spans the Actor declaration. A grain edit
    # may cover its start inside a replaced substring; permit only that exact
    # independently located boundary, only as a W-RESP-0 span start.
    old_actors=actor_starts(old);new_actors=actor_starts(new)
    assert len(old_actors)==len(new_actors)
    actor_mapping={}
    for (old_start,old_name),(new_start,new_name) in zip(old_actors,new_actors):
        assert old_name.replace(a,b)==new_name
        actor_mapping[new_start]=old_start
    left=row['old'];right=row['new']
    assert left['model']['sha256']==hashlib.sha256(old).hexdigest()
    assert right['model']['sha256']==hashlib.sha256(new).hexdigest()
    right['model']['sha256']=left['model']['sha256']
    if row.get('grain'):
        before=left['actors'][0]['grain'];after=right['actors'][0]['grain']
        assert before=={'value':None,'status':'default'} and after=={'value':'team','status':'declared'}
        right['actors'][0]['grain']=before
    def point(offset, responsibility_start=False):
        if responsibility_start:
            assert offset in actor_mapping,('not an Actor declaration start',offset)
            if offset not in mapping:
                return actor_mapping[offset]
        assert offset in mapping,('unregistered interior source offset',offset)
        return mapping[offset]
    def linecol(offset):
        prefix=old[:offset];return prefix.count(b'\n')+1,len(prefix.rsplit(b'\n',1)[-1])+1
    def visit(v, responsibility_span=False):
        if isinstance(v,list):return [visit(c) for c in v]
        if isinstance(v,dict):
            out={k:visit(c, k=='span' and v.get('code')=='W-RESP-0') for k,c in v.items()}
            if set(v)=={'start','end'} and isinstance(v['start'],int):
                out={'start':point(v['start'],responsibility_span),'end':point(v['end'])}
            if isinstance(v.get('span'),dict) and all(k in v for k in ('line','column','end_line','end_column')):
                start,end=out['span']['start'],out['span']['end']
                out['line'],out['column']=linecol(start);out['end_line'],out['end_column']=linecol(end)
            return out
        if isinstance(v,str) and row.get('rename'):
            return re.sub(r'\b'+re.escape(row['to'])+r'\b',row['from'],v)
        return v
    normalized=visit(right)
    assert left==normalized, 'name/position projection changed a semantic field'


def check_with_negative_controls(row):
    compare(copy.deepcopy(row))
    for mode in ('one_character_shift', 'non_actor_position'):
        broken=copy.deepcopy(row)
        for key,source in (('old','old_source'),('new','new_source')):
            warning=next(d for d in broken[key]['diagnostics'] if d['code']=='W-RESP-0')
            warning['span']['start']=(warning['span']['start']+1 if mode=='one_character_shift'
                                      else broken[source].encode().index(b'capability'))
        try:
            compare(broken)
        except AssertionError:
            pass
        else:
            raise AssertionError(('invalid source position accepted',mode))


if __name__=='__main__':
    check_with_negative_controls(json.load(sys.stdin))
