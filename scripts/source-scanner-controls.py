"""Independent source-scanner hand controls recovered without any old binary/output gate."""

import argparse, hashlib, json, re, os, subprocess

from typing import Any

from pathlib import Path



def integer_controls():
    def candidates(source):
        """Independent finite source census of C1..C5, with declaration precedence.

        Token offsets are bytes. Balanced blocks, rather than line matching, handle
        multiline/forward declarations. Strings are indivisible and never keywords.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]', source)]
        pairs, stack = {}, []
        for i, (t, _, _) in enumerate(ts):
            if t in (b"{", b"(", b"["):
                stack.append(i)
            elif t in (b"}", b")", b"]") and stack:
                a = stack.pop()
                assert (ts[a][0], t) in ((b"{", b"}"), (b"(", b")"), (b"[", b"]"))
                pairs[a] = i
        words = [t[0] for t in ts]
        enums = {words[i+1] for i in range(len(ts)-3)
                 if words[i] == b"type" and words[i+2:i+4] == [b"=", b"enum"]}
        observed, sources, state_types, params, reads, args = set(), set(), [], [], [], []
        for i, t in enumerate(words):
            if words[i:i+3] == [b"observed", b"case", b"{"] and i+2 in pairs:
                end = pairs[i+2]
                for j in range(i+3, end):
                    if words[j] == b":" and j+1 < end:
                        k = j-1
                        observed.add(words[k])
                        while k >= i+5 and words[k-1] == b",":
                            k -= 2
                            observed.add(words[k])
                        state_types.append(j+3 if words[j+1:j+3] == [b"optional", b"<"] else j+1)
            if words[i:i+3] == [b"latent", b"case", b"{"] and i+2 in pairs:
                state_types.extend(j+3 if words[j+1:j+3] == [b"optional", b"<"] else j+1
                                   for j in range(i+3, pairs[i+2]) if words[j] == b":")
            if t == b"actor" and i+2 in pairs and words[i+2] == b"{":
                # source is an actor-level field, not a capability/body string.
                j = i+3
                while j < pairs[i+2]:
                    if words[j] == b"source":
                        sources.add(words[i+1])
                    j = pairs[j]+1 if j in pairs else j+1
            if t == b"capability" and i+2 in pairs and words[i+2] == b"(":
                params.extend(j+1 for j in range(i+3, pairs[i+2]) if words[j] == b":")
            if t == b"reads" and i+1 in pairs and words[i+1] == b"[":
                reads.extend(j for j in range(i+2, pairs[i+1]) if words[j] != b",")
            if t == b"(" and i >= 3 and words[i-2] == b"." and i in pairs:
                args.extend(j for j in range(i+1, pairs[i]-2)
                            if words[j:j+3] == [b"case", b".", b"id"])
        result = []
        for j in state_types:
            if words[j] not in enums and words[j] in (b"int", b"CaseId"):
                result.append(("C1" if words[j] == b"int" else "C5", ts[j][1], ts[j][2]))
        result += [("C2", ts[j][1], ts[j][2]) for j in params if words[j] == b"CaseId" and words[j] not in enums]
        result += [("C3", ts[j][1], ts[j+2][2]) for j in args]
        result += [("C4", ts[j][1], ts[j][2]) for j in reads if words[j] in observed and words[j] not in sources]
        return sorted(set(result))

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def self_test():
        tests = [(b'observed case { n: int }', [('C1', b'int')]), (b'observed case { n: CaseId }', [('C5', b'CaseId')]), (b'actor A { capability c(k: CaseId) -> T {} }', [('C2', b'CaseId')]), (b'process P { a = A.c(case.id) }', [('C3', b'case.id')]), (b'actor A { capability c() -> T { reads [flag] } }; observed case { flag: bool = false }', [('C4', b'flag')]), (b'observed case { a,b: bool = true }; actor A { capability c() -> T { reads [a,b] } }', [('C4', b'a'), ('C4', b'b')]), (b'observed case { n: int }; type int = enum { ok }', []), (b'observed case { n: CaseId }; actor A { capability c(k: CaseId) -> T {} }; type CaseId = enum { ok }', []), (b'observed case { flag: bool = true }; actor flag { source record }; actor A { capability c() -> T { reads [flag] } }', []), (b'contract C { at_most_once A.c.commit per case.id }; actor A { capability c() -> T { reads [Unknown] } }', []), (b'authority G { basis "n: int; A.c(case.id)" }', [])]
        for source, expected in tests:
            got = [(c, source[a:b]) for c, a, b in candidates(source)]
            assert got == expected, (source, got, expected)
        nested = {'original': {}, 'mutations': [{'mutants': [{'report': {}}, {'report': None}]}]}
        assert [p for p, _ in reports(nested)] == ['/original', '/mutations/0/mutants/0/report']
        print('11 independent classification-census controls passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'integer':
        print('fault applied source-scanner integer', flush=True)
        candidates = lambda source: []

    self_test()


def actor_state_controls():
    def candidates(source):
        """Byte-token independent C1/C2 census. Strings stay indivisible.

        Only conditions inside if/requires/scope/invariant/responsible are C1;
        event paths (three components) and end.name probability queries are not.
        Only qualified assignment LHS inside capability finish blocks are C2.
        Balanced nesting and all LHS positions, not one regex per line.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|!=|<=|>=|==|->|[^\s]', source)]
        w = [t[0] for t in ts]
        pairs, stack = {}, []
        for i, t in enumerate(w):
            if t in (b'{', b'(', b'['):
                stack.append(i)
            elif t in (b'}', b')', b']') and stack:
                a = stack.pop()
                assert (w[a], t) in ((b'{', b'}'), (b'(', b')'), (b'[', b']'))
                pairs[a] = i
        ident = lambda t: re.fullmatch(rb'[A-Za-z_][A-Za-z_0-9]*', t) is not None
        actors, caps, finishes, conditions = [], [], [], []
        for i, t in enumerate(w):
            if t == b'actor' and i+2 in pairs and w[i+2] == b'{':
                actors.append((i+2, pairs[i+2]))
            if t == b'capability' and i+2 in pairs and w[i+2] == b'(':
                close = pairs[i+2]
                body = next((j for j in range(close+1, len(w)) if w[j] == b'{'), None)
                if body in pairs:
                    caps.append((body, pairs[body]))
        for i, t in enumerate(w):
            if t == b'finish' and i+1 in pairs and w[i+1] == b'{' and any(a<i<b for a,b in caps):
                finishes.append((i+1, pairs[i+1]))
            if t in (b'if', b'requires', b'scope', b'invariant', b'responsible', b'reached'):
                end = i+1
                while end < len(w) and w[end] not in (b'{', b'}', b';', b'else'):
                    # A following DSL field starts a new condition context.
                    if end > i+1 and w[end] in (b'verify', b'duration', b'result', b'grants', b'issued_by'):
                        break
                    end += 1
                conditions.append((i, end))
        found = []
        for i in range(len(w)-3):
            if not (ident(w[i]) and w[i+1] == b'.' and ident(w[i+2])):
                continue
            if i and w[i-1] == b'.':
                continue
            op = w[i+3]
            if op in (b'!=', b'<', b'<=', b'>', b'>=') and w[i] not in (b'end', b'case') and any(a<i<b for a,b in conditions):
                found.append(('qualified-comparison', ts[i+3][1], ts[i+3][2]))
            if op == b'=' and any(a<i<b for a,b in finishes):
                found.append(('qualified-finish', ts[i][1], ts[i+2][2]))
        return sorted(set(found))

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def self_test():
        tests = [(b'process P { if A.x != ok { } }', [('qualified-comparison', b'!=')]), (b'actor A { capability c() -> T { requires A.x <= 2 else no } }', [('qualified-comparison', b'<=')]), (b'contract K { invariant A.c.finish -> A.x > 1 }', [('qualified-comparison', b'>')]), (b'authority G { scope A.x >= 1 }', [('qualified-comparison', b'>=')]), (b'responsible p for (A.x < 1)', [('qualified-comparison', b'<')]), (b'actor L { capability c() -> T { finish { a = ok;\n L.n = ok } } }', [('qualified-finish', b'L.n')]), (b'actor A { capability c() -> T { finish { Other.x = ok } } }', [('qualified-finish', b'Other.x')]), (b'actor L { latent n:T=ok; capability c() -> T { finish { L.n = ok } } }', [('qualified-finish', b'L.n')]), (b'actor A { capability c() -> T { writes A.x; start { A.x = ok } } }', []), (b'process P { if x != ok { } }; verify probability reached end.done <= 1', []), ('authority G { basis "日本語 A.x != ok finish { L.n = ok }" }'.encode(), []), (b'contract K { invariant A.c.finish -> B.d.finish }', []), (b'verify probability reached A.x != ok <= 1', [('qualified-comparison', b'!=')]), (b'process P { if A.x\n < 1 {} }; actor A { latent x:T=ok }', [('qualified-comparison', b'<')])]
        for source, expected in tests:
            assert [(c, source[a:b]) for c, a, b in candidates(source)] == expected, source
        nested = {'original': {}, 'mutations': [{'report': {}, 'mutants': [{'report': {}}]}]}
        assert [p for p, _ in reports(nested)] == ['/original', '/mutations/0/report', '/mutations/0/mutants/0/report']
        print('14 source controls and nested Report traversal passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'actor_state':
        print('fault applied source-scanner actor_state', flush=True)
        candidates = lambda source: []

    self_test()


def state_invariant_controls():
    def candidates(source):
        """Independent byte-token contract walker, not the new Relca parser.

        Strings are indivisible. Semicolons/newlines do not affect event lookahead.
        Multiple clauses are bounded by contract fields. A malformed earlier clause
        does not turn a later candidate into a permission: old diagnostic intersection
        is computed separately. Prefix ! belongs to A3 and is not a C3 candidate.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|->|==|!=|<=|>=|&&|\|\||[^\s]', source)]
        w = [t[0] for t in ts]
        pairs, stack = {}, []
        for i,t in enumerate(w):
            if t in (b'{',b'(',b'['):
                stack.append(i)
            elif t in (b'}',b')',b']') and stack:
                a=stack.pop()
                if (w[a],t) in ((b'{',b'}'),(b'(',b')'),(b'[',b']')):
                    pairs[a]=i
        ident=lambda t: re.fullmatch(rb'[A-Za-z_][A-Za-z_0-9]*',t) is not None
        clauses={b'invariant',b'at_most_once',b'response',b'requires_event',b'separation',b'permitted',b'authority_basis',b'contest',b'excludes',b'waive'}
        found=[]
        for i in range(len(w)-3):
            if w[i]!=b'contract' or w[i+2]!=b'{' or i+2 not in pairs:
                continue
            stop=pairs[i+2]
            j=i+3
            while j<stop:
                if w[j]!=b'invariant':
                    j=pairs[j]+1 if j in pairs else j+1
                    continue
                a=j+1
                end=a
                while end<stop and w[end] not in clauses and w[end]!=b';':
                    end+=1
                z=w[a:end]
                if not z:
                    j=end
                    continue
                event=(len(z)>=5 and ident(z[0]) and z[1]==b'.' and ident(z[2]) and z[3]==b'.' and ident(z[4]))
                if z[0]==b'!':
                    pass
                elif not event:
                    cls='state-form'
                    if z[:3] in ([b'end',b'.',b'posted'],[b'case',b'.',b'received']) or (len(z)>=3 and z[0]==b'end' and z[1]==b'.'):
                        cls='terminal-or-receipt'
                    found.append((cls,ts[a][1],ts[end-1][2]))
                else:
                    arrows=[k for k in range(a,end) if w[k]==b'->']
                    if len(arrows)>1:
                        found.append(('event-nested-arrow',ts[arrows[1]][1],ts[arrows[1]][2]))
                    if arrows:
                        rhs=w[arrows[0]+1:end]
                        if b'!' not in rhs:
                            # A bare atom is an identifier bounded by grouping/logical
                            # operators, not an event component, comparison or value.
                            for k in range(arrows[0]+1,end):
                                if ident(w[k]) and (k==arrows[0]+1 or w[k-1] in (b'(',b'&&',b'||')) and (k+1==end or w[k+1] in (b')',b'&&',b'||')):
                                    found.append(('event-bare-atom',ts[k][1],ts[k][2]))
                j=max(end,j+1)
        return sorted(set(found))

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def self_test():
        good = [b'contract C { invariant A.cap.finish -> x == ok }', b'contract C { invariant A.cap.finish(ok) -> (x == ok && n != 2) }', b'contract C { invariant A.cap.start\n -> A.n <= 2; invariant B.cap.finish -> x == true }', 'actor A { boundary "日本語 invariant (x) -> y" }'.encode(), b'contract C { invariant A.cap.finish -> !ok }', b'contract C { invariant !ok }']
        for s in good:
            assert not candidates(s), (s, candidates(s))
        cases = [(b'contract C { invariant x -> y == true }', 'state-form'), (b'contract C { invariant (x == true) -> y == true }', 'state-form'), (b'contract C { invariant Ledger.x\n <= 2 }', 'state-form'), (b'contract C { invariant case.received -> x == true }', 'terminal-or-receipt'), (b'contract C { invariant end.done -> x == true }', 'terminal-or-receipt'), (b'contract C { invariant A.cap.finish -> x }', 'event-bare-atom'), (b'contract C { invariant A.cap.finish -> (x -> y == true) }', 'event-nested-arrow'), (b'contract C { invariant A.cap.finish -> x == true -> y == true }', 'event-nested-arrow')]
        for s, c in cases:
            assert any((k == c for k, _, _ in candidates(s))), (s, candidates(s))
        s = b'contract C { invariant x; invariant A.cap.finish -> y; invariant L.n != 2 }'
        assert len(candidates(s)) == 3
        v = {'original': {}, 'mutations': [{'report': {}, 'mutants': [{'report': {}}]}]}
        assert [p for p, _ in reports(v)] == ['/original', '/mutations/0/report', '/mutations/0/mutants/0/report']
        print('15 independent token controls and canonical nested-report traversal passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'state_invariant':
        print('fault applied source-scanner state_invariant', flush=True)
        candidates = lambda source: []

    self_test()


def policy_controls():
    def candidates(source):
        """Model-item byte tokens only; strings and nested item bodies are opaque."""
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]', source)]
        depth, model_depth, found = 0, None, []
        for i, (word, start, end) in enumerate(ts):
            if word == b'{':
                depth += 1
                if model_depth is None and any(t[0] == b'model' for t in ts[:i]):
                    model_depth = depth
            elif word == b'}':
                depth -= 1
            elif word == b'policy' and depth == model_depth:
                words = [t[0] for t in ts[i+1:i+7]]
                if words[:1] == [b'observable']:
                    found.append(('observable', start, ts[i+1][2]))
                elif words[:3] == [b'fifo', b'tie_by', b'case_id'] and words[3:4] in ([b'then'], [b'assign']):
                    found.append(('extension', start, ts[i+4][2]))
        return found

    def self_test():
        controls = [('policy observable fifo tie_by case_id', 1), ('policy fifo tie_by case_id then call_order', 1), ('policy fifo tie_by case_id assign first_idle', 1), ('policy\nfifo tie_by case_id\nthen hard;', 1), ('policy fifo tie_by case_id; policy observable fifo tie_by case_id', 1), ('actor A { control policy; capability assign() -> T {} }', 0), ('process P { assign = A.call_order(); end done }', 0), ('cut C { question "policy observable fifo tie_by case_id" }', 0), ('actor observable { grain assign }', 0), ('policy fifo tie_by case_id', 0), ('policy fifo tie_by case_id; actor assign {}', 0), ('actor A { boundary "日本語 policy fifo tie_by case_id then hard" }', 0)]
        for text, count in controls:
            src = ('model M version "0.2" { ' + text + ' }').encode()
            assert len(candidates(src)) == count, text
        src = 'model M version "0.2" { cut C { question "日本語" }; policy observable fifo }'.encode()
        [(kind, a, b)] = candidates(src)
        assert kind == 'observable' and src[a:b] == b'policy observable'
        print('13 independent byte scanner controls passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'policy':
        print('fault applied source-scanner policy', flush=True)
        candidates = lambda source: []

    self_test()


def resource_controls():
    def candidates(source):
        # Strings are indivisible: words in descriptions are not syntax candidates.
        return [{"word": m.group().decode(), "span": {"start": m.start(), "end": m.end()}}
                for m in re.finditer(rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*', source)
                if m.group() in (b"resource", b"uses")]

    def self_test():
        for source, words in [(b'resource R capacity 2', ['resource']), (b'capability c() -> T { uses R: 1, S: 2 }', ['uses']), (b'actor uses {} observed case { resource: bool }', ['uses', 'resource']), (b'question "resource uses"', []), (b'question "escaped \\" resource uses" resource R capacity 0', ['resource']), (b'actor resource2 {} actor uses_backup {}', [])]:
            got = candidates(source)
            assert [x['word'] for x in got] == words, (source, got, words)
            assert all((source[x['span']['start']:x['span']['end']].decode() == x['word'] for x in got))
        print('six independent token/byte-span controls passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'resource':
        print('fault applied source-scanner resource', flush=True)
        candidates = lambda source: []

    self_test()


def record_controls():
    def candidates(source):
        """Byte tokens, balanced delimiters, indivisible strings; no Relca parser.

        A colon or arrow introduces a type, except a record literal value's equals.
        source record and a named enum called record are not record declarations.
        Return enough context to distinguish forbidden inline positions independently.
        """
        tokens = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|->|==|!=|[^\s]', source)]
        w = [t[0] for t in tokens]
        stack, pairs, unbalanced = [], {}, False
        for i, t in enumerate(w):
            if t in (b'{', b'(', b'['):
                stack.append(i)
            elif t in (b'}', b')', b']'):
                if stack:
                    a = stack.pop()
                    if (w[a], t) not in ((b'{', b'}'), (b'(', b')'), (b'[', b']')):
                        # An intentionally malformed source (ADR 0037 adds
                        # sinv-static-delimiter.broken-delimiter). Scopes are then
                        # unknown, so any record/enum token refuses below.
                        unbalanced = True
                    pairs[a] = i
        if unbalanced:
            assert not any(
                (t == b'enum' and i and w[i-1] in (b':', b'->', b'<'))
                or (t == b'record' and i and w[i-1] == b'=')
                for i, t in enumerate(w)), 'unbalanced source with a record/inline enum token'
            return []
        records = {w[i+1] for i in range(len(w)-4)
                   if w[i] == b'type' and w[i+2:i+5] == [b'=', b'record', b'{']}
        fields, parameters, actors, cases = [], [], [], []
        for i, t in enumerate(w):
            if t == b'record' and i+1 in pairs and i and w[i-1] == b'=':
                fields.append((i+1, pairs[i+1]))
            if t == b'capability' and i+2 in pairs:
                parameters.append((i+2, pairs[i+2]))
            if t in (b'actor', b'case'):
                j = i+2 if t == b'actor' else i+1
                if j in pairs:
                    (actors if t == b'actor' else cases).append((j, pairs[j]))
        result = []
        for i, t in enumerate(w):
            kind = None
            if t == b'record' and i+1 < len(w) and w[i+1] == b'{' and i and w[i-1] == b'=':
                kind = 'record-declaration'
            if t == b'enum' and i and w[i-1] in (b':', b'->', b'<'):
                if any(a < i < b for a, b in fields):
                    kind = 'inline-field'
                elif w[i-1] == b'->':
                    kind = 'inline-result'
                elif any(a < i < b for a, b in parameters):
                    kind = 'inline-parameter-forbidden'
                elif any(a < i < b for a, b in cases):
                    kind = 'inline-case-forbidden'
                elif any(a < i < b for a, b in actors):
                    kind = 'inline-actor-forbidden'
                else:
                    kind = 'inline-unknown-forbidden'
            if t in records and i+1 < len(w) and w[i+1] == b'{' and (not i or w[i-1] != b'type'):
                kind = 'record-value'
            if kind:
                result.append((kind, tokens[i][1], tokens[i][2]))
        return result

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def self_test():
        controls = [(b'type R = record { f: enum { a,b } }', ['record-declaration', 'inline-field']), (b'capability c() -> enum {a,b} { result nondet { a } }', ['inline-result']), (b'capability c(x: enum {a,b}) -> T {}', ['inline-parameter-forbidden']), (b'observed case { x: enum {a,b}; y: optional<enum {a,b}> }', ['inline-case-forbidden'] * 2), (b'latent case { x: enum {a,b} }', ['inline-case-forbidden']), (b'actor A { state x: enum {a,b}; latent y: enum {a,b} }', ['inline-actor-forbidden'] * 2), (b'actor A { source record; capability c() -> record {} }; type record = enum {a}', []), ('model M { basis "日本語 : enum {a} = record {x}"; observed case { x:\n enum {a} } }'.encode(), ['inline-case-forbidden']), (b'observed case { x:R = R { f=a } }; type R = record { f:T }', ['record-value', 'record-declaration']), (b'if x == a {} else {}; type record = enum {a}; capability c(x:record)->record {}', []), (b'unknown { x: enum {a} }', ['inline-unknown-forbidden']), (b'type R = enum { ok }; contract C { invariant (ok }', [])]
        for source, want in controls:
            assert [k for k, _, _ in candidates(source)] == want, (source, candidates(source))
            for _, a, b in candidates(source):
                assert source[a:b] in (b'record', b'enum', b'R')
        for source in (b'x: enum {a} ( ]', b'type R = record { f: T } ( ]'):
            try:
                candidates(source)
            except AssertionError:
                continue
            raise AssertionError(('unbalanced record/enum source must refuse', source))
        nested = {'original': {}, 'mutations': [{'report': {}, 'mutants': [{'report': {}}]}]}
        assert [p for p, _ in reports(nested)] == ['/original', '/mutations/0/report', '/mutations/0/mutants/0/report']
        print('12 byte-token controls, 2 unbalanced refusals and canonical nested Report traversal passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'record':
        print('fault applied source-scanner record', flush=True)
        candidates = lambda source: []

    self_test()


def limits_controls():
    def candidates(source):
        """Independent byte tokens, balanced depth, model items and 2-part events.

        Quoted strings are indivisible; actor/process local names and three-part
        case.admitted.finish events are not builtin admission syntax.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|->|[^\s]', source)]
        words = [t[0] for t in ts]
        stack, found = [], []
        for i, word in enumerate(words):
            top = len(stack) == 1 and words[stack[0]] == b'{'
            tail = words[i:i+5]
            kind = None
            if top and tail[:2] == [b'concurrent', b'cases']:
                kind = 'concurrent'
            if top and tail[:2] == [b'queue', b'capacity'] and len(tail) == 5 and tail[3] == b'on_overflow' and tail[4] in (b'wait_external', b'end'):
                kind = 'queue-overflow'
            if tail[:3] == [b'case', b'.', b'admitted'] and (len(tail) < 4 or tail[3] != b'.'):
                kind = 'admitted-event'
            if kind:
                found.append((kind, ts[i][1], ts[min(i+4,len(ts)-1)][2]))
            if word in (b'{', b'[', b'('):
                stack.append(i)
            elif word in (b'}', b']', b')'):
                if not stack or (words[stack.pop()], word) not in ((b'{',b'}'),(b'[',b']'),(b'(',b')')):
                    return unbalanced(source)
        return found

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def unbalanced(source):
        """A deliberately malformed old input (e.g. sinv-static-delimiter's broken
        delimiter) has no reliable depth. It is accepted only when no limit word
        occurs anywhere in its bytes, so nothing can hide behind the bad nesting."""
        assert not re.search(rb'concurrent|queue|admitted', source), 'unbalanced old input with a limit word'
        return []

    def self_test():
        for source, kinds in [(b'model M { concurrent\n cases 2 on_overflow reject }', ['concurrent']), (b'model M { queue capacity 1 on_overflow wait_external fifo }', ['queue-overflow']), (b'model M { queue capacity 1 on_overflow end spill }', ['queue-overflow']), (b'model M { contract C { response case . admitted -> one_of(end.x) by 1min after case.admitted otherwise unresolved } }', ['admitted-event'] * 2), ('model M { actor A { boundary "日本語 concurrent cases 1 on_overflow end spill case.admitted" } }'.encode(), []), (b'model M { actor concurrent { capability cases() -> T {} } process P { x=concurrent.cases();end done } }', []), (b'model M { actor case { capability admitted() -> T {} } contract C { invariant case.admitted.finish -> x == true } }', []), (b'model M { queue capacity 0 on_overflow reject }', []), (b'model M { actor A { concurrent cases 1; queue capacity 1 on_overflow end x } }', []), (b'model M { type concurrent=enum { wait_external, fifo }; process P {end admitted} }', []), (b'model M { contract C { invariant (ok } }', [])]:
            assert [k for k, _, _ in candidates(source)] == kinds, source
        for source in [b'model M { invariant (ok } concurrent cases 1 on_overflow reject }', b'model M { ] queue capacity 1 }', b'model M { ) case.admitted }']:
            try:
                candidates(source)
            except AssertionError:
                continue
            raise AssertionError(('unbalanced limit word accepted', source))
        assert [p for p, _ in reports({'original': {}, 'mutations': [{'report': {}}]})] == ['/original', '/mutations/0/report']
        print('11 source controls, 3 unbalanced limit-word refusals and independent nested Report traversal passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'limits':
        print('fault applied source-scanner limits', flush=True)
        candidates = lambda source: []

    self_test()


def quality_controls():
    def arguments(inv, path):
        """Keep every command's ordinary flags, then its explicit scenario flag.

        In particular an integrated compare scenario must not lose plans/mode,
        nor a diff scenario its from/to selections. No scenario is inferred.
        """
        if inv is None:
            return fixture_args(None,path)
        ordinary={k:v for k,v in inv.items() if k!='scenario'}
        if ordinary.get('command')=='check' and 'configuration' not in ordinary:
            args=['check',path]
        else:
            args=fixture_args(ordinary,path)
        if 'scenario' in inv:
            args+=['--scenario',inv['scenario']]
        return args

    def candidates(source):
        """Independent byte tokens, strings indivisible, model-direct scope only.

        An identifier called property/metric, or a quoted occurrence, is never an
        opt-out. Nested declarations remain unsupported, not migrated old jobs.
        Brace nesting is tracked without calling the new Relca parser.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]', source)]
        w = [t[0] for t in ts]
        stack = []
        found = []
        for i,t in enumerate(w):
            if t == b'{':
                # First top-level brace is the model envelope in valid DSL.
                stack.append('model' if not stack and b'model' in w[:i] else 'nested')
            elif t == b'}':
                if stack: stack.pop()
            elif stack == ['model'] and i+3 < len(w):
                if (t, w[i+1], w[i+2]) in ((b'property',b'case',b'.'),(b'metric',b'cohort',b'.')):
                    found.append(dict(kind=t.decode(), start=ts[i][1], end=ts[i+3][2]))
        return found

    def fixture_args(inv, path):
        if inv is None:
            return ["check", path]
        cmd = inv["command"]
        args = [cmd, path]
        if cmd == "check":
            args += ["--configuration", inv["configuration"]]
        elif cmd == "simulate":
            if "configuration" in inv:
                args += ["--configuration", inv["configuration"]]
        elif cmd == "diff":
            args += ["--from", inv["from"], "--to", inv["to"]]
        elif cmd == "compare":
            args += ["--plans", ",".join(inv["plans"]), "--mode", inv["mode"]]
        elif cmd == "mutate":
            if "set" in inv:
                args += ["--set", inv["set"]]
        elif cmd != "explain":
            raise ValueError(f"unknown invocation {inv}")
        return args

    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def self_test():
        for source, expected in [(b'model M { property case.X = reached(x == true) }', 1), (b'model M { property\ncase.X = reached(x == true); metric cohort.R = expected(count(case.X)) / arrivals }', 2), ('model M { cut C { question "日本語 property case.X metric cohort.R" } }'.encode(), 0), (b'model M { observed case { metric:bool=false }; process P { property = A.c() } }', 0), (b'model M { property p = x; metric m = elapsed }', 0), (b'model M { configuration C { property case.X = reached(x == true) } }', 0), (b'model M { verify probability cohort reached x == true <= 1 }', 0), (b'model M { property case.A = reached(x == true);property case.B = reached(x == true) }', 2), (b'model M { verify reachable "x property case.X = 1" }', 0), (b'model M { people ["metric cohort.R"] }', 0)]:
            assert len(candidates(source)) == expected, source
        for nested, expected in [({'original': {}, 'mutations': [{'report': {}, 'mutants': [{'report': {}}]}]}, {'/original', '/mutations/0/report', '/mutations/0/mutants/0/report'}), ({'plans': [{'report': {}}]}, {'/plans/0/report'}), ({'from': {'report': {}}, 'to': {'report': {}}}, {'/from/report', '/to/report'}), ({'explanation': {}}, {''})]:
            paths = {p for p, _ in reports(nested)}
            assert expected == paths, paths
        print('10 independent byte-token controls and canonical nested Report traversal passed')
        for inv, expected in [(None, ['check', 'p']), ({'command': 'check'}, ['check', 'p']), ({'command': 'check', 'scenario': 'Day'}, ['check', 'p', '--scenario', 'Day']), ({'command': 'check', 'configuration': 'A', 'scenario': 'Day'}, ['check', 'p', '--configuration', 'A', '--scenario', 'Day']), ({'command': 'simulate', 'configuration': 'A', 'scenario': 'Day'}, ['simulate', 'p', '--configuration', 'A', '--scenario', 'Day']), ({'command': 'compare', 'plans': ['A', 'B'], 'mode': 'simulation', 'scenario': 'Day'}, ['compare', 'p', '--plans', 'A,B', '--mode', 'simulation', '--scenario', 'Day']), ({'command': 'diff', 'from': 'A', 'to': 'B', 'scenario': 'Day'}, ['diff', 'p', '--from', 'A', '--to', 'B', '--scenario', 'Day']), ({'command': 'explain', 'scenario': 'Day'}, ['explain', 'p', '--scenario', 'Day']), ({'command': 'mutate', 'set': 'bpr-default', 'scenario': 'Day'}, ['mutate', 'p', '--set', 'bpr-default', '--scenario', 'Day'])]:
            assert arguments(inv, 'p') == expected, (inv, arguments(inv, 'p'), expected)
        print('9 independent ordered-argument controls passed, including integrated legacy and scenario recapture')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'quality':
        print('fault applied source-scanner quality', flush=True)
        candidates = lambda source: []

    self_test()


def waits_controls():
    def candidates(source):
        """Byte tokens: quoted strings indivisible; only a metrics item opts in."""
        tokens = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[^\s]', source)]
        depth = 0
        metrics = None
        found = []
        for i, (word, start, end) in enumerate(tokens):
            declaration_name = i > 0 and tokens[i-1][0] in (
                b'actor',b'process',b'cut',b'contract',b'configuration',b'scenario',b'authority',b'kernel',b'policy')
            if word == b'metrics' and not declaration_name and depth == 1 and i+1 < len(tokens) and tokens[i+1][0] == b'{':
                metrics = depth+1
            if word == b'breakdown' and depth == metrics and i+1 < len(tokens) and tokens[i+1][0] == b'waits':
                found.append(dict(start=start, end=tokens[i+1][2]))
            if word == b'}':
                if depth == metrics:
                    metrics = None
                depth -= 1
            if word == b'{':
                depth += 1
        return found

    def self_test():
        assert candidates(b'model M { actor breakdown {} process P { waits=A.x(); end done } }') == []
        assert candidates(b'model M { actor metrics { breakdown waits } }') == []
        assert candidates(b'model M { process metrics { breakdown waits } }') == []
        assert candidates(b'model M { metrics { currency "breakdown waits"; window 1min } }') == []
        assert candidates(b'model M { metrics { breakdown queue_quantiles } }') == []
        assert len(candidates(b'model M { metrics { breakdown waits } }')) == 1

    if os.environ.get('RELCA_SCANNER_FAULT') == 'waits':
        print('fault applied source-scanner waits', flush=True)
        candidates = lambda source: []

    self_test()


def quantile_controls():
    def candidates(source):
        """Independent byte-token census: quoted strings indivisible, not regex opt-out."""
        ts=[(m.group(),*m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]',source)]
        w=[t[0] for t in ts]; depth=0; sites=[]
        for i,word in enumerate(w):
            if i+3<len(w) and w[i+1:i+3]==[b'quantile',b'.']:
                if word==b'metric' and depth==1 or word in (b'minimize',b'maximize'):
                    sites.append({'kind':word.decode(),'start':ts[i][1],'end':ts[i+3][2]})
            if word==b'{':depth+=1
            if word==b'}':depth-=1
        return sites

    def self_test():
        for source, n in [(b'model M { metric quantile.X = quantile(0.5, case.P) }', 1), (b'model M { compare [A,B] cut C objectives { minimize quantile.X } }', 1), (b'model M { compare [A,B] cut C objectives { maximize quantile.X } }', 1), (b'model M { actor quantile {} process P { given = quantile.run() } }', 0), (b'model M { observed case { reached: bool = false } }', 0), ('model M { cut C { question "日本語 metric quantile.X minimize quantile.X" } }'.encode(), 0), (b'model M { metric cohort.X = quantile(count(case.X)) }', 0), (b'model M { metrics { quantiles [0.5]; p95 lead_time; median lead_time } }', 0)]:
            assert len(candidates(source)) == n, (source, n)

    if os.environ.get('RELCA_SCANNER_FAULT') == 'quantile':
        print('fault applied source-scanner quantile', flush=True)
        candidates = lambda source: []

    self_test()


def rule_actor_controls():
    PROBES = {
        'version-only': 'actor A { version "v" }',
        'version-before-bad': 'actor A { version "v"; nonsense [B] }',
        'version-string-decides': 'actor A { version "v"; boundary "decides x:bool=true"; nonsense [B] }',
        'decides-malformed': 'actor A { decides [B] }',
        'decides-truncated': 'actor A { decides x bool = true }',
        'capability-decides': 'actor A { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok }; decides x: bool = true } }',
        'under-malformed': 'people [p]; actor A {}; authority G { basis "b"; grants p to A.run; under [A] }',
        'amend-malformed': 'actor A {}; cut C { include [A] }; configuration X { cut C; process P; amend [A] }',
        'ordinary-words': 'type Words = enum { rule, change, under, amend, from, to }; actor change { capacity 1; capability run() -> T { duration fixed 1min; result nondet { ok } } }; actor rule { grain rule; boundary "decides x:bool=true amend A from r1 to r2 under G" }',
        'ordinary-change-capability': 'people [p]; actor change { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }; authority G { basis "b"; grants p to change.run }',
        'old-self-issued-gate': 'people [p]; actor A { executors [p]; capability run() -> T { duration fixed 1min; result nondet { ok } } }; authority G { basis "b"; grants p to A.run; issued_by p }',
    }

    def candidates(source):
        """Independent byte tokens; strings opaque, model version not an opt-in.

        Inspect only top-level Actor/Authority/configuration items, not identifiers
        in call paths, comments (not a language feature), strings or grain labels.
        Well-formed prefix decides IDENT ':' is required for the new Actor route.
        """
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|==|->|[^\s]', source)]
        w = [t[0] for t in ts]
        stack, pairs = [], {}
        for i,t in enumerate(w):
            if t == b'{': stack.append(i)
            elif t == b'}' and stack: pairs[stack.pop()] = i
        found=[]
        ident=lambda t: re.fullmatch(rb'[A-Za-z_][A-Za-z_0-9]*',t) is not None
        for i,t in enumerate(w):
            if t not in (b'actor', b'authority', b'configuration') or i+2 not in pairs: continue
            a,b=i+2,pairs[i+2]
            depth=0
            top=[]
            for j in range(a+1,b):
                if depth==0: top.append(j)
                if w[j]==b'{': depth+=1
                if w[j]==b'}': depth-=1
            if t==b'actor':
                for j in top:
                    if w[j]==b'decides' and j+2<b and ident(w[j+1]) and w[j+2]==b':':
                        found.append(('actor-decides',ts[j][1],ts[j+2][2]))
            elif t==b'authority':
                for j in top:
                    if w[j]==b'change' and j>=2 and w[j-1]==b'to' and j+1<b and ident(w[j+1]) and (j+2==b or w[j+2]!=b'.'):
                        found.append(('change-grant',ts[j][1],ts[j+1][2]))
                    if w[j]==b'under' and j+2<b and ident(w[j+1]) and w[j+2].startswith(b'"'):
                        found.append(('authority-under',ts[j][1],ts[j+2][2]))
            else:
                for j in top:
                    if w[j]==b'amend' and j+3<b and ident(w[j+1]) and w[j+2]==b'from' and w[j+3].startswith(b'"'):
                        found.append(('configuration-amend',ts[j][1],ts[j+3][2]))
                # All rule-target intent routes are new, including unsupported
                # override; strings and non-rule target names never match.
                for j in range(a+1,b-1):
                    if w[j]==b'rule' and j>0 and w[j-1] in (b'change',b'on',b'maintain',b'override') and ident(w[j+1]):
                        found.append(('rule-intent',ts[j][1],ts[j+1][2]))
        return sorted(set(found))

    def self_test():
        for fragment in PROBES.values():
            assert candidates(fragment.encode()) == [], fragment
        positive = [(b'actor X { version "r"; decides x:bool=true }', 'actor-decides'), (b'actor X { decides x:bool=true; version "r" }', 'actor-decides'), (b'authority G { grants [p,q] to change R }', 'change-grant'), (b'authority G { under R "r1" }', 'authority-under'), (b'configuration C { cut X; process P; amend R from "r1" to "r2" by p under G { x=false } }', 'configuration-amend'), (b'configuration C { cut X; process P; intent { preserve K on rule R } }', 'rule-intent')]
        for src, kind in positive:
            assert [c for c, _, _ in candidates(src)] == [kind], src
        assert not candidates(b'authority G { grants p to change.run }')
        assert not candidates(b'actor decides { grain rule; capability c() -> T { reads under } }')
        utf = 'actor X { boundary "日本語"; decides x:bool=true }'.encode()
        _, a, b = candidates(utf)[0]
        assert utf[a:b] == b'decides x:'
        print('independent source classifier: 20 controls passed; UTF-8 byte spans')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'rule_actor':
        print('fault applied source-scanner rule_actor', flush=True)
        candidates = lambda source: []

    self_test()


def effect_standing_controls():
    def reports(v):
        if "plans" in v:
            return [(f"/plans/{i}/report", p["report"]) for i, p in enumerate(v["plans"]) if p.get("report") is not None]
        if "original" in v:
            result = [("/original", v["original"])]
            for i, row in enumerate(v["mutations"]):
                if row.get("report") is not None:
                    result.append((f"/mutations/{i}/report", row["report"]))
                for j, mutant in enumerate(row.get("mutants", [])):
                    if mutant.get("report") is not None:
                        result.append((f"/mutations/{i}/mutants/{j}/report", mutant["report"]))
            return result
        if "from" in v and "to" in v:
            return [("/from/report", v["from"]["report"]), ("/to/report", v["to"]["report"])]
        return [("", v)]

    def targets(source):
        ts = [(m.group(), *m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|->|[^\s]', source)]
        w = [x[0] for x in ts]
        rows=[]
        def add(context, start, end):
            words=w[start:end]
            if not words:
                return
            form = ('end' if len(words)==3 and words[0]==b'end' and words[1]==b'.'
                    else 'commit' if len(words)==5 and words[1]==words[3]==b'.' and words[4]==b'commit'
                    else 'unsupported')
            rows.append(dict(context=context,kind=form,start=ts[start][1],end=ts[end-1][2]))
        for i,t in enumerate(w):
            if t==b'standing' and w[i+1:i+3]==[b'over',b'[']:
                start=i+3; depth=0
                for j in range(start,len(w)):
                    if w[j] in (b'(',b'['): depth+=1
                    if w[j] in (b')',b']'):
                        if depth==0:
                            add('standing',start,j); break
                        depth-=1
                    if w[j]==b',' and depth==0:
                        add('standing',start,j); start=j+1
            if t==b'contest' and i+2<len(w) and w[i+2]==b'on':
                end=next((j for j in range(i+3,len(w)) if w[j] in (b'route',b';',b'}')),len(w))
                add('contest',i+3,end)
            if t in (b'change',b'add',b'retire',b'preserve',b'override',b'maintain') and i+1<len(w) and w[i+1]==b'contest':
                end=i+2
                while end<len(w) and w[end] not in (b'reason',b'by',b'from',b';',b'}'):
                    end+=1
                add('contest-intent',i+2,end)
        return rows

    def self_test():
        cases = [(b'actor P { standing over [end.x] basis "s" }', ['end']), (b'standing over [\nend.x,\n A.c.commit] basis "s"', ['end', 'commit']), (b'contest P on A.c.finish(ok) route B.d reversible_by [p] respond_by 1min', ['unsupported']), (b'contest P on A.c.commit route B.d', ['commit']), (b'change contest A.c.commit reason "s"', ['commit']), (b'change contest A.c.finish reason "s"', ['unsupported']), (b'change contest A.b.c.commit reason "s"', ['unsupported']), ('basis "日本語 standing over [X.c.commit]" standing over [Q]'.encode(), ['unsupported']), (b'actor standing { capability contest() -> T {} }', []), (b'standing over [commit.c.commit] basis "standing over [a]"', ['commit'])]
        for src, kinds in cases:
            rows = targets(src)
            assert [r['kind'] for r in rows] == kinds, (src, rows)
            assert all((src[r['start']:r['end']] for r in rows))
        nested = {'from': {'report': {'diagnostics': []}}, 'to': {'report': {'diagnostics': []}}, 'other': {'diagnostics': [{}]}}
        assert [p for p, _ in reports(nested)] == ['/from/report', '/to/report']
        mut = {'original': {}, 'mutations': [{'report': {}, 'mutants': [{'report': {}}]}]}
        assert [p for p, _ in reports(mut)] == ['/original', '/mutations/0/report', '/mutations/0/mutants/0/report']
        print('10 byte-token controls and canonical nested Report traversal passed')

    if os.environ.get('RELCA_SCANNER_FAULT') == 'effect_standing':
        print('fault applied source-scanner effect_standing', flush=True)
        targets = lambda source: []

    self_test()


def conflict_controls():
    def census(source):
        """Independent byte-token walker: strings indivisible, all nested LHS.

        Extract owner/capacity/executors/uses and actual start/finish/atomic writes.
        It does NOT use the production parser or post-change reports.
        """
        ts=[(m.group(),*m.span()) for m in re.finditer(rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|\+=|-=|->|==|[^\s]',source)]
        w=[t[0] for t in ts];match={};stack=[]
        for i,t in enumerate(w):
            if t in (b'{',b'[',b'('):stack.append(i)
            elif t in (b'}',b']',b')') and stack:
                a=stack.pop()
                if (w[a],t) in ((b'{',b'}'),(b'[',b']'),(b'(',b')')):match[a]=i
        resources={}
        for i in range(len(w)-3):
            if w[i]==b'resource' and w[i+2]==b'capacity' and w[i+3].isdigit():resources[w[i+1]]=int(w[i+3])
        result=[]
        for i in range(len(w)-2):
            if w[i]!=b'actor' or w[i+2]!=b'{' or i+2 not in match:continue
            end=match[i+2];name=w[i+1];states=set();capacity=None;percase=False;caps=[];j=i+3
            while j<end:
                if w[j:j+2]==[b'instance',b'per_case']:percase=True
                if w[j]==b'capacity' and j+1<end and w[j+1].isdigit():capacity=int(w[j+1])
                if w[j]==b'executors' and w[j+1]==b'[' and j+1 in match:
                    capacity=sum(t!=b',' for t in w[j+2:match[j+1]])
                if w[j]==b'state':
                    k=j+1
                    while k<end and w[k]!=b':':
                        if w[k]!=b',':states.add(w[k])
                        k+=1
                if w[j]==b'capability' and j+2 in match:
                    k=match[j+2]+1
                    while k<end and w[k]!=b'{':k+=1
                    if k in match:caps.append((j,k,match[k]));j=match[k]+1;continue
                j=match[j]+1 if j in match else j+1
            if percase or capacity==1:continue
            writers={s:[] for s in states}
            for start,body,close in caps:
                uses={};writes={s:[] for s in states};j=body+1
                for q in range(body+1,close):
                    if w[q]==b'uses':
                        z=q+1
                        while z+2<close and w[z+1]==b':' and w[z+2].isdigit():
                            uses[w[z]]=int(w[z+2]);z+=3
                            if w[z:z+1]!=[b',']:break
                            z+=1
                    if w[q] not in (b'start',b'finish',b'atomic') or w[q+1:q+2]!=[b'{'] or q+1 not in match:continue
                    for z in range(q+2,match[q+1]-1):
                        lhs=w[z];op=z+1
                        if w[z+1:z+2]==[b'.']:
                            if lhs!=name:continue
                            lhs=w[z+2];op=z+3
                        elif z and w[z-1]==b'.':continue
                        if lhs in writes and w[op] in (b'=',b'+=',b'-='):writes[lhs].append(w[op].decode())
                for state,ops in writes.items():
                    if ops:writers[state].append(dict(capability=(name+b'.'+w[start+1]).decode(),span={'start':ts[start][1],'end':ts[close][2]},ops=ops,uses={k.decode():v for k,v in uses.items()}))
            for state,ws in sorted(writers.items()):
                for a,left in enumerate(ws):
                    for right in ws[a:]:
                        lock=next((r.decode() for r,c in resources.items() if left['uses'].get(r.decode(),0)+right['uses'].get(r.decode(),0)>c),None)
                        ops=left['ops']+right['ops']
                        kind='lock '+lock if lock else ('commutative' if len(set(ops))==1 and ops[0] in ('+=','-=') else 'unresolved')
                        result.append(dict(variable=(name+b'.'+state).decode(),left=left['capability'],right=right['capability'],span=right['span'],resolution=kind,capacity=capacity))
        return result

    def self_test():
        src = b'model M{resource R capacity 1; actor A{capacity 2;state x:bool=false;capability c()->T{uses R:1;start{x=true};finish{x=false}}}}'
        p = census(src)
        assert len(p) == 1 and p[0]['resolution'] == 'lock R'
        t = src.replace(b'R capacity 1', b'R capacity 2')
        assert t != src
        assert census(t)[0]['resolution'] == 'unresolved'
        for op in (b'+=', b'-='):
            t = src.replace(b'uses R:1;', b'').replace(b'x=true', b'x' + op + b'1').replace(b'x=false', b'x' + op + b'1')
            assert t != src
            assert census(t)[0]['resolution'] == 'commutative'
        assert census(b'actor A{capacity 2;boundary "finish{x=true}";state x:bool=false}') == []

    if os.environ.get('RELCA_SCANNER_FAULT') == 'conflict':
        print('fault applied source-scanner conflict', flush=True)
        census = lambda source: []

    self_test()


def scenario_controls():
    def candidates(source):
        """Byte-token census, strings indivisible; Cut.horizon is not new syntax."""
        ts=[(m.group(),*m.span()) for m in re.finditer(
            rb'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]',source)]
        depth=0
        found=[]
        for i,(word,a,b) in enumerate(ts):
            if word==b'scenario' and depth==1 and i+2<len(ts) and ts[i+2][0]==b'{':
                found.append(('scenario',a,b))
            if word==b'fixed_every' and i and ts[i-1][0]==b'arrivals' and depth==1:
                found.append(('fixed_every',a,b))
            if word==b'{': depth+=1
            if word==b'}': depth-=1
        return found

    def self_test():
        assert candidates(b'model M { actor fixed_every { } cut C { horizon x..y within 1min } }') == []
        assert candidates(b'model M { actor A { boundary "scenario X { arrivals fixed_every" } }') == []
        s = b'model M { scenario X {} arrivals fixed_every 1min during [0min,2min) }'
        assert [k for k, a, b in candidates(s)] == ['scenario', 'fixed_every']

    if os.environ.get('RELCA_SCANNER_FAULT') == 'scenario':
        print('fault applied source-scanner scenario', flush=True)
        candidates = lambda source: []

    self_test()


def evaluation_controls():
    def census(source):
        ts = tokens(source)
        depth = 0
        cohort, compare = [], []
        for i, (word, start, end) in enumerate(ts):
            following = [t[0] for t in ts[i:i+4]]
            if depth == 1 and following[:3] == ["metric", "cohort", "."]:
                cohort.append(dict(name=following[3], start=start))
            if depth == 1 and following[:2] == ["compare", "["]:
                compare.append(start)
            if word == "{": depth += 1
            elif word == "}": depth -= 1
        return dict(cohort=cohort, compare=compare)

    def tokens(source):
        """Quoted strings are indivisible; namesakes and binding names are not declarations."""
        return [(m.group(), m.start(), m.end()) for m in re.finditer(
            r'"(?:[^"\\]|\\.)*"|[A-Za-z_][A-Za-z_0-9]*|[^\s]', source)]

    def self_test():
        source = 'model M { process P { compare = A.run(); end compare }; metric cohort.X = q; }'
        assert census(source)['cohort'] == [dict(name='X', start=source.index('metric'))]
        for source in ['model M { actor X { meaning "metric cohort.X compare [A,B]" } }', 'model M { process P { compare = A.run(); end done } }']:
            assert census(source) == dict(cohort=[], compare=[])

    if os.environ.get('RELCA_SCANNER_FAULT') == 'evaluation':
        print('fault applied source-scanner evaluation', flush=True)
        census = lambda source: []  # type: ignore[assignment]  # Deliberately malformed fault result.

    self_test()


MIGRATED={
 'fx-aobs-capacity.broken-two-writers':('violated',1),
 'fx-aset-cases.broken-capacity':('satisfied',0),
 'fx-aset-static-capacity-with-resource.broken-capacity-with-resource':('satisfied',0),
}

def migration_oracle(job,v):
    verdict,exit_=MIGRATED[job]
    assert v['verdict']==verdict and v['mode']=='exhaustive' and v['unit']=='min'
    assert not [d for d in v['diagnostics'] if d['severity']=='error']
    assert v['bounds']['cases']==2 and v['bounds']['truncated'] is False
    cs=v['case_outcomes'];assert [c['case_id'] for c in cs]==[1,2]
    assert all(c['arrival']==0 for c in cs)
    assert v['admission']=={'rejected_count':{'min':0,'max':0}}
    props={p['id']:p for p in v['properties']}
    if 'aobs-' in job:
        assert [p['id'] for p in v['properties']]==['safety no_resource_overcommit','state_in_range']
        assert props['state_in_range']['verdict']=='violated'
        assert props['safety no_resource_overcommit']['verdict']=='inconclusive'
        assert v['unchecked']==['unreachable']
        ws=v['state_range_violations'];assert len(ws)==2
        assert {w['case_id'] for w in ws}=={1,2}
        for r in ws:
            assert (r['variable'],r['before'],r['operand'],r['attempted'],r['time'],r['phase'])==('Ledger.posted','1','1','2',1,'finish')
            assert (r['lower'],r['upper'])==('0','1')
        for c in cs:
            assert len(c['terminals'])==1 and c['terminals'][0]['end']=='posted'
            assert c['terminals'][0]==dict(kind='end',end='posted',completed_at={'min':1,'max':1},elapsed={'min':1,'max':1},work={'min':1,'max':1},result_at=None)
    else:
        assert [p['id'] for p in v['properties']]==['contract C #1 at_most_once','reachable end.done']
        for p in v['properties']:
            assert set(p)=={'id','verdict','cases'}
            assert [c['case_id'] for c in p['cases']]==[1,2]
            assert all(c['arrival']==0 and c['verdict']=='satisfied' for c in p['cases'])
        assert 'unchecked' not in v
        p=props['contract C #1 at_most_once'];assert p['verdict']=='satisfied' and 'notes' not in p
        assert props['reachable end.done']['verdict']=='satisfied'
        lock='with-resource' in job
        for i,c in enumerate(cs):
            t=c['terminals'];assert len(t)==1 and t[0]['end']=='done'
            assert t[0]['work']=={'min':1,'max':1}
            n=i+1 if lock else 1
            assert t[0]['completed_at']=={'min':n,'max':n}
            assert t[0]['elapsed']=={'min':n,'max':n} and t[0]['result_at'] is None and t[0]['kind']=='end'
        a=[r for r in v['assumptions'] if r['field']=='Ledger.keys.concurrency']
        assert len(a)==1 and a[0]['value']==('lock R' if lock else 'commutative')
    return exit_


def migration_controls(binary):
    root=Path(__file__).resolve().parent.parent
    for job,(_,exit_code) in MIGRATED.items():
        args=[str(binary), 'check', 'fixtures/'+job.removeprefix('fx-')+'/model.relca']
        output=subprocess.run(args,cwd=root,capture_output=True,check=False)
        value=json.loads(output.stdout)
        if os.environ.get('RELCA_SCANNER_FAULT') == 'migration':
            print('fault applied migration work/time',flush=True)
            value['case_outcomes'][0]['terminals'][0]['elapsed']['max']=99
        assert output.returncode==migration_oracle(job,value)==exit_code
        broken=json.loads(output.stdout)
        broken['case_outcomes'][0]['terminals'][0]['elapsed']['max']=99
        try: migration_oracle(job,broken)
        except AssertionError: pass
        else: raise AssertionError('migration hand time comparator did not reject corruption')

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary',type=Path,required=True)
    parser.add_argument('--family')
    args=parser.parse_args()
    families=['integer', 'actor_state', 'state_invariant', 'policy', 'resource', 'record', 'limits', 'quality', 'waits', 'quantile', 'rule_actor', 'effect_standing', 'conflict', 'scenario', 'evaluation']
    if args.family:
        assert args.family in families+['migration']
        families=[args.family]
    for family in families:
        if family=='migration': migration_controls(args.binary.resolve())
        else: globals()[family+'_controls']()
    if not args.family: migration_controls(args.binary.resolve())
    print('independent scanners',len(families),'and hand migration controls GREEN')

if __name__=='__main__': main()
