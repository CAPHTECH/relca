#!/usr/bin/env python3
"""Relca の出力 JSON とモデルの source から、人間が確かめるための単一 HTML を作る。

python3 標準ライブラリだけを使う。ページは 3 段:
  1. モデルが業務に合っているか(誰が・何を・どの順で・誰の承認で、仮定、Cut)
  2. 結論(確かめられていないものを先頭に、反例は DSL の行と列つき)
  3. 検査の効き(mutate の検出・未検出。渡されなければ「未実施」)

fail closed:
  - 未知の verdict は unsupported として扱う。
  - verdict と exit が食い違う、exit が無い、モデルの sha256 が合わない、
    Cut が未宣言、計算が切断された、のどれも警告として冒頭に出す。
  - 確かめられていないものは、入力に含まれる全 report(compare の各案、
    mutate の original を含む)を同じ規則で走査して集める。
  - JSON の知らない鍵では落ちず、読まなかった最上位の鍵はページに列挙する。
    型が違う欄は「読めなかった」と書いて続ける。
  - --assumptions は AI が書く申告なので、confirmed / measured も確認済みとは見せず、
    人が確かめる必要のある AI 申告として冒頭に集める。
    節の生成に失敗したら、その節に失敗を書き、確かめられていないものに足す。

使い方は skills/relca-model/SKILL.md を参照。
"""

import argparse
import datetime
import hashlib
import html
import json
import os
import sys
import traceback

MERMAID_URL = "https://cdn.jsdelivr.net/npm/mermaid@10.9.1/dist/mermaid.min.js"
# 2026-11-11 に jsdelivr と unpkg の実体の sha384 が一致し、jsdelivr API の sha256 とも一致した値
MERMAID_SRI = "sha384-WmdflGW9aGfoBdHc4rRyWzYuAjEmDwMdGdiPNacbwfGKxBW/SO6guzuQ76qjnSlr"

# crates/relca-core/src/lib.rs Verdict::exit_code
EXIT_OF = {
    "satisfied": 0,
    "violated": 1,
    "inconclusive": 2,
    "invalid": 3,
    "unsupported": 4,
    "vacuous": 5,
}
# ADR 0002 の判定の優先順(重いほど大きい)
RANK = {
    "satisfied": 0,
    "vacuous": 1,
    "inconclusive": 2,
    "unsupported": 3,
    "violated": 4,
    "invalid": 5,
}
UNVERIFIED = {"inconclusive", "unsupported", "invalid", "vacuous"}
VERDICT_JA = {
    "satisfied": "満たす",
    "violated": "破れる",
    "inconclusive": "判定不能",
    "invalid": "モデルが不正",
    "unsupported": "未対応",
    "vacuous": "空虚(前提が起きない)",
}
ROLE_JA = {
    "check": "check",
    "explain": "explain",
    "simulate": "simulate",
    "compare": "compare",
    "mutate": "mutate",
}
STATUS_JA = {"declared": "宣言", "default": "既定"}
OMIT_REASON_JA = {
    "probabilistic_property": "確率の性質なので、一本の反例経路は出ない(確率値で判定)",
    "static_property": "構造の性質なので、実行経路の反例は出ない",
}
# crates/relca-core/src/ast.rs Receipt(呼出し元の通信路の観測。結果の enum ではない)
RECEIPT_JA = {"delivered": "delivered(応答が届いた)", "unknown": "unknown(届いたか分からない)"}
# crates/relca-core/src/report.rs CaseTerminalKind
CASE_TERMINAL_JA = {
    "receipt_unknown": "応答が届いたか分からないまま止まった。業務の End ではなく、成功でも失敗でもない(未確認)",
    "end": "業務の End に届いた",
    "rejected": "受け付けで拒否された",
    "overflow": "待ち行列の上限を超えた",
    "cut_off": "計算上の切断で止まった(未確認。成功にも失敗にも数えない)",
}
STEP_KIND_JA = {"queue": "待ち行列に入った", "start": "開始した", "finish": "終わった", "cancel": "取り消した"}
RESPONSE_CONTROL_NOTE = (
    "relca は応答の受信と採用に依存する制御(response_control)を導出していない。"
    "そのため JSON の agency.diagnostics に unchecked(response_control) とある警告は出ない。"
    "警告が消えたのは問題が解決したからではなく、導出しなかったからである。"
    "このページも、その導出(F/U の観点)を代わりに行っていない。")
ABSENT = object()


def parse_call_tail(tail):
    """call の引数の後ろ(response_loss と retry)を読む。表示用で、正本は syntax.rs の call。

    返り値の rest が空でなければ、読めなかった部分がある。
    """
    toks = [t for t in lex(tail) if t.kind != "nl"]
    n = len(toks)
    out = {"response_loss": None, "retry": None, "rest": ""}

    def text(a, b):
        return tail[toks[a].start:toks[b - 1].end] if a < b else ""

    def upto(i, stops):
        j = i
        while j < n and toks[j].text not in stops:
            j += 1
        return j

    i = 0
    try:
        if i < n and toks[i].text == "response_loss":
            st = i
            i += 1
            mode = toks[i].text
            rl = {"mode": mode, "p": None, "basis": None, "sequence": None}
            if mode in ("none", "nondet"):
                i += 1
            elif mode == "sequence":
                j = upto(i, ("]",))
                if j >= n:
                    raise ValueError("sequence の ] が無い")
                rl["sequence"] = [t.text for t in toks[i + 1:j] if t.kind == "word"]
                i = j + 1
            elif mode == "probability":
                j = upto(i + 1, ("basis", "retry"))
                rl["p"] = text(i + 1, j)
                i = j
                if i < n and toks[i].text == "basis":
                    if i + 1 >= n or toks[i + 1].kind != "str":
                        raise ValueError("basis の文字列が無い")
                    rl["basis"] = toks[i + 1].text.strip('"')
                    i += 2
            else:
                raise ValueError("response_loss の方式 %s を知らない" % mode)
            rl["text"] = text(st, i)
            out["response_loss"] = rl
        if i < n and toks[i].text == "retry":
            st = i
            if toks[i + 1].text != "max_attempts" or toks[i + 2].kind != "int" or toks[i + 3].text != "when":
                raise ValueError("retry max_attempts N when の形でない")
            rt = {"max_attempts": int(toks[i + 2].text)}
            j = upto(i + 4, ("backoff",))
            rt["when"] = text(i + 4, j)
            if j + 1 >= n or toks[j + 1].text != "fixed":
                raise ValueError("backoff fixed が無い")
            k = upto(j + 2, ("exhausted",))
            rt["backoff"] = text(j + 2, k)
            if k + 2 >= n or toks[k + 1].text != "end" or toks[k + 2].kind != "word":
                raise ValueError("exhausted end NAME が無い")
            rt["exhausted"] = toks[k + 2].text
            i = k + 3
            rt["text"] = text(st, i)
            out["retry"] = rt
    except (IndexError, ValueError):
        pass
    out["rest"] = text(i, n)
    return out


def parse_kv(value):
    """'max_attempts=2; backoff=0; loss=Nondet' を dict に。形が違えば None。"""
    if not isinstance(value, str):
        return None
    out = {}
    for part in value.split(";"):
        if "=" not in part:
            return None
        k, v = part.split("=", 1)
        out[k.strip()] = v.strip()
    return out


# ---------------------------------------------------------------- 小道具

def esc(value):
    return html.escape("" if value is None else str(value), quote=True)


def get(obj, *path, default=None):
    """dict/list を型を確かめながらたどる。途中で合わなければ default。"""
    cur = obj
    for key in path:
        if isinstance(key, int):
            if isinstance(cur, list) and -len(cur) <= key < len(cur):
                cur = cur[key]
            else:
                return default
        else:
            if isinstance(cur, dict) and key in cur:
                cur = cur[key]
            else:
                return default
    return cur


def as_list(value):
    return value if isinstance(value, list) else []


def as_dict(value):
    return value if isinstance(value, dict) else {}


def sha256_file(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def short(value, n=160):
    s = value if isinstance(value, str) else json.dumps(value, ensure_ascii=False)
    return s if len(s) <= n else s[: n - 1] + "…"


def canon_verdict(raw):
    """(扱う verdict, 注記)。未知・欠落は unsupported に寄せる。"""
    if isinstance(raw, str) and raw in EXIT_OF:
        return raw, None
    if raw is None:
        return "unsupported", "verdict が無いので未対応として扱う"
    return "unsupported", "未知の verdict %s を未対応として扱う" % short(raw, 60)


def verdict_badge(v, raw=None):
    cls = {
        "satisfied": "ok",
        "violated": "bad",
    }.get(v, "warn")
    label = "%s %s" % (v, VERDICT_JA.get(v, ""))
    if raw is not None and raw != v:
        label = "%s → %s として扱う" % (short(raw, 40), v)
    return '<span class="badge %s">%s</span>' % (cls, esc(label))


# ---------------------------------------------------------------- source の字句と構文(表示用)
# 正本は crates/relca-core/src/syntax.rs。ここは表示のための粗い読みで、relca の解釈ではない。

class Tok:
    __slots__ = ("kind", "text", "start", "end")

    def __init__(self, kind, text, start, end):
        self.kind, self.text, self.start, self.end = kind, text, start, end


def lex(src):
    toks = []
    i, n = 0, len(src)
    while i < n:
        ch = src[i]
        if ch in " \t\r":
            i += 1
            continue
        if ch == "\n":
            toks.append(Tok("nl", "\n", i, i + 1))
            i += 1
            continue
        if ch.isascii() and (ch.isalpha() or ch == "_"):
            j = i + 1
            while j < n and src[j].isascii() and (src[j].isalnum() or src[j] == "_"):
                j += 1
            toks.append(Tok("word", src[i:j], i, j))
            i = j
            continue
        if ch.isdigit():
            j = i + 1
            while j < n and src[j].isdigit():
                j += 1
            toks.append(Tok("int", src[i:j], i, j))
            i = j
            continue
        if ch == '"':
            j = i + 1
            while j < n and src[j] != '"' and src[j] != "\n":
                j += 1
            j = min(j + 1, n)
            toks.append(Tok("str", src[i:j], i, j))
            i = j
            continue
        two = src[i : i + 2]
        if two in ("->", "==", "&&", "||", "<=", ">=", "!=", "+=", "-=", ".."):
            toks.append(Tok("sym", two, i, i + 2))
            i += 2
            continue
        toks.append(Tok("sym", ch, i, i + 1))
        i += 1
    return toks


class Source:
    def __init__(self, text):
        self.text = text
        self.raw = text.encode("utf-8")
        self.toks = lex(text)
        self.items = []  # (kind, name, toks)
        self.notes = []
        self._split_items()

    # バイト位置(relca の span)から行・列
    def line_col(self, byte_offset):
        if not isinstance(byte_offset, int) or byte_offset < 0:
            return None
        head = self.raw[:byte_offset]
        line = head.count(b"\n") + 1
        col = len(head[head.rfind(b"\n") + 1 :].decode("utf-8", "replace")) + 1
        return line, col

    def line_text(self, line):
        lines = self.text.split("\n")
        if isinstance(line, int) and 1 <= line <= len(lines):
            return lines[line - 1].strip()
        return ""

    def line_of_char(self, char_offset):
        return self.text.count("\n", 0, char_offset) + 1

    def slice(self, toks):
        toks = [t for t in toks if t.kind != "nl"]
        if not toks:
            return ""
        return " ".join(self.text[toks[0].start : toks[-1].end].split())

    def _split_items(self):
        toks = self.toks
        # model NAME version "x" {
        i = 0
        while i < len(toks) and toks[i].text != "{":
            i += 1
        if i >= len(toks):
            self.notes.append("model の { が見つからない")
            return
        i += 1
        depth = 1
        cur = []
        while i < len(toks):
            t = toks[i]
            if t.text == "{":
                depth += 1
            elif t.text == "}":
                depth -= 1
                if depth == 0:
                    break
            if depth == 1 and (t.kind == "nl" or t.text == ";") and t.text != "}":
                self._push_item(cur)
                cur = []
            else:
                cur.append(t)
            if depth == 1 and t.text == "}":
                # ブロックの閉じで一項目が終わる(同じ行の続きは次の項目)
                self._push_item(cur)
                cur = []
            i += 1
        self._push_item(cur)

    def _push_item(self, toks):
        toks = [t for t in toks if t.kind != "nl"] if toks and all(t.kind == "nl" for t in toks) else toks
        while toks and toks[0].kind == "nl":
            toks = toks[1:]
        if not toks:
            return
        kind = toks[0].text
        name = toks[1].text if len(toks) > 1 and toks[1].kind == "word" else None
        self.items.append((kind, name, toks))

    def items_of(self, kind):
        return [(name, toks) for k, name, toks in self.items if k == kind]

    def item_text(self, toks):
        return self.slice(toks)

    def line_of_toks(self, toks):
        for t in toks:
            if t.kind != "nl":
                return self.line_of_char(t.start)
        return None


def block_body(toks, open_index):
    """toks[open_index] が '{' のとき、対応する '}' までの中身と閉じの位置。"""
    depth = 0
    for j in range(open_index, len(toks)):
        if toks[j].text == "{":
            depth += 1
        elif toks[j].text == "}":
            depth -= 1
            if depth == 0:
                return toks[open_index + 1 : j], j
    return toks[open_index + 1 :], len(toks)


def split_top(toks):
    """深さ 0 の ';' と改行で区切る。"""
    parts, cur, depth = [], [], 0
    for t in toks:
        if t.text in "{[(" and t.kind == "sym":
            depth += 1
        elif t.text in "}])" and t.kind == "sym":
            depth -= 1
        if depth == 0 and (t.kind == "nl" or t.text == ";"):
            if cur:
                parts.append(cur)
            cur = []
            continue
        cur.append(t)
        if depth == 0 and t.text == "}":
            parts.append(cur)
            cur = []
    if cur:
        parts.append(cur)
    return [p for p in parts if p]


def first_index(toks, text):
    for i, t in enumerate(toks):
        if t.text == text:
            return i
    return -1


def parse_actor(src, toks):
    """actor NAME { ... } の欄と capability を読む。"""
    i = first_index(toks, "{")
    info = {"fields": {}, "capabilities": [], "line": src.line_of_toks(toks)}
    if i < 0:
        return info
    body, _ = block_body(toks, i)
    for part in split_top(body):
        key = part[0].text
        if key == "capability":
            info["capabilities"].append(parse_capability(src, part))
        else:
            info["fields"].setdefault(key, []).append(src.slice(part))
    return info


def parse_capability(src, toks):
    name = toks[1].text if len(toks) > 1 else "?"
    sig_end = first_index(toks, "{")
    sig = src.slice(toks[: sig_end if sig_end >= 0 else len(toks)])
    clauses = {}
    if sig_end >= 0:
        body, _ = block_body(toks, sig_end)
        for part in split_top(body):
            clauses.setdefault(part[0].text, []).append(src.slice(part))
    return {"name": name, "signature": sig, "clauses": clauses, "line": src.line_of_toks(toks)}


def parse_contract(src, toks):
    i = first_index(toks, "{")
    if i < 0:
        return []
    body, _ = block_body(toks, i)
    return [(src.slice(p), src.line_of_toks(p)) for p in split_top(body)]


# ---------- process の文

def parse_process(src, toks):
    i = first_index(toks, "{")
    if i < 0:
        return []
    body, _ = block_body(toks, i)
    return parse_block(src, body)


def parse_block(src, toks):
    stmts = []
    i = 0
    n = len(toks)
    while i < n:
        t = toks[i]
        if t.kind == "nl" or t.text == ";":
            i += 1
            continue
        line = src.line_of_char(t.start)
        statement_start = i
        head = []
        for k in range(i, n):
            if toks[k].kind != "nl":
                head.append((k, toks[k]))
                if len(head) == 3:
                    break
        merge_binding = None
        if (len(head) == 3 and head[0][1].kind == "word" and
                head[1][1].text == "=" and head[2][1].text == "if"):
            merge_binding = t.text
            i = head[2][0]
            t = toks[i]
        if t.text == "repeat" and i + 1 < n and toks[i + 1].text != "=":
            stmt, i = parse_repeat(src, toks, i)
            stmts.append(stmt)
            continue
        if t.text == "if":
            j = i + 1
            depth = 0
            while j < n and not (toks[j].text == "{" and depth == 0):
                if toks[j].text in "([":
                    depth += 1
                elif toks[j].text in ")]":
                    depth -= 1
                j += 1
            cond = src.slice(toks[i + 1 : j])
            then_body, close = block_body(toks, j) if j < n else ([], n)
            k = close + 1
            while k < n and toks[k].kind == "nl":
                k += 1
            else_body = []
            if k < n and toks[k].text == "else":
                k += 1
                while k < n and toks[k].kind == "nl":
                    k += 1
                if k < n and toks[k].text == "{":
                    else_body, close2 = block_body(toks, k)
                    k = close2 + 1
                    else_stmts = parse_block(src, else_body)
                else:
                    # else if ...: 残りを一文として読む
                    rest_start = k
                    else_stmts = parse_block(src, toks[rest_start:])
                    k = n
            else:
                else_stmts = []
            then_stmts = parse_block(src, then_body)
            st = {"kind": "merge" if merge_binding else "if", "cond": cond, "line": line,
                  "then": then_stmts, "else": else_stmts}
            if merge_binding:
                st.update(binding=merge_binding, text=src.slice(toks[statement_start:k]))
                st["span"] = {"start": len(src.text[:toks[statement_start].start].encode("utf-8")),
                              "end": len(src.text[:toks[min(k, n) - 1].end].encode("utf-8"))}
                for body in (then_stmts, else_stmts):
                    for sub in body:
                        if sub["kind"] == "yield":
                            sub["binding"] = merge_binding
                if (any(not body or body[-1]["kind"] not in ("yield", "end") or
                        any(s["kind"] == "yield" for s in body[:-1]) for body in (then_stmts, else_stmts))
                        or all(body and body[-1]["kind"] == "end" for body in (then_stmts, else_stmts))):
                    st["kind"], st["reason"] = "rejected_merge", "枝の末尾に yield または end が必要。少なくとも一方は yield"
            stmts.append(st)
            i = k
            continue
        if t.text == "fork":
            name = toks[i + 1].text if i + 1 < n else "?"
            j = i + 2
            while j < n and toks[j].text != "{":
                j += 1
            body, close = block_body(toks, j) if j < n else ([], n)
            stmts.append({"kind": "fork", "name": name, "line": line, "body": parse_block(src, body)})
            i = close + 1
            continue
        # 一文: 深さ 0 の区切りまで。join any は次の行の tie/remaining/if none も含める。
        j = i
        depth = 0
        while j < n:
            x = toks[j]
            if x.text in "{[(" and x.kind == "sym":
                depth += 1
            elif x.text in "}])" and x.kind == "sym":
                depth -= 1
            if depth == 0 and (x.kind == "nl" or x.text == ";"):
                if t.text == "join":
                    k = j
                    while k < n and toks[k].kind == "nl":
                        k += 1
                    if k < n and (
                        toks[k].text in ("tie", "remaining")
                        or (toks[k].text == "if" and k + 1 < n and toks[k + 1].text == "none")
                    ):
                        j = k
                        continue
                break
            j += 1
        part = toks[i:j]
        stmts.append(classify_stmt(src, part, line))
        i = j
    return stmts


def parse_repeat(src, toks, start):
    """表示用の有限 repeat。未知の拡張を正常な loop に読み替えない。"""
    j = start
    while j < len(toks) and toks[j].text != "{":
        j += 1
    body, close = block_body(toks, j) if j < len(toks) else ([], len(toks))
    k = close + 1
    while k < len(toks) and (toks[k].kind == "nl" or toks[k].text == ";"):
        k += 1
    limit, end = [], close
    has_limit = k < len(toks) and toks[k].text == "on_limit"
    if has_limit:
        k += 1
        while k < len(toks) and toks[k].kind == "nl":
            k += 1
        has_limit = k < len(toks) and toks[k].text == "{"
        if has_limit:
            limit, end = block_body(toks, k)
    head = [t for t in toks[start:j] if t.kind != "nl"]
    w = [t.text for t in head]
    st = {"kind": "repeat", "name": w[1] if len(w) > 1 else "?",
          "line": src.line_of_toks(toks[start:]), "text": src.slice(toks[start:end + 1]),
          "body": parse_block(src, body), "on_limit": parse_block(src, limit),
          "at_most": None, "gap": None, "header": src.slice(head)}
    reason = None
    if "over" in w[2:]:
        reason = "loop_stage_unsupported: over"
    elif (len(head) not in (4, 8) or head[1].kind != "word" or w[2] != "at_most"
          or head[3].kind != "int"):
        reason = "repeat NAME at_most N [gap fixed DURATION] が必要"
    else:
        st["at_most"] = int(w[3])
        if len(head) == 8:
            if w[4:6] != ["gap", "fixed"] or head[6].kind != "int" or w[7] not in ("min", "h", "d"):
                reason = "gap fixed DURATION が必要"
            else:
                st["gap"] = "gap fixed " + "".join(w[6:8])
        if not 1 <= st["at_most"] <= 32:
            reason = "at_most は初回を含む1〜32回"
    if not has_limit or end >= len(toks) or close >= len(toks):
        reason = reason or "body と on_limit の block が必要"

    def control_paths(stmts, phase, fallthrough=True):
        """lower/repetition.rs の構造上の閉じ方だけを読む。時間や型の検査は Relca に任せる。"""
        for index, s in enumerate(stmts):
            k = s["kind"]
            if k in ("again", "leave") and index + 1 < len(stmts):
                return False, False
            if k in ("again", "leave", "end"):
                if k == "again" and phase == "on_limit":
                    return False, False
                fallthrough = False
            elif k == "if":
                tf, tv = control_paths(s["then"], phase, fallthrough)
                ef, ev = control_paths(s["else"], phase, fallthrough)
                if not tv or not ev:
                    return False, False
                fallthrough = tf or ef
        return fallthrough, True

    for s in flatten_steps(st["body"] + st["on_limit"]):
        if (s["kind"] not in ("call", "if", "wait", "again", "leave", "end")
                or (s["kind"] == "call" and s.get("tail"))
                or (s["kind"] == "wait" and s["text"].split()[1:2] == ["until"])):
            reason = reason or "loop_feature_combination: 入れ子・未対応の文や呼出し suffix"
    if control_paths(st["body"], "body") != (False, True) or control_paths(st["on_limit"], "on_limit") != (False, True):
        reason = reason or "loop の経路が閉じない、on_limit の again、または制御文の後続"
    if reason:
        st["kind"], st["reason"] = "rejected_repeat", reason
    return st, min(end + 1, len(toks))


def repeat_label(stmt):
    return "repeat %s at_most %s / %s" % (stmt["name"], stmt["at_most"], stmt["gap"] or "gap 0min(既定)")


def classify_stmt(src, part, line):
    words = [t for t in part if t.kind != "nl"]
    text = src.slice(words)
    if not words:
        return {"kind": "unknown", "text": text, "line": line}
    w0 = words[0].text
    if w0 == "yield" and len(words) == 2 and words[1].kind == "word":
        return {"kind": "yield", "source": words[1].text, "line": line, "text": text}
    if (w0 == "yield" and len(words) == 4 and words[1].kind == "word"
            and words[2].text == "." and words[3].kind == "word"):
        return {"kind": "yield", "source": words[1].text + "." + words[3].text, "line": line, "text": text}
    if w0 in ("again", "leave") and len(words) == 1:
        return {"kind": w0, "text": text, "line": line}
    if w0 == "end" and len(words) >= 2:
        return {"kind": "end", "name": words[1].text, "line": line, "text": text}
    if w0 == "join":
        return {"kind": "join", "text": text, "line": line,
                "name": words[2].text if len(words) > 2 else "?"}
    if w0 == "wait":
        return {"kind": "wait", "text": text, "line": line}
    # binding = Actor.cap(args)
    if (len(words) >= 6 and words[1].text == "=" and words[3].text == "."
            and words[5].text == "("):
        close = len(words) - 1
        depth = 0
        for k in range(5, len(words)):
            if words[k].text == "(":
                depth += 1
            elif words[k].text == ")":
                depth -= 1
                if depth == 0:
                    close = k
                    break
        args = src.slice(words[6:close])
        tail = src.slice(words[close + 1 :])
        return {"kind": "call", "binding": w0, "actor": words[2].text,
                "capability": words[4].text, "args": args, "tail": tail,
                "line": line, "text": text}
    return {"kind": "unknown", "text": text, "line": line}


def flatten_steps(stmts, conds=None, out=None, branch=None):
    conds = conds or []
    out = [] if out is None else out
    for s in stmts:
        if s["kind"] in ("if", "merge"):
            out.append(dict(s, conds=list(conds), branch=branch))
            flatten_steps(s["then"], conds + ["%s のとき" % s["cond"]], out, branch)
            flatten_steps(s["else"], conds + ["%s でないとき" % s["cond"]], out, branch)
        elif s["kind"] == "fork":
            out.append(dict(s, conds=list(conds), branch=branch))
            flatten_steps(s["body"], conds, out, "fork %s(並行)" % s["name"])
        elif s["kind"] == "repeat":
            out.append(dict(s, conds=list(conds), branch=branch))
            flatten_steps(s["body"], conds + ["repeat %s body(初回を含む)" % s["name"]], out, branch)
            flatten_steps(s["on_limit"], conds + ["repeat %s on_limit(最後の again の後だけ)" % s["name"]], out, branch)
        else:
            out.append(dict(s, conds=list(conds), branch=branch))
    return out


def mermaid_flow(stmts, executors_of):
    lines = ["flowchart TD"]
    counter = [0]
    ends = {}

    def node(label, shape="rect", cls=None):
        counter[0] += 1
        nid = "n%d" % counter[0]
        lab = mermaid_label(label)
        if shape == "diamond":
            lines.append('  %s{"%s"}' % (nid, lab))
        elif shape == "stadium":
            lines.append('  %s(["%s"])' % (nid, lab))
        else:
            lines.append('  %s["%s"]' % (nid, lab))
        if cls:
            lines.append("  class %s %s" % (nid, cls))
        return nid

    def connect(pend, target):
        for src_id, label in pend:
            if label:
                lines.append('  %s -->|"%s"| %s' % (src_id, label, target))
            else:
                lines.append("  %s --> %s" % (src_id, target))

    def build(stmts, pend, loop=None):
        for s in stmts:
            if loop is not None and not pend:
                break  # end の後の source は表に残すが、loop 内の到達可能な経路には戻さない。
            k = s["kind"]
            if k == "call":
                who = executors_of(s["actor"])
                label = "%s.%s(%s)\n担当: %s\n→ %s" % (s["actor"], s["capability"], s["args"], who, s["binding"])
                n = node(label)
                connect(pend, n)
                pend = [(n, None)]
            elif k == "fork":
                f = node("fork %s(並行に開始)" % s["name"], "stadium")
                connect(pend, f)
                exits = []
                for sub in s["body"]:
                    exits += build([sub], [(f, None)])
                pend = exits
            elif k == "join":
                j = node(s["text"], "stadium")
                connect(pend, j)
                pend = [(j, None)]
            elif k in ("if", "merge"):
                d = node((s["binding"] + " = if " if k == "merge" else "") + s["cond"], "diamond")
                connect(pend, d)
                pend = build(s["then"], [(d, "はい")], loop) + build(s["else"], [(d, "いいえ")], loop)
            elif k == "yield":
                y = node("yield %s → %s(値の複写。Task なし)" % (s["source"], s.get("binding", "?")), "stadium")
                connect(pend, y)
                pend = [(y, None)]
            elif k == "repeat":
                entry = node(repeat_label(s), "stadium")
                body = node("body %s(初回は1回目)" % s["name"], "stadium")
                limit = node("on_limit %s" % s["name"], "stadium")
                has_leave = any(x["kind"] == "leave" for x in flatten_steps(s["body"] + s["on_limit"]))
                after = node("leave %s(繰り返しの後へ)" % s["name"], "stadium") if has_leave else None
                connect(pend, entry)
                connect([(entry, "初回(gap なし)")], body)
                ctx = {"body": body, "on_limit": limit, "after": after, "gap": s["gap"] or "gap 0min"}
                build(s["body"], [(body, None)], ctx)
                build(s["on_limit"], [(limit, None)], ctx)
                # Only a leave connects to the continuation; end/again never do.
                pend = [(after, None)] if has_leave else []
            elif k == "again" and loop:
                a = node("again", "diamond")
                connect(pend, a)
                connect([(a, "次の回へ(%s、次の回があるときだけ)" % loop["gap"])], loop["body"])
                connect([(a, "最後の回なら on_limit(gap なし)")], loop["on_limit"])
                pend = []
            elif k == "leave" and loop:
                connect([(p, "leave(繰り返しを抜ける)" + (" / " + label if label else "")) for p, label in pend], loop["after"])
                pend = []
            elif k == "end":
                if s["name"] not in ends:
                    ends[s["name"]] = node("end %s" % s["name"], "stadium", "endnode")
                connect(pend, ends[s["name"]])
                pend = []
            elif k == "wait":
                w = node(s["text"])
                connect(pend, w)
                pend = [(w, None)]
            elif k in ("rejected_repeat", "rejected_merge"):
                u = node("Relca が受け付けない形: %s\n%s" % (s["reason"], s["text"]), cls="unknown")
                connect(pend, u)
                pend = []
            else:
                u = node("未解釈: %s" % s["text"], cls="unknown")
                connect(pend, u)
                pend = [(u, None)]
        return pend

    start = node("案件の受付", "stadium")
    rest = build(stmts, [(start, None)])
    if rest:
        dangling = node("終端の宣言なし", cls="unknown")
        connect(rest, dangling)
    lines.append("  classDef endnode fill:#e8f0fe,stroke:#335;")
    lines.append("  classDef unknown fill:#fff3cd,stroke:#a60,stroke-dasharray: 4 3;")
    return "\n".join(lines)


# ---------------------------------------------------------------- 入力

class Run:
    def __init__(self, role, path, exit_code, stderr_path):
        self.role = role
        self.path = path
        self.exit = exit_code
        self.stderr_path = stderr_path
        self.stderr = ""
        self.data = None
        self.load_error = None
        self.sha256 = None
        try:
            self.sha256 = sha256_file(path)
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
            if not isinstance(data, dict):
                raise ValueError("最上位が object ではない")
            self.data = data
        except Exception as e:  # noqa: BLE001  読めない入力は未対応として残す
            self.load_error = "%s: %s" % (type(e).__name__, e)
        if stderr_path:
            try:
                with open(stderr_path, "r", encoding="utf-8", errors="replace") as f:
                    self.stderr = f.read()
            except OSError as e:
                self.stderr = "(stderr を読めない: %s)" % e
        raw = get(self.data, "verdict") if self.data is not None else None
        self.raw_verdict = raw
        if self.load_error:
            self.verdict, self.verdict_note = "unsupported", "JSON を読めないので未対応として扱う(%s)" % self.load_error
        else:
            self.verdict, self.verdict_note = canon_verdict(raw)
        if self.role == "check" and get(self.data, "explanation") is not None:
            self.role = "explain"

    @property
    def mode(self):
        return get(self.data, "mode")

    def version(self):
        v = get(self.data, "ledger", "producer", "version")
        src = get(self.data, "ledger", "producer", "version_source")
        bid = get(self.data, "ledger", "producer", "build_identity", "status")
        if v is None:
            return "不明(JSON に ledger.producer.version が無い)"
        extra = []
        if src:
            extra.append("出所 %s" % src)
        if bid:
            extra.append("build identity %s" % bid)
        return "%s(%s)" % (v, "、".join(extra)) if extra else str(v)

    def model_sha(self):
        return (get(self.data, "model", "sha256")
                or get(self.data, "original", "model", "sha256"))


# 走査で読む鍵。report の種類ごとに、これに加えて表示で読む鍵がある。
# これ以外の最上位の鍵は「読まなかった鍵」としてページに出す。
SCAN_KEYS = {"verdict", "cut", "bounds", "properties", "diagnostics", "unchecked", "agency", "repetition",
             "explanation", "mismatch", "model", "ledger", "mode", "contract_scope"}
SHOWN_KEYS = {
    "main": {"actors", "cut_context", "metrics", "responsibilities", "assumptions", "uncertainty", "terminals", "unit",
             "response_receipts", "response_analysis", "case_outcomes"},
    "simulate": {"simulation", "terminals", "unit"},
    "plan": {"actors", "terminals"},
    "mutate_original": set(),
    "compare": {"analysis", "plans", "tables", "assumptions"},
    "mutate": {"set", "original", "original_verdict", "run", "mutations", "assumptions"},
}
UNCHECKED_PATHS = (("unchecked",), ("agency", "unchecked"), ("cut", "unchecked"), ("explanation", "unchecked"),
                   ("response_analysis", "unchecked"), ("repetition", "unchecked"))
# --assumptions は AI が書くファイル。confirmed / measured も AI の申告であり、確認済みとして見せない。
# 緑(ok)・decl のバッジは使わない。
AI_STATUS = {
    "placeholder": ("warn", "AI の仮置き(実測ではない)"),
    "confirmed": ("warn", "AI の申告: 確認済み(confirmed_by %s)。人が未確認"),
    "measured": ("warn", "AI の申告: 実測(出典 %s)。人が未確認"),
}


def ai_label(a):
    cls, label = AI_STATUS[a["status"]]
    if a["status"] == "confirmed":
        label = label % short(a.get("confirmed_by"), 60)
    elif a["status"] == "measured":
        label = label % short(a.get("source"), 60)
    return '<span class="badge %s">%s</span>' % (cls, esc(label))


def fold_report(data):
    """crates/relca-core/src/report.rs fold_verdict の写し。性質・error 診断・deadlock を畳む。"""
    props = [p for p in as_list(get(data, "properties")) if isinstance(p, dict)]
    diags = [d for d in as_list(get(data, "diagnostics")) if isinstance(d, dict)]
    vs = [canon_verdict(p.get("verdict"))[0] for p in props]
    for d in diags:
        if d.get("severity") == "error":
            vs.append("unsupported" if str(d.get("code", "")).startswith("U-") else "invalid")
    if props:
        vs += ["violated" for d in diags if d.get("code") == "W-DEADLOCK-0"]
    if not vs:
        return "unsupported"
    return max(vs, key=lambda v: RANK[v])


def mermaid_label(text):
    """Mermaid の label 用。HTML として解釈される文字は数値の entity にする。"""
    return (text.replace("&", "#38;").replace("<", "#60;").replace(">", "#62;")
            .replace('"', "#quot;").replace("\n", "<br/>"))


# ---------------------------------------------------------------- ページ

class Page:
    def __init__(self, args):
        self.args = args
        self.alerts = []          # (重さ, 文)
        self.unverified = []      # (出所, 対象, 扱い, 理由)
        self.self_unchecked = {}  # 項目 -> [出所]
        self.relca_diags = []     # (出所, diagnostic, Source)
        self.not_derived = []     # (出所, path, 理由)
        self.basis_unknown = []   # (出所, path)
        self.unread = {}          # 鍵 -> [出所]
        self.mechanism = []       # (出所, 性質)
        self.bounds_rows = []     # (出所, bounds, scenario)
        self.not_run = []
        self.ai_assumptions = []
        self._decls = None
        self.unprovenanced = []  # (target, 種類, 宣言, 行)
        self.unreadable_rows = []  # (出所, 欄, 理由) 新しい欄の型違い・欠落・未知の語
        self.resp_steps = []      # 反例の command のある段の読み
        self.case_view = []       # case_outcomes の案件ごとの読み
        self.repetitions = []     # (出所, repetition, Source)。全 report を同じ規則で読む。
        self.contract_scopes = []  # (出所, 範囲外の契約)。verdict には混ぜない。
        self.runs = []
        self.model_path = args.model
        self.model_sha = None
        self.src = None
        try:
            with open(args.model, "r", encoding="utf-8") as f:
                text = f.read()
            self.model_sha = sha256_file(args.model)
            self.src = Source(text)
        except Exception as e:  # noqa: BLE001
            self.alert("error", "モデルの source を読めない: %s。業務との照合の節は JSON だけから作る" % e)
        self.compare_src = None
        self.compare_sha = None
        if args.compare_model:
            try:
                with open(args.compare_model, "r", encoding="utf-8") as f:
                    self.compare_src = Source(f.read())
                self.compare_sha = sha256_file(args.compare_model)
            except Exception as e:  # noqa: BLE001
                self.alert("error", "compare のモデルを読めない: %s" % e)
        for role in ("check", "simulate", "compare", "mutate"):
            path = getattr(args, role)
            if path:
                self.runs.append(Run(role, path, getattr(args, role + "_exit"),
                                     getattr(args, role + "_stderr")))
            elif role != "check":
                self.not_run.append("%s(入力に渡されていない)" % role)
        self.main = self.runs[0] if self.runs and self.runs[0].role in ("check", "explain") else None
        if self.main is not None and self.main.role == "check":
            self.not_run.append("explain(check の JSON なので反例と DSL 位置が無い)")
        self._load_ai_assumptions()
        claimed = {a["target"] for a in self.ai_assumptions}
        self.unprovenanced = [d for d in self.declarations() if d[0] not in claimed]
        self._check_runs()
        for r in self.runs:
            self._scan_run(r)

    def alert(self, level, text):
        self.alerts.append((level, text))

    def unv(self, origin, what, how, why):
        self.unverified.append((str(origin), str(what), str(how), str(why)))

    def run(self, role):
        for r in self.runs:
            if r.role == role or (role == "check" and r.role == "explain"):
                return r
        return None

    # ---- AI が置いた値
    def _load_ai_assumptions(self):
        path = self.args.assumptions
        if not path:
            self.not_run.append("--assumptions(AI が置いた値の一覧)が渡されていない。モデルの所要時間・確率が実測か仮置きかを区別できない")
            return
        try:
            with open(path, "r", encoding="utf-8") as f:
                data = json.load(f)
        except Exception as e:  # noqa: BLE001
            self.alert("error", "--assumptions を読めない: %s" % e)
            self.unv("assumptions", "AI が置いた値", "unsupported", "読めない: %s" % e)
            return
        items = data.get("assumptions") if isinstance(data, dict) else data
        if not isinstance(items, list):
            self.alert("error", "--assumptions の形が違う(list か {\"assumptions\": list})")
            self.unv("assumptions", "AI が置いた値", "unsupported", "形が違う")
            return
        declared = {d[0] for d in self.declarations()}
        seen = set()
        for it in items:
            if not isinstance(it, dict):
                self.alert("warn", "--assumptions に object でない要素: %s" % short(it, 80))
                continue
            status = it.get("status")
            # 根拠を伴わない confirmed / measured は仮置きに戻す(fail closed)
            if status == "confirmed" and not it.get("confirmed_by"):
                status = "placeholder"
            if status == "measured" and not it.get("source"):
                status = "placeholder"
            if status not in AI_STATUS:
                status = "placeholder"
            target = str(it.get("target", ""))
            if target in seen:
                self.alert("warn", "--assumptions の target %s が重複している。どちらの申告が正しいか分からない" % short(target, 80))
            seen.add(target)
            if self.src and target not in declared:
                self.alert("warn", "--assumptions の target %s がモデルの数値・選択の宣言に対応しない" % short(target, 80))
            self.ai_assumptions.append(dict(it, status=status, target=target))

    def ai_status_of(self, target):
        found = [a for a in self.ai_assumptions if a["target"] == target]
        if found:
            return " ".join(ai_label(a) for a in found)
        return '<span class="badge warn">出所の申告なし</span>'

    def declarations(self):
        """モデルにある数値と選択の宣言を source から列挙する。(target, 種類, 宣言, 行)。

        bounds と simulation は計算の設定なので含めない(上限の表に出す)。
        """
        if self._decls is not None:
            return self._decls
        out = []
        src = self.src
        if src is None:
            self._decls = out
            return out

        def add(target, kind, text, line):
            out.append((target, kind, text, line))

        def body_parts(toks):
            i = first_index(toks, "{")
            return split_top(block_body(toks, i)[0]) if i >= 0 else []

        def var_of(text):
            words = text.replace(":", " : ").split()
            return words[1] if len(words) > 1 else "?"

        for name, toks in src.items_of("actor"):
            info = parse_actor(src, toks)
            for v in info["fields"].get("via", []):
                if [t.text for t in lex(v)][:2] == ["via", "model"]:
                    add("%s.via_model" % name, "経由するモデルの版", v, info["line"])
            for v in info["fields"].get("capacity", []):
                add("%s.capacity" % name, "容量", v, info["line"])
            for v in info["fields"].get("latent", []):
                if "~" in v:
                    add("%s.%s" % (name, var_of(v)), "潜在の初期分布", v, info["line"])
            for c in info["capabilities"]:
                for v in c["clauses"].get("duration", []):
                    add("%s.%s.duration" % (name, c["name"]), "所要時間", v, c["line"])
                for v in c["clauses"].get("uses", []):
                    add("%s.%s.uses" % (name, c["name"]), "資源の使用量", v, c["line"])
                for v in c["clauses"].get("idempotency", []):
                    add("%s.%s.idempotency" % (name, c["name"]), "冪等キー(現実の機構の仮定)", v, c["line"])
                for v in c["clauses"].get("effect_scope", []):
                    add("%s.%s.effect_scope" % (name, c["name"]), "効果の範囲(現実の機構の仮定)", v, c["line"])
        for name, toks in src.items_of("kernel"):
            i = first_index(toks, "{")
            add("kernel.%s" % name, "確率の表", src.slice(toks[:i]) if i >= 0 else src.slice(toks),
                src.line_of_toks(toks))
        for kind in ("latent", "observed"):
            for _, toks in src.items_of(kind):
                for part in body_parts(toks):
                    text = src.slice(part)
                    if "~" in text:
                        add("case.%s" % part[0].text, "潜在の初期分布" if kind == "latent" else "観測の初期分布",
                            text, src.line_of_toks(part))
        for name, toks in src.items_of("resource"):
            add("resource.%s" % name, "資源の容量", src.item_text(toks), src.line_of_toks(toks))
        for _, toks in src.items_of("metrics"):
            for part in body_parts(toks):
                w = [t.text for t in part]
                if w and w[0] == "rate" and len(w) > 1:
                    end = w.index("per_minute") if "per_minute" in w else (w.index("per_start") if "per_start" in w else 2)
                    add("metrics.rate.%s" % "".join(w[1:end]), "単価", src.slice(part), src.line_of_toks(part))
                elif w and w[0] == "window":
                    add("metrics.window", "分析窓", src.slice(part), src.line_of_toks(part))
        for kind, label in (("policy", "方策"), ("arrivals", "流入"), ("queue", "待ち行列の上限"),
                            ("concurrent", "同時案件の上限"), ("calendar", "カレンダー"),
                            ("person_calendar", "人のカレンダー"), ("case_priority", "優先度")):
            items = src.items_of(kind)
            for k, (name, toks) in enumerate(items, 1):
                tgt = kind if len(items) == 1 else "%s#%d" % (kind, k)
                if kind == "calendar" and name:
                    tgt = "calendar.%s" % name
                add(tgt, label, src.item_text(toks), src.line_of_toks(toks))
        for name, toks in src.items_of("scenario"):
            for part in body_parts(toks):
                if part[0].text in ("arrivals", "queue", "concurrent", "horizon"):
                    add("scenario.%s.%s" % (name, part[0].text), "シナリオの %s" % part[0].text,
                        src.slice(part), src.line_of_toks(part))
        probs = [t for _, t in src.items_of("verify") if len(t) > 1 and t[1].text == "probability"]
        for k, toks in enumerate(probs, 1):
            add("verify.probability#%d" % k, "確率の上限", src.item_text(toks), src.line_of_toks(toks))
        for name, toks in src.items_of("contract"):
            for k, (text, line) in enumerate(parse_contract(src, toks), 1):
                if any(ch.isdigit() for ch in text) and any(w in text.split() for w in ("by", "respond_by", "within")):
                    add("contract.%s#%d" % (name, k), "契約の期限", text, line)
        for name, toks in src.items_of("cut"):
            for part in body_parts(toks):
                if part[0].text == "horizon":
                    add("cut.%s.horizon@%d" % (name, src.line_of_toks(part)), "Cut の時間地平",
                        src.slice(part), src.line_of_toks(part))
        for name, toks in src.items_of("process"):
            for st in flatten_steps(parse_process(src, toks)):
                if st["kind"] == "repeat":
                    add("%s.at_most" % st["name"], "繰り返しの上限回数", "at_most %s" % st["at_most"], st["line"])
                    if st["gap"] is not None:
                        add("%s.gap" % st["name"], "繰り返しの間隔", st["gap"], st["line"])
                if st["kind"] == "wait":
                    add("%s.wait@%d" % (name, st["line"]), "待ち", st["text"], st["line"])
        for rd in self.response_decls():
            if rd["response_loss"]:
                add(rd["target_loss"], "応答喪失の方式", rd["response_loss"]["text"], rd["line"])
            if rd["retry"]:
                add(rd["target_retry"], "再送(上限回数・間隔・尽きたときの行先)", rd["retry"]["text"], rd["line"])
        self._decls = out
        return out

    def response_decls(self):
        """process の call に付いた response_loss と retry。target は retry.<binding> と response_loss.<binding>。

        同じ binding が複数の process にあれば、その全部を retry.<process>.<binding> にする。
        """
        if getattr(self, "_rdecls", None) is not None:
            return self._rdecls
        out = []
        src = self.src
        for pname, toks in (src.items_of("process") if src else []):
            for st in flatten_steps(parse_process(src, toks)):
                if st["kind"] != "call" or not st.get("tail"):
                    continue
                parsed = parse_call_tail(st["tail"])
                out.append(dict(parsed, process=pname, binding=st["binding"], actor=st["actor"],
                                capability=st["capability"], line=st["line"], tail=st["tail"]))
        counts = {}
        for rd in out:
            counts[rd["binding"]] = counts.get(rd["binding"], 0) + 1
        for rd in out:
            key = rd["binding"] if counts[rd["binding"]] == 1 else "%s.%s" % (rd["process"], rd["binding"])
            rd["target_retry"] = "retry.%s" % key
            rd["target_loss"] = "response_loss.%s" % key
        self._rdecls = out
        return out

    # ---- 入力ごと(プロセスの exit と sha256)
    def _check_runs(self):
        for r in self.runs:
            label = "%s(%s)" % (r.role, r.path)
            if r.load_error:
                self.alert("error", "%s: JSON を読めない(%s)。未対応として扱う" % (label, r.load_error))
                self.unv(r.role, "結果全体", "unsupported", "JSON を読めない")
                continue
            if r.exit is None:
                self.alert("error", "%s: exit が渡されていない。verdict と終了コードを照合していないので、合格として扱わない" % label)
                self.unv(r.role, "exit", "exit 未確認", "終了コードが渡されていない。verdict %s を確かめていない" % short(r.raw_verdict, 40))
                if r.verdict == "satisfied":
                    r.verdict = "unsupported"
                    r.verdict_note = "exit 未確認なので satisfied を合格として扱わない"
            else:
                expected = EXIT_OF.get(r.raw_verdict) if isinstance(r.raw_verdict, str) else None
                if expected is None or expected != r.exit:
                    self.alert("error", "%s: JSON の verdict %s と exit %s が食い違う(verdict から期待する exit は %s)。"
                               "この結果を合格として扱わない" % (label, short(r.raw_verdict, 40), r.exit,
                                                       "無し" if expected is None else expected))
                    self.unv(r.role, "exit", r.verdict, "verdict %s と exit %s が食い違う" % (short(r.raw_verdict, 40), r.exit))
                    if r.verdict == "satisfied":
                        r.verdict = "unsupported"
                        r.verdict_note = "exit が 0 でないので satisfied を未対応として扱う"
            if r.role in ("check", "explain", "simulate", "mutate") and self.model_sha:
                js = r.model_sha()
                if js and js != self.model_sha:
                    self.alert("error", "%s: JSON の model.sha256 %s… が --model の sha256 %s… と違う。別のモデルの結果の可能性がある"
                               % (label, str(js)[:12], self.model_sha[:12]))
                elif not js:
                    self.alert("warn", "%s: JSON に model.sha256 が無く、モデルとの対応を確かめていない" % label)
            if r.role == "compare":
                if not self.compare_sha:
                    self.alert("warn", "compare: --compare-model が無いので、案の model.sha256 を照合していない")
                for pl in as_list(get(r.data, "plans")):
                    js = get(pl, "report", "model", "sha256")
                    if self.compare_sha and js != self.compare_sha:
                        self.alert("error", "compare の案 %s: model.sha256 が --compare-model と違う" % short(get(pl, "configuration"), 60))
            for line in r.stderr.splitlines():
                code = line.split(":", 1)[0].strip()
                tail = code.split(" ")[-1] if code else ""
                if tail.startswith(("E-", "U-")) or (": E-" in line) or (": U-" in line):
                    self.alert("warn", "%s の stderr: %s" % (r.role, short(line, 240)))

    # ---- 全 report を同じ規則で走査する
    def _scan_run(self, r):
        if r.data is None:
            return
        if r.role in ("check", "explain"):
            self.scan(r.role, r.data, "main", self.src)
        elif r.role == "simulate":
            self.scan("simulate", r.data, "simulate", self.src)
        elif r.role == "compare":
            self.scan("compare", r.data, "compare", None)
            for pl in as_list(r.data.get("plans")):
                name = get(pl, "configuration")
                self.scan("compare 案 %s" % short(name, 40), get(pl, "report"), "plan", self.compare_src)
        elif r.role == "mutate":
            self.scan("mutate", r.data, "mutate", None)
            self.scan("mutate original", r.data.get("original"), "mutate_original", self.src)

    def scan(self, origin, data, kind, src):
        if not isinstance(data, dict):
            self.unv(origin, "結果全体", "unsupported", "report が object でない(%s)" % type(data).__name__)
            self.alert("error", "%s: report を読めない" % origin)
            return
        v, note = canon_verdict(data.get("verdict"))
        if note:
            self.alert("error", "%s: %s" % (origin, note))
            self.unv(origin, "結果全体", "unsupported", note)
        elif v in UNVERIFIED:
            self.unv(origin, "結果全体", v, self._reason_of(data))
        # 全体の判定の照合
        if kind in ("main", "simulate", "plan", "mutate_original"):
            folded = fold_report(data)
            self._compare_fold(origin, v, folded, "性質と診断をまとめた判定")
        elif kind == "compare":
            plan_vs = [canon_verdict(get(pl, "report", "verdict"))[0] for pl in as_list(data.get("plans"))]
            if plan_vs:
                self._compare_fold(origin, v, max(plan_vs, key=lambda x: RANK[x]), "各案をまとめた判定")
            else:
                self.unv(origin, "plans", "unsupported", "案が一つも無い")
        # Cut
        diags = [d for d in as_list(data.get("diagnostics")) if isinstance(d, dict)]
        if kind in ("main", "simulate", "plan", "mutate_original"):
            cut = data.get("cut")
            model = data.get("model")
            cut_warning = any(d.get("code") == "W-CUT-0" for d in diags)
            warning_note = "diagnostics に W-CUT-0 がある" if cut_warning else "diagnostics に W-CUT-0 は無い"
            # SYN codes can also originate in lower. Only report metadata
            # establishes whether parsing completed; the code/message is detail.
            if cut == "undeclared":
                self.unv(origin, "Cut", "undeclared", "Cut が宣言されていない(cut: undeclared)。何をモデルから外したか分からない。%s" % warning_note)
                self.alert("error", "%s: Cut が宣言されていない(cut: undeclared)。exit 0 でも結論の範囲が不明" % origin)
            elif (cut == "unresolved" and isinstance(model, dict) and "name" in model
                  and model["name"] is None and isinstance(model.get("sha256"), str)):
                msg = "; ".join("%s: %s" % (d.get("code"), d.get("message"))
                                for d in diags if d.get("severity") == "error")
                self.unv(origin, "Cut", v if v in UNVERIFIED else "unresolved",
                         "モデルの parse に失敗した(model.name は null、入力hashあり)。Cut の有無は判定できていない(cut: unresolved)。%s。%s" % (warning_note, msg))
                self.alert("error", "%s: モデルの parse に失敗した(model.name は null、入力hashあり)。%s" % (origin, short(msg, 160)))
            elif isinstance(cut, str):
                self.unv(origin, "Cut", cut, "Cut を解決できていない。モデルが通っていない可能性(%s)。%s" % (self._reason_of(data), warning_note))
            elif isinstance(cut, dict):
                for k in ("exclude", "defer"):
                    for e in as_list(cut.get(k)):
                        if not get(e, "loses"):
                            self.alert("warn", "%s: Cut の %s %s に loses(失うもの)が無い" % (origin, k, short(get(e, "target"), 60)))
            else:
                self.unv(origin, "Cut", "unsupported", "cut の欄が無い・読めない")
            if cut_warning:
                self.alert("error", "%s: W-CUT-0: Cut が宣言されていないという診断がある(cut: %s)" % (origin, short(cut, 80)))
            # 上限
            b = data.get("bounds")
            scen = get(data, "ledger", "execution", "selection", "effective", "scenario", "value")
            self.bounds_rows.append((origin, b, scen))
            if get(b, "truncated") is True:
                self.unv(origin, "bounds", "inconclusive", "計算上の上限で切断された(truncated)。切断に関わる性質は判定不能")
                self.alert("error", "%s: 計算上の上限で切断された(truncated)" % origin)
            # 性質
            for p in as_list(data.get("properties")):
                if not isinstance(p, dict):
                    self.unv(origin, "性質(形が違う)", "unsupported", short(p, 80))
                    continue
                pv, pnote = canon_verdict(p.get("verdict"))
                pid = str(p.get("id"))
                if pv in UNVERIFIED or pnote:
                    reason = pnote or ", ".join(map(str, as_list(p.get("notes")))) or VERDICT_JA[pv]
                    self.unv(origin, pid, pv, reason)
                case_vs = []
                for c in as_list(p.get("cases")):
                    cv, cnote = canon_verdict(get(c, "verdict"))
                    case_vs.append((cv, get(c, "case_id")))
                    if (cv in UNVERIFIED or cnote) and not (pv in UNVERIFIED or pnote):
                        self.unv(origin, "%s case %s" % (pid, get(c, "case_id")), cv, cnote or "案件ごとの verdict")
                if case_vs:
                    worst, wid = max(case_vs, key=lambda x: RANK[x[0]])
                    if RANK[worst] > RANK[pv]:
                        self.alert("error", "%s: 性質 %s は %s だが case %s が %s。全体と案件ごとの判定が矛盾している"
                                   % (origin, pid, pv, wid, worst))
                        self.unv(origin, "%s の全体判定" % pid, worst,
                                 "性質は %s だが cases[] に %s(case %s)がある" % (pv, worst, wid))
                if "mechanism_unmodeled" in as_list(p.get("notes")):
                    self.mechanism.append((origin, pid))
        # mismatch
        if "contract_scope" in data:
            if kind == "mutate" and isinstance(data.get("original"), dict):
                if data["contract_scope"] != data["original"].get("contract_scope", []):
                    self.unv(origin, "contract_scope", "unsupported", "最上位と original の適用範囲が一致しない")
                    self.alert("error", "%s: contract_scope が original と違う" % origin)
            else:
                self._scan_contract_scopes(origin, data["contract_scope"])
        if data.get("mismatch") is not None:
            self.unv(origin, "mismatch", "mismatch", "比較条件が合わない: %s" % short(data.get("mismatch"), 200))
            self.alert("error", "%s: 比較条件が合わない(mismatch が null でない)" % origin)
        # mutate の生成状況
        if kind == "mutate":
            for m in as_list(data.get("mutations")):
                if not isinstance(m, dict):
                    continue
                status = m.get("status")
                if status != "generated":
                    self.unv(origin, m.get("name"), status,
                             "この種の変異は作られていない(%s)。この種で検査の効きは確かめていない" % (m.get("reason") or "理由なし"))
                for mt in as_list(m.get("mutants")):
                    res = get(mt, "result")
                    if res not in ("detected", "undetected"):
                        self.unv(origin, get(mt, "id"), res, "検出にも未検出にも数えない(%s)" % (get(mt, "reason") or "理由なし"))
        # 診断・自己申告の未検査・導出されなかった観点
        for d in diags:
            self.relca_diags.append((origin, d, src))
        for path in UNCHECKED_PATHS:
            for item in as_list(get(data, *path)):
                self.self_unchecked.setdefault(str(item), []).append("%s %s" % (origin, "/".join(path)))
        self._walk_viewpoints(origin, get(data, "agency", "viewpoints"), "agency/viewpoints")
        if "repetition" in data:
            rep = data["repetition"]
            self.repetitions.append((origin, rep, src))
            if isinstance(rep, dict) and rep.get("coverage") == "not_derived":
                self.unv(origin, "repetition.coverage", "not_derived",
                         "; ".join(map(str, as_list(rep.get("unchecked")))) or "繰り返しの回数表を導出できていない")
        # 読まなかった鍵
        read = SCAN_KEYS | SHOWN_KEYS.get(kind, set())
        for k in data:
            if k not in read:
                self.unread.setdefault(str(k), []).append(origin)

    def _scan_contract_scopes(self, origin, rows):
        keys = {"id", "contract", "applies_to", "reason", "status", "masks_call"}
        if not isinstance(rows, list):
            self.unv(origin, "contract_scope", "unsupported", "適用範囲の表が list でない")
            self.alert("error", "%s: contract_scope を読めない" % origin)
            return
        seen = set()
        for row in rows:
            valid = (isinstance(row, dict) and set(row) == keys
                     and all(isinstance(row[k], str) for k in ("id", "contract", "reason"))
                     and row["id"] and row["contract"]
                     and row["status"] == "not_applicable" and type(row["masks_call"]) is bool
                     and isinstance(row["applies_to"], list) and row["applies_to"]
                     and all(isinstance(n, str) and n for n in row["applies_to"])
                     and len(set(row["applies_to"])) == len(row["applies_to"])
                     and row["id"] not in seen)
            if not valid:
                self.unv(origin, "contract_scope", "unsupported", "適用範囲の行が不正: %s" % short(row, 160))
                self.alert("error", "%s: contract_scope の行を読めない" % origin)
                continue
            seen.add(row["id"])
            self.contract_scopes.append((origin, row))
            self.unv(origin, row["id"], "not_applicable",
                     "構成の適用範囲外で未検査。適用先 %s、理由: %s、masks_call: %s"
                     % (", ".join(row["applies_to"]), row["reason"], row["masks_call"]))

    def _compare_fold(self, origin, reported, folded, how):
        if RANK[folded] > RANK[reported]:
            self.alert("error", "%s: 全体の verdict %s が%s %s より軽い。全体の verdict を信用しない" % (origin, reported, how, folded))
            self.unv(origin, "全体の判定", folded, "全体は %s だが%sは %s" % (reported, how, folded))
            for r in self.runs:
                if r.role == origin and r.verdict == "satisfied":
                    r.verdict = folded
        elif RANK[folded] < RANK[reported]:
            self.alert("warn", "%s: 全体の verdict %s が%s %s より重い(後段の判定で重くなった可能性。理由は確かめていない)"
                       % (origin, reported, how, folded))

    def _walk_viewpoints(self, origin, node, path):
        if isinstance(node, dict):
            if node.get("status") == "not_derived":
                self.not_derived.append((origin, path, node.get("reason")))
            if node.get("basis") == "unknown":
                label = node.get("capability") or node.get("actor") or node.get("call") or ""
                self.basis_unknown.append((origin, "%s %s" % (path, label)))
            for k, v in node.items():
                self._walk_viewpoints(origin, v, "%s/%s" % (path, k))
        elif isinstance(node, list):
            for i, v in enumerate(node):
                self._walk_viewpoints(origin, v, "%s/%d" % (path, i))

    def _reason_of(self, data):
        msgs = [get(d, "message") for d in as_list(get(data, "diagnostics"))
                if get(d, "severity") == "error" or str(get(d, "code", default="")).startswith(("E-", "U-"))]
        msgs = [str(m) for m in msgs if m]
        run = get(data, "run")
        if isinstance(run, dict) and (run.get("reason") or run.get("completion")):
            msgs.append("run: completion %s、reason %s" % (run.get("completion"), run.get("reason")))
        status = get(data, "simulation", "status")
        if status and status != "complete":
            msgs.append("simulation status %s" % status)
        return "; ".join(msgs) if msgs else "理由は JSON の diagnostics と stderr を参照"

    @staticmethod
    def _props_of(data):
        return [p for p in as_list(get(data, "properties")) if isinstance(p, dict)]

    # ---- 共通の表
    @staticmethod
    def table(headers, rows, cls=""):
        out = ['<div class="tw"><table class="%s"><thead><tr>' % cls]
        out += ["<th>%s</th>" % esc(h) for h in headers]
        out.append("</tr></thead><tbody>")
        for row in rows:
            out.append("<tr>" + "".join("<td>%s</td>" % c for c in row) + "</tr>")
        if not rows:
            out.append('<tr><td colspan="%d" class="muted">(なし)</td></tr>' % len(headers))
        out.append("</tbody></table></div>")
        return "".join(out)

    def guarded(self, title, fn):
        try:
            return fn()
        except Exception as e:  # noqa: BLE001  節の失敗は見える形で残す
            self.unv("review_page", title, "unsupported", "この節を生成できなかった: %s" % e)
            self.alert("error", "節「%s」を生成できなかった(%s)。この節の内容は確かめられていない" % (title, e))
            return ('<div class="alert error"><b>この節を生成できなかった。</b>%s<pre>%s</pre></div>'
                    % (esc(e), esc(traceback.format_exc(limit=3))))

    def bounds_table(self):
        rows = []
        for origin, b, scen in self.bounds_rows:
            if isinstance(b, dict):
                tr = b.get("truncated")
                rows.append([esc(origin), esc(b.get("cases")), esc(b.get("live_tasks")), esc(b.get("clock")),
                             '<span class="badge bad">切断</span>' if tr is True else esc("切断なし" if tr is False else "未記録"),
                             esc(scen or "—")])
            else:
                rows.append([esc(origin), "—", "—", "—", '<span class="badge warn">bounds 欄なし</span>', esc(scen or "—")])
        unit = get(self.main.data if self.main else None, "unit") or "?"
        return (self.table(["出所", "cases", "live_tasks", "clock(%s)" % unit, "truncated", "scenario"], rows)
                + '<p class="note">結論はこの上限の中だけのもの。業務上の上限ではなく計算の上限を含む。</p>')

    # ---- 1. モデル
    def actors_json(self, report=None):
        data = report if report is not None else (self.main.data if self.main else None)
        return [a for a in as_list(get(data, "actors")) if isinstance(a, dict)]

    def executors_of(self, actor, report=None, src=None):
        src = src or self.src
        for a in self.actors_json(report):
            if a.get("name") == actor:
                ex = get(a, "executors", "value")
                st = get(a, "executors", "status")
                tag = "(%s)" % STATUS_JA.get(st, st) if st else ""
                if isinstance(ex, dict):
                    if ex.get("kind") == "named":
                        return ", ".join(map(str, as_list(ex.get("ids")))) + tag
                    if ex.get("kind") == "anonymous":
                        return "匿名×%s%s" % (ex.get("count"), tag)
                    return short(ex, 60) + tag
                return "なし" + tag
        if src:
            for name, toks in src.items_of("actor"):
                if name == actor:
                    info = parse_actor(src, toks)
                    ex = info["fields"].get("executors")
                    return (ex[0] + "(source)") if ex else "未宣言(source)"
        return "不明"

    def selected_process(self, run, src):
        name = get(run.data if run else None, "ledger", "execution", "selection", "effective", "process", "value")
        procs = src.items_of("process") if src else []
        if name:
            for n, toks in procs:
                if n == name:
                    return name, toks, "JSON の ledger.execution.selection.effective.process"
            return name, None, "JSON が選んだ process が source に無い"
        if len(procs) == 1:
            return procs[0][0], procs[0][1], "source に process が一つだけ"
        return None, None, "選ばれた process を JSON から読めず、source に複数ある"

    def flow_html(self, src, toks, title, report=None):
        stmts = parse_process(src, toks)
        # 意味の検査は Relca に任せる。拒否された loop を正常な流れとして描かない。
        data = report if report is not None else (self.main.data if self.main else None)
        errors = [d for d in as_list(get(data, "diagnostics")) if get(d, "severity") == "error"]
        loop_errors = [d for d in errors if any(w in str(get(d, "message")) for w in
                      ("loop", "iteration", "expected on_limit", "expected at_most"))]
        merge_errors = [d for d in errors if "merge_" in str(get(d, "message"))]

        def mark_rejected(block):
            for st in block:
                span = st.get("span", {})
                local_errors = [d for d in errors if isinstance(get(d, "span", "start"), int)
                                and span.get("start", -1) <= get(d, "span", "start") < span.get("end", -1)] if span else []
                if loop_errors and (st["kind"] == "repeat" or (st["kind"] == "unknown" and
                        any(t.kind == "word" and t.text == "repeat" for t in lex(st["text"])))):
                    st["kind"] = "rejected_repeat"
                    st.setdefault("name", "?")
                    st["reason"] = "; ".join(str(get(d, "message")) for d in loop_errors)
                elif (merge_errors or local_errors) and st["kind"] == "merge":
                    st["kind"] = "rejected_merge"
                    st["reason"] = "; ".join(str(get(d, "message")) for d in (local_errors or merge_errors))
                elif merge_errors and st["kind"] == "rejected_merge":
                    st["reason"] = "; ".join(str(get(d, "message")) for d in merge_errors)
                elif merge_errors and st["kind"] == "yield" and "binding" not in st:
                    st.update(kind="rejected_merge", binding="?", reason="; ".join(str(get(d, "message")) for d in merge_errors))
                elif st["kind"] in ("if", "merge"):
                    mark_rejected(st["then"])
                    mark_rejected(st["else"])
                elif st["kind"] == "fork":
                    mark_rejected(st["body"])
        mark_rejected(stmts)
        who = lambda actor: self.executors_of(actor, report, src)  # noqa: E731
        steps = flatten_steps(stmts)
        rows = []
        no = 0
        unknown = 0
        for s in steps:
            k = s["kind"]
            cond = "<br>".join(esc(c) for c in s.get("conds", [])) or '<span class="muted">常に</span>'
            if s.get("branch"):
                cond = esc(s["branch"]) + "<br>" + cond
            if k == "call":
                no += 1
                rows.append([str(no), "<b>%s</b>" % esc(s["actor"]), esc(who(s["actor"])),
                             "<code>%s.%s(%s)</code>" % (esc(s["actor"]), esc(s["capability"]), esc(s["args"])),
                             "<code>%s</code>" % esc(s["binding"]), cond, str(s["line"])])
            elif k == "merge":
                rows.append(["", "値を返す分岐", "", "<code>%s = if %s</code>" % (esc(s["binding"]), esc(s["cond"])), esc(s["binding"]), cond, str(s["line"])])
            elif k == "yield":
                rows.append(["", "値の複写(Task なし)", "", "<code>yield %s</code>" % esc(s["source"]), esc(s.get("binding", "?")), cond, str(s["line"])])
            elif k == "if":
                rows.append(["", "分岐", "", "<code>if %s</code>" % esc(s["cond"]), "", cond, str(s["line"])])
            elif k == "end":
                rows.append(["", "終端", "", "<code>end %s</code>" % esc(s["name"]), "", cond, str(s["line"])])
            elif k == "repeat":
                rows.append(["", "繰り返し", "", "<code>%s</code>" % esc(repeat_label(s)), "", cond, str(s["line"])])
            elif k in ("again", "leave"):
                desc = ("次の回へ。最後の回なら on_limit(gap なし)" if k == "again" else "繰り返しの後へ")
                rows.append(["", esc(k), "", "<code>%s</code>: %s" % (esc(k), esc(desc)), "", cond, str(s["line"])])
            elif k in ("rejected_repeat", "rejected_merge"):
                self.unv(title, ("repeat %s" % s["name"]) if k == "rejected_repeat" else "merge %s" % s["binding"], "unsupported", s["reason"])
                rows.append(["", '<span class="badge warn">Relca が受け付けない形</span>', "",
                             "<code>%s</code><br>%s" % (esc(s["text"]), esc(s["reason"])), "", cond, str(s["line"])])
            elif k in ("fork", "join", "wait"):
                rows.append(["", esc(k), "", "<code>%s</code>" % esc(s.get("text") or "fork " + s.get("name", "")),
                             "", cond, str(s["line"])])
            else:
                unknown += 1
                rows.append(["", '<span class="badge warn">未解釈</span>', "", "<code>%s</code>" % esc(s["text"]),
                             "", cond, str(s["line"])])
        out = ["<h4>%s</h4>" % esc(title)]
        if unknown:
            self.alert("warn", "%s: process の %d 文をこのページは解釈できなかった。表と図ではそのまま source の文字列を出す" % (title, unknown))
        out.append(self.table(["順", "担当 Actor", "実行者", "操作", "結果の名前", "実行される条件", "行"], rows))
        if not self.args.no_mermaid:
            out.append('<details open><summary>流れ図(Mermaid。表示されないときは上の表が同じ内容)</summary>'
                       '<pre class="mermaid">%s</pre></details>'
                       % esc(mermaid_flow(stmts, who)))
        return "".join(out)

    def section_model(self):
        out = ['<section id="model"><h2>1. モデルが業務に合っているか</h2>',
               '<p class="lead">ここが業務の実態と合っていなければ、2 の判定には意味がない。'
               '担当者・順序・承認・外した範囲・既定で埋めた値・仮に置いた確率と時間を、業務を知る人が確かめてほしい。</p>']
        main = self.main
        cut = get(main.data if main else None, "cut")
        q = get(cut, "question") if isinstance(cut, dict) else None
        name = get(main.data if main else None, "model", "name")
        out.append(self.table(["項目", "値"], [
            ["モデル", esc(name or "(不明)")],
            ["問い(Cut の question)", esc(q) if q else '<span class="badge warn">宣言なし</span>'],
            ["モデルの path", "<code>%s</code>" % esc(self.model_path)],
        ]))
        out.append("<h3>1.0 上限(この範囲の中だけの結論)</h3>")
        out.append(self.guarded("上限", self.bounds_table))
        out.append("<h3>1.1 誰が・何を(Actor と操作)</h3>")
        out.append(self.guarded("Actor", self._actors_html))
        out.append("<h3>1.2 どの順で(業務の流れ)</h3>")
        out.append(self.guarded("流れ", self._flow_main_html))
        out.append("<h3>1.3 権限・責務と U の条件・経路</h3>")
        out.append(self.guarded("承認", self._approval_html))
        out.append("<h3>1.4 Cut で外した範囲</h3>")
        out.append(self.guarded("Cut", self._cut_html))
        out.append("<h3>1.5 確率・方策・既定値の仮定</h3>")
        out.append(self.guarded("仮定", self._assumptions_html))
        out.append("<h3>1.6 relca の診断(全入力、DSL の位置つき)</h3>")
        out.append(self.guarded("診断", self._diagnostics_html))
        out.append("</section>")
        return "".join(out)

    def _actors_html(self):
        actors = self.actors_json()
        actors_from_json = isinstance(get(self.main.data if self.main else None, "actors"), list)
        src_actors = {}
        if self.src:
            for n, toks in self.src.items_of("actor"):
                src_actors[n] = parse_actor(self.src, toks)
        rows = []

        def cell(a, key):
            v = get(a, key, "value")
            st = get(a, key, "status")
            if isinstance(v, dict) and v.get("kind") == "named":
                text = ", ".join(map(str, as_list(v.get("ids"))))
            elif isinstance(v, dict) and v.get("kind") == "anonymous":
                text = "匿名×%s" % v.get("count")
            elif isinstance(v, list):
                text = ", ".join(map(str, v))
            elif v is None:
                text = "—"
            else:
                text = str(v)
            badge = ' <span class="badge %s">%s</span>' % ("warn" if st == "default" else "decl",
                                                           esc(STATUS_JA.get(st, st))) if st else ""
            return esc(text) + badge

        if actors_from_json:
            for a in actors:
                rows.append(["<b>%s</b>" % esc(a.get("name")), cell(a, "executors"), cell(a, "capacity"),
                             cell(a, "control"), cell(a, "source"), cell(a, "capability")])
            note = "出所: JSON の actors。「既定」は relca が省略時既定で埋めた欄(宣言していない)。Actor の名前や種別は意味の根拠にしていない。"
        else:
            for n, info in src_actors.items():
                f = info["fields"]
                rows.append(["<b>%s</b>" % esc(n), esc("; ".join(f.get("executors", [])) or "—"),
                             esc("; ".join(f.get("capacity", [])) or "—"), esc("; ".join(f.get("control", [])) or "—"),
                             esc("; ".join(f.get("source", [])) or "—"),
                             esc(", ".join(c["name"] for c in info["capabilities"]) or "—")])
            note = "JSON に actors が無いので source から読んだ(relca の解釈ではない)。既定値は分からない。"
            self.alert("warn", "Actor 表を JSON から作れず source から読んだ。既定で埋めた欄は表示できていない")
        out = [self.table(["Actor", "実行者", "容量", "制御", "原本(source)", "Capability"], rows),
               '<p class="note">%s</p>' % esc(note)]
        caprows = []
        for n, info in src_actors.items():
            for c in info["capabilities"]:
                cl = c["clauses"]

                def j(*keys):
                    vals = []
                    for k in keys:
                        vals += cl.get(k, [])
                    return "<br>".join("<code>%s</code>" % esc(v) for v in vals) or '<span class="muted">—</span>'
                known = {"reads", "duration", "outcomes", "result", "writes", "requires", "commits",
                         "kernel_reads", "finish", "cancel", "atomic", "uses", "start"}
                other = [v for k, vs in cl.items() if k not in known for v in vs]
                dur = j("duration")
                if cl.get("duration"):
                    dur += "<br>" + self.ai_status_of("%s.%s.duration" % (n, c["name"]))
                caprows.append(["<code>%s.%s</code><br><code>%s</code>" % (esc(n), esc(c["name"]), esc(c["signature"])),
                                j("reads"), dur, j("outcomes", "result", "atomic"), j("writes", "finish", "commits"),
                                j("requires") if cl.get("requires") else '<span class="muted">—</span>',
                                "<br>".join("<code>%s</code>" % esc(v) for v in other) or '<span class="muted">—</span>',
                                str(c["line"])])
        if caprows:
            out.append("<h4>操作(Capability)の中身</h4>")
            out.append(self.table(["操作", "読むもの", "所要時間と出所", "結果の出方", "書くもの・確定", "前提", "その他", "行"], caprows))
            out.append('<p class="note">出所: モデルの source(このページの粗い読み)。所要時間の「出所」は --assumptions の申告で、'
                       '申告が無い値は実測か仮置きか分からない。</p>')
        return "".join(out)

    def _flow_main_html(self):
        if not self.src:
            return '<p class="muted">source を読めないので流れを出せない。</p>'
        name, toks, how = self.selected_process(self.main, self.src)
        if toks is None:
            self.alert("warn", "業務の流れを出せない: %s" % how)
            return '<div class="alert warn">流れを出せない: %s</div>' % esc(how)
        return ('<p class="note">process <code>%s</code>(選んだ根拠: %s)。表と図は source を読んで作った。</p>'
                % (esc(name), esc(how)) + self.flow_html(self.src, toks, "process %s" % name))

    def _approval_html(self):
        out = []
        rows = []
        if self.src:
            for kind in ("people", "authority", "permit", "responsible"):
                for name, toks in self.src.items_of(kind):
                    rows.append([esc(kind), "<code>%s</code>" % esc(self.src.item_text(toks)), str(self.src.line_of_toks(toks))])
        out.append("<h4>宣言(source)</h4>")
        out.append(self.table(["種類", "宣言", "行"], rows))
        data = self.main.data if self.main else None
        rrows = []
        for r in as_list(get(data, "responsibilities")):
            if not isinstance(r, dict):
                continue
            obs = get(r, "observation", "status")
            miss = as_list(get(r, "observation", "missing_sources"))
            ref = get(r, "refusal", "status")
            rrows.append([esc(get(r, "person")), "<code>%s</code>" % esc(get(r, "target", "value")),
                          esc({"present": "原本に届く", "missing": "原本に届かない"}.get(obs, obs))
                          + (" (" + esc(", ".join(map(str, miss))) + ")" if miss else ""),
                          esc({"present": "止められる", "missing": "止められない"}.get(ref, ref)),
                          esc(", ".join(map(str, as_list(get(r, "warnings")))))])
        out.append("<h4>責務(JSON の responsibilities)</h4>")
        out.append(self.table(["人", "対象", "原本への到達", "拒否が効果に届くか", "警告"], rrows))
        frows = []
        for f in as_list(get(data, "agency", "refusal")):
            if not isinstance(f, dict):
                continue
            js = []
            for jd in as_list(get(f, "judgements")):
                stops = get(jd, "stops")
                js.append("%s (%s): %s、迂回 %s" % (get(jd, "call"), get(jd, "capability"),
                                                {"all_paths": "全経路で止める", "conditional": "条件付きで止める",
                                                 "none": "止めない"}.get(stops, stops), get(jd, "bypasses")))
            frows.append(["<code>%s</code>" % esc(get(f, "effect")), esc(get(f, "call")),
                          "<br>".join(esc(x) for x in js) or '<span class="badge warn">止める判定なし</span>'])
        out.append("<h4>確定(commit)の前にある判定(JSON の agency.refusal)</h4>")
        out.append(self.table(["確定する操作", "呼出し", "止める判定"], frows))
        out.append('<p class="note">構造から導いたもので、現実に承認者が止めているかの測定ではない。</p>')
        return "".join(out)

    def _cut_html(self):
        data = self.main.data if self.main else None
        cut = get(data, "cut")
        if not isinstance(cut, dict):
            return ('<div class="alert error">Cut: %s。何を外したかを確かめられていない(理由は冒頭の一覧)。</div>'
                    % esc(short(cut, 80)))
        out = [self.table(["項目", "値"], [
            ["名前", esc(cut.get("name"))],
            ["問い", esc(cut.get("question"))],
            ["含める(include)", esc(", ".join(map(str, as_list(cut.get("include")))))],
            ["外部性(externality)", esc(", ".join(map(str, as_list(cut.get("externality"))))) or "—"],
            ["再入経路(reentry)", esc(", ".join(map(str, as_list(cut.get("reentry"))))) or "—"],
        ])]
        rows = []
        for kind in ("exclude", "defer"):
            for e in as_list(cut.get(kind)):
                loses = get(e, "loses")
                rows.append([esc({"exclude": "外す", "defer": "保留"}[kind]), "<b>%s</b>" % esc(get(e, "target")),
                             esc(get(e, "reason")),
                             esc(loses) if loses else '<span class="badge warn">loses が無い</span>'])
        out.append("<h4>外した・保留した範囲</h4>")
        out.append(self.table(["扱い", "対象", "理由", "失うもの(モデルが見なくなること)"], rows))
        hz = as_list(get(data, "cut_context", "horizon"))
        notes = as_list(get(data, "cut_context", "notices"))
        if hz:
            out.append("<p>時間地平: %s</p>" % esc(short(hz, 400)))
        if notes:
            out.append("<ul>" + "".join("<li>%s</li>" % esc(n) for n in notes) + "</ul>")
        if cut.get("unchecked"):
            out.append('<p class="note">Cut について relca が未検査: %s</p>' % esc(", ".join(map(str, as_list(cut.get("unchecked"))))))
        return "".join(out)

    def _assumptions_html(self):
        data = self.main.data if self.main else None
        out = []
        # AI が置いた値
        out.append("<h4>AI が置いた値(--assumptions)</h4>")
        if self.args.assumptions:
            rows = []
            for a in self.ai_assumptions:
                rows.append(["<code>%s</code>" % esc(a["target"]), esc(short(a.get("value"), 80)),
                             ai_label(a), esc(a.get("basis") or "")])
            out.append(self.table(["対象", "値", "出所(AI の申告)", "根拠・説明"], rows))
            out.append('<p class="note">このファイルは AI が書いた申告で、確認済み・実測もそのままでは確かめられていない。'
                       'confirmed は confirmed_by、measured は source が無ければ仮置きとして扱う。人が確認者や出典に当たって確かめること。</p>')
        else:
            out.append('<div class="alert warn">渡されていない。モデルの所要時間・確率が業務の実測か AI の仮置きかを、このページは区別できない。</div>')
        out.append("<h4>モデルの数値・選択の宣言と出所(%d 件)</h4>" % len(self.declarations()))
        out.append(self.table(["target", "種類", "宣言", "出所", "行"], [
            ["<code>%s</code>" % esc(t), esc(k), "<code>%s</code>" % esc(short(x, 160)), self.ai_status_of(t), esc(l)]
            for t, k, x, l in self.declarations()]))
        out.append('<p class="note">「出所の申告なし」は、業務の実測か AI の仮置きか分からない値。'
                   'bounds と simulation は計算の設定なので上限の表に出す。</p>')
        if self.response_decls():
            out.append("<h4>再送と応答喪失の宣言(source と JSON の対応)</h4>")
            out.append(self.guarded("再送の宣言", self._response_decls_html))
        # 確率の表
        krows = []
        if self.src:
            for name, toks in self.src.items_of("kernel"):
                i = first_index(toks, "{")
                head = self.src.slice(toks[:i]) if i >= 0 else self.src.slice(toks)
                body = block_body(toks, i)[0] if i >= 0 else []
                rows = [self.src.slice(p) for p in split_top(body)]
                krows.append(["<code>%s</code>" % esc(head), "<br>".join("<code>%s</code>" % esc(r) for r in rows),
                              self.ai_status_of("kernel.%s" % name), str(self.src.line_of_toks(toks))])
        out.append("<h4>確率の表(kernel)</h4>")
        out.append(self.table(["kernel と条件", "行(確率: 結果)", "出所", "行"], krows))
        # 観測と潜在
        lrows = []
        if self.src:
            for kind in ("observed", "latent"):
                for _, toks in self.src.items_of(kind):
                    lrows.append([esc({"observed": "観測", "latent": "潜在(真値)"}[kind]),
                                  "<code>%s</code>" % esc(self.src.item_text(toks)), str(self.src.line_of_toks(toks))])
            for n, toks in self.src.items_of("actor"):
                for v in parse_actor(self.src, toks)["fields"].get("latent", []):
                    lrows.append(["潜在(Actor %s)" % esc(n), "<code>%s</code>" % esc(v), str(self.src.line_of_toks(toks))])
        out.append("<h4>観測と潜在の宣言</h4>")
        out.append(self.table(["種類", "宣言", "行"], lrows))
        out.append('<p class="note">潜在は業務の判断や方策から読めない(読むと E-POL-LATENT)。初期分布 ~ { … } も確率の仮定である。</p>')
        # 検査の宣言と方策・流入
        vrows = []
        if self.src:
            for kind in ("verify", "policy", "arrivals", "scenario", "simulation", "metrics", "bounds"):
                for _, toks in self.src.items_of(kind):
                    vrows.append([esc(kind), "<code>%s</code>" % esc(self.src.item_text(toks)), str(self.src.line_of_toks(toks))])
            for name, toks in self.src.items_of("contract"):
                for text, line in parse_contract(self.src, toks):
                    vrows.append(["contract %s" % esc(name), "<code>%s</code>" % esc(text), str(line)])
        out.append("<h4>検査・方策・流入の宣言(verify / contract / policy 等)</h4>")
        out.append(self.table(["種類", "宣言", "行"], vrows))
        scope_headers = []
        for origin, src in (("model", self.src), ("compare-model", self.compare_src)):
            if not src or (origin == "compare-model" and self.compare_sha == self.model_sha):
                continue
            for name, toks in src.items_of("contract"):
                opening = next((i for i, t in enumerate(toks) if t.text == "{"), len(toks))
                header = toks[:opening]
                if len(header) > 2 and header[2].text == "applies_to":
                    scope_headers.append([esc(origin), esc(name), "<code>%s</code>" % esc(src.slice(header)), str(src.line_of_toks(toks))])
        if scope_headers or self.contract_scopes:
            out.append("<h4>契約の適用範囲</h4>")
            out.append(self.table(["source", "契約", "適用先と理由の宣言", "行"], scope_headers))
            scope_rows = [[esc(origin), "<code>%s</code>" % esc(row["id"]), esc(", ".join(row["applies_to"])), esc(row["reason"]),
                           "not_applicable / verdict なし", esc(row["masks_call"])] for origin, row in self.contract_scopes]
            out.append(self.table(["結果の出所", "未検査の契約", "適用先", "理由", "扱い", "検査を隠す可能性(masks_call)"], scope_rows))
            out.append('<p class="note">範囲外は satisfied ではなく未検査。未確認の件数に数えるが、全体の判定には数えない。masks_call は手順や異議・復旧等の経路に呼出しがある場合と、Capability を参照しない節を外す場合に true。実際の到達性の判定ではない。</p>')
        # 既定値
        drows = []
        for a in self.actors_json():
            for key, v in a.items():
                if isinstance(v, dict) and v.get("status") == "default" and v.get("value") is not None:
                    drows.append(["<b>%s</b>.%s" % (esc(a.get("name")), esc(key)), esc(short(v.get("value"), 80))])
        out.append("<h4>Actor の欄で既定値が入ったもの</h4>")
        out.append(self.table(["欄", "既定で入った値"], drows))
        rates = get(data, "metrics", "rates")
        if rates is not None:
            rrows = []
            for r in as_list(get(rates, "actors")) + as_list(get(rates, "capabilities")):
                if isinstance(r, dict) and r.get("reason") == "not_configured":
                    rrows.append([esc(r.get("actor") or r.get("capability")), "必要" if r.get("required") else "不要",
                                  "未設定(0 円として扱っていない)"])
            out.append("<h4>単価</h4>")
            out.append(self.table(["対象", "費用計算に必要か", "状態"], rrows))
        arows = [["<code>%s</code>" % esc(get(a, "field")), esc(short(get(a, "value"), 80)), esc(get(a, "affects"))]
                 for a in as_list(get(data, "assumptions")) if isinstance(a, dict)]
        out.append("<h4>relca が置いた仮定(JSON の assumptions、%d 件)</h4>" % len(arows))
        out.append(self.table(["欄", "値", "影響"], arows))
        unc = get(data, "uncertainty", "model", "items")
        if isinstance(unc, list) and unc:
            out.append("<h4>モデル不確実性(JSON の uncertainty.model、%d 件)</h4>" % len(unc))
            out.append("<pre>%s</pre>" % esc(json.dumps(unc, ensure_ascii=False, indent=1)[:20000]))
        return "".join(out)

    # ---- 再送と応答喪失(R5)
    def unreadable(self, origin, where, why):
        """新しい欄の型違い・欠落・未知の語。補わずに、読めなかった情報として警告する。"""
        self.unreadable_rows.append((str(origin), str(where), str(why)))
        self.alert("warn", "%s: %s を読めなかった(%s)。この欄は補わず、確かめられていないものとして扱う"
                   % (origin, where, why))

    def typed(self, origin, obj, key, types, what=None):
        """(状態, 値)。状態は absent(欄なし)・null(明示の null)・ok・bad(型違い。警告に出す)。"""
        if not isinstance(obj, dict) or key not in obj:
            return "absent", None
        v = obj[key]
        if v is None:
            return "null", None
        if isinstance(v, types) and not (isinstance(v, bool) and bool not in types):
            return "ok", v
        self.unreadable(origin, what or key, "型が %s(値 %s)" % (type(v).__name__, short(v, 60)))
        return "bad", v

    @staticmethod
    def state_cell(state, value, absent="欄なし", null="null"):
        if state == "absent":
            return '<span class="muted">%s</span>' % esc(absent)
        if state == "null":
            return esc(null)
        if state == "bad":
            return '<span class="badge warn">読めない: %s</span>' % esc(short(value, 60))
        return esc(value)

    def receipt_cell(self, origin, obj, key="observed_receipt"):
        st, v = self.typed(origin, obj, key, (str,))
        if st == "ok" and v not in RECEIPT_JA:
            self.unreadable(origin, key, "未知の語 %s" % short(v, 40))
            return "bad", v, '<span class="badge warn">未知の語 %s(読めない)</span>' % esc(short(v, 40))
        if st == "ok":
            return st, v, esc(RECEIPT_JA[v])
        return st, v, self.state_cell(st, v, absent="—(この段に応答は無い)")

    def _response_decls_html(self):
        data = self.main.data if self.main else None
        unit = get(data, "unit") or ""
        json_resp = {}
        for a in as_list(get(data, "assumptions")):
            f = get(a, "field")
            if isinstance(f, str) and f.startswith("response."):
                json_resp[f[len("response."):]] = get(a, "value")
        caps = {}
        for n, toks in self.src.items_of("actor"):
            for c in parse_actor(self.src, toks)["capabilities"]:
                caps["%s.%s" % (n, c["name"])] = c
        rows = []
        used = set()
        for rd in self.response_decls():
            origin = "source %d 行の %s" % (rd["line"], rd["binding"])
            rl, rt = rd["response_loss"], rd["retry"]
            if rd["rest"]:
                self.unreadable(origin, "call の後ろ", "このページが読めない部分 %s" % short(rd["rest"], 80))
            cap = caps.get("%s.%s" % (rd["actor"], rd["capability"]))
            mech = []
            for k in ("idempotency", "effect_scope"):
                for v in (cap["clauses"].get(k, []) if cap else []):
                    mech.append("<code>%s</code><br>%s" % (esc(v), self.ai_status_of("%s.%s.%s" % (
                        rd["actor"], rd["capability"], k))))
            # JSON の response.<binding> と照合
            jv = json_resp.get(rd["binding"], ABSENT)
            used.add(rd["binding"])
            check = []
            if data is None:
                jcell = '<span class="muted">JSON なし</span>'
            elif jv is ABSENT:
                jcell = '<span class="badge warn">JSON の assumptions に response.%s が無い</span>' % esc(rd["binding"])
                if rl or rt:
                    self.unreadable("JSON", "assumptions の response.%s" % rd["binding"], "source に宣言があるのに欄が無い")
            else:
                kv = parse_kv(jv)
                jcell = "<code>response.%s = %s</code>" % (esc(rd["binding"]), esc(short(jv, 120)))
                if kv is None:
                    self.unreadable("JSON", "response.%s の値" % rd["binding"], "形が違う %s" % short(jv, 60))
                else:
                    pairs = []
                    if rt:
                        pairs.append(("max_attempts", str(rt["max_attempts"]), kv.get("max_attempts")))
                        if unit and rt["backoff"] == "%s%s" % (kv.get("backoff"), unit):
                            pairs.append(("backoff", kv.get("backoff"), kv.get("backoff")))
                        else:
                            pairs.append(("backoff", rt["backoff"], (kv.get("backoff") or "") + unit))
                    if rl:
                        pairs.append(("loss", rl["mode"], (kv.get("loss") or "").split("(")[0].split("{")[0].strip().lower()))
                    for k, s, j in pairs:
                        if s == j:
                            check.append("%s 一致" % k)
                        else:
                            check.append("%s 不一致(source %s / JSON %s)" % (k, s, j))
                            self.alert("error", "source %d 行の %s: %s が source(%s)と JSON(%s)で食い違う。"
                                       "別のモデルの JSON か、このページの読み違い" % (rd["line"], rd["binding"], k, s, j))
            loss = "—"
            if rl:
                loss = "<code>%s</code>" % esc(rl["text"])
                if rl["p"] is not None:
                    loss += "<br>p = %s" % esc(rl["p"])
                if rl["mode"] == "probability":
                    loss += "<br>basis: %s" % (esc(rl["basis"]) if rl["basis"] else '<span class="badge warn">basis なし</span>')
                if rl["sequence"] is not None:
                    loss += "<br>順に: %s" % esc(", ".join(rl["sequence"]))
                loss += "<br>" + self.ai_status_of(rd["target_loss"])
            retry = ['<span class="muted">retry の宣言なし</span>']
            if rt:
                retry = ["<code>%s</code>" % esc(rd["target_retry"]),
                         "試行の上限 %d 回(初回を含む。再送は最大 %d 回)" % (rt["max_attempts"], max(rt["max_attempts"] - 1, 0)),
                         "再送する条件: %s" % esc(rt["when"]), "間隔: %s" % esc(rt["backoff"]),
                         "尽きたときの行先: <code>end %s</code>" % esc(rt["exhausted"]),
                         self.ai_status_of(rd["target_retry"])]
            rows.append(["<code>%s</code><br>process %s" % (esc(rd["binding"]), esc(rd["process"])),
                         "<code>%s.%s</code>" % (esc(rd["actor"]), esc(rd["capability"])),
                         "<br>".join(retry), loss,
                         "<br>".join(mech) or '<span class="muted">idempotency・effect_scope の宣言なし</span>',
                         jcell + ("<br>" + esc("、".join(check)) if check else ""), str(rd["line"])])
        for b, v in json_resp.items():
            if b not in used:
                self.unreadable("JSON", "assumptions の response.%s" % b, "source の call に対応する宣言が無い")
        out = [self.table(["結果の名前", "操作", "再送(retry)", "応答喪失(response_loss)",
                           "冪等キー・効果の範囲(Capability の宣言)", "JSON の仮定と照合", "行"], rows)]
        out.append('<p class="note">max_attempts は初回を含む試行の回数。idempotency key と effect_scope modeled は'
                   'モデルに置いた仮定で、現実の決済サービスがそう振る舞うことを relca もこのページも確かめていない'
                   '(名前や宣言から機構の保証は作らない)。値の出所は target(retry.&lt;結果の名前&gt; など)で --assumptions に申告する。</p>')
        return "".join(out)

    def _response_steps_html(self, cx):
        steps = [s for s in as_list(cx.get("steps")) if isinstance(s, dict) and "command" in s]
        if not steps:
            return ""
        cases = {s.get("case_id") for s in steps}
        rows = []
        for s in steps:
            idx = s.get("index")
            origin = "反例 %s の step %s" % (short(cx.get("id"), 60), idx)
            c_st, cmd = self.typed(origin, s, "command", (str,))
            a_st, att = self.typed(origin, s, "attempt", (int,))
            if a_st == "ok" and att < 1:
                self.unreadable(origin, "attempt", "1 より小さい %s" % att)
                a_st = "bad"
            if a_st in ("absent", "null"):
                self.unreadable(origin, "attempt", "command があるのに試行番号が無い。1 を補わない")
            r_st, rcv, rcell = self.receipt_cell(origin, s)
            o_st, obs = self.typed(origin, s, "result", (str,))
            x_st, act = self.typed(origin, s, "actual_result", (str,))
            kind = s.get("kind")
            if kind == "finish" and r_st == "absent":
                self.unreadable(origin, "observed_receipt", "finish の段なのに応答の状態が無い")
            # 呼出し元が観測した結果。unknown なら未受信(実際の結果をここへ移さない)。
            if r_st == "ok" and rcv == "unknown":
                if o_st == "ok":
                    self.alert("error", "%s: 応答が unknown なのに呼出し元の観測結果 %s がある。"
                               "観測と実際の結果を取り違えた入力の可能性" % (origin, short(obs, 40)))
                    self.unv(origin, "応答と観測結果", "unsupported", "応答 unknown と観測結果 %s が両立しない" % short(obs, 40))
                    ocell = '<span class="badge bad">矛盾: %s</span>' % esc(short(obs, 40))
                else:
                    ocell = "未受信(%s。業務へは未公開)" % ("null" if o_st == "null" else "欄なし")
            else:
                ocell = self.state_cell(o_st, obs, null="null(結果はまだ無い)" if r_st == "absent" else "null")
                if r_st == "ok" and rcv == "delivered" and o_st == "ok" and x_st == "ok" and obs != act:
                    self.alert("error", "%s: 応答が delivered なのに、観測結果 %s とサーバ側の実際の結果 %s が違う"
                               % (origin, short(obs, 40), short(act, 40)))
                if r_st == "ok" and rcv == "delivered" and o_st != "ok":
                    self.unreadable(origin, "result", "応答が delivered なのに観測結果が無い")
            xcell = self.state_cell(x_st, act, absent="—(欄なし)", null="null(明示)")
            ev = s.get("events", ABSENT)
            if isinstance(ev, list) and all(isinstance(e, dict) for e in ev):
                kinds = [str(e.get("kind")) for e in ev]
                commit = "commit" in kinds
                ecell = ("commit あり" if commit else "commit なし") + '<br><span class="muted">events: %s</span>' % esc(
                    ", ".join(kinds) or "なし")
                c_known = True
            else:
                self.unreadable(origin, "events", "欄が無い" if ev is ABSENT else "list でない(%s)" % type(ev).__name__)
                ecell = '<span class="badge warn">読めない</span>'
                commit, c_known = None, False
            rl = s.get("retry_location", ABSENT)
            if rl is ABSENT or rl is None:
                lcell = '<span class="muted">%s</span>' % ("—(retry の宣言なし)" if rl is ABSENT else "null")
            elif isinstance(rl, dict):
                where, text = self.loc(rl.get("span"))
                if where == "—" and isinstance(rl.get("line"), int):
                    where = "%s:%s" % (rl.get("line"), rl.get("column"))
                lcell = esc(where) + ("<br><code>%s</code>" % esc(short(text, 80)) if text else "")
            else:
                self.unreadable(origin, "retry_location", "object でない(%s)" % type(rl).__name__)
                lcell = '<span class="badge warn">読めない</span>'
            # 読み方
            who = cmd if c_st == "ok" else "(command 読めない)"
            nth = "%d 回目" % att if a_st == "ok" else "(試行番号の記録なし)"
            head = ("案件 %s の " % s.get("case_id") if len(cases) > 1 else "") + "%s の %s" % (who, nth)
            if kind == "finish":
                parts = ["実際は %s" % act if x_st == "ok" else "実際の結果は記録なし"]
                if c_known:
                    parts[0] += " で commit 済み" if commit else "(commit なし)"
                else:
                    parts[0] += "(commit は読めない)"
                if r_st == "ok" and rcv in RECEIPT_JA:
                    parts.append("応答は %s" % rcv)
                    parts.append("業務への結果は未公開" if rcv == "unknown" else "業務への結果は %s" % (obs if o_st == "ok" else "記録なし"))
                else:
                    parts.append("応答の状態は読めない")
                sentence = "%s: %s" % (head, "、".join(parts))
            else:
                sentence = "%s: %s" % (head, STEP_KIND_JA.get(kind, str(kind)))
            self.resp_steps.append({"counterexample": cx.get("id"), "index": idx, "case_id": s.get("case_id"),
                                    "command": cmd, "attempt": att if a_st == "ok" else None,
                                    "receipt": rcv, "commit": commit, "sentence": sentence})
            rows.append([esc(idx), esc(s.get("time")), esc(s.get("case_id")), self.state_cell(c_st, cmd),
                         self.state_cell(a_st, att, absent="記録なし", null="記録なし(null)"), esc(kind),
                         rcell, ocell, xcell, ecell, lcell, esc(sentence)])
        return ('<h5>再送と応答の段(command のある step)</h5>'
                + self.table(["#", "時刻", "案件", "元の command", "試行(初回=1)", "事象", "呼出し元から見た応答",
                              "呼出し元が観測した結果", "サーバ側の実際の結果(分析者用。呼出し元には見えない)",
                              "commit(step.events)", "retry を宣言した位置", "読み方"], rows)
                + '<p class="note">サーバ側の実際の結果は分析者のための欄で、方策や業務判断の入力ではない(ADR 0056)。'
                  '応答が unknown の段では、呼出し元は結果を受け取っていない(未受信)。commit は step.events から読み、'
                  '新しい段を足していない。試行番号が無い段に 1 を補わない。この反例は一つの例で、原因の全部の説明や発生率ではない。</p>')

    def _response_html(self, data):
        keys = ("response_receipts", "case_outcomes", "response_analysis")
        if not any(k in data for k in keys):
            return ""
        out = ["<h4>応答喪失と再送</h4>"]
        out.append(self.guarded("case_outcomes", lambda: self._case_outcomes_html(data)))
        out.append(self.guarded("response_receipts", lambda: self._receipts_html(data)))
        out.append(self.guarded("response_analysis", lambda: self._response_analysis_html(data)))
        return "".join(out)

    def _range_cell(self, origin, t, key, null):
        st, v = self.typed(origin, t, key, (dict,))
        if st == "ok":
            lo, hi = v.get("min", ABSENT), v.get("max", ABSENT)
            if lo is ABSENT or hi is ABSENT:
                self.unreadable(origin, key, "min・max が無い")
                return st, '<span class="badge warn">読めない</span>'
            return st, esc("%s 〜 %s" % (lo, hi))
        return st, self.state_cell(st, v, absent="—", null=null)

    def _case_outcomes_html(self, data):
        co = data.get("case_outcomes", ABSENT)
        out = ["<h5>案件ごとの終わり方(case_outcomes)</h5>"]
        if co is ABSENT:
            return out[0] + '<p class="muted">case_outcomes の欄が無い。</p>'
        if not isinstance(co, list):
            self.unreadable("case_outcomes", "case_outcomes", "list でない(%s)" % type(co).__name__)
            return out[0] + '<div class="alert warn">case_outcomes を読めない。</div>'
        rr = {}
        rr_prop = [p for p in self._props_of(data) if p.get("id") == "receipt_resolution"]
        for p in rr_prop:
            for c in as_list(p.get("cases")):
                rr[get(c, "case_id")] = canon_verdict(get(c, "verdict"))[0]
        trunc = get(data, "bounds", "truncated")
        rows, summary = [], []
        for c in co:
            if not isinstance(c, dict):
                self.unreadable("case_outcomes", "案件", "object でない")
                continue
            cid = c.get("case_id")
            origin = "case_outcomes 案件 %s" % cid
            terms = c.get("terminals", ABSENT)
            if not isinstance(terms, list):
                self.unreadable(origin, "terminals", "欄が無い" if terms is ABSENT else "list でない")
                terms = []
            ends, ru, cut = [], 0, 0
            for t in terms:
                if not isinstance(t, dict):
                    self.unreadable(origin, "終端", "object でない")
                    continue
                k_st, kind = self.typed(origin, t, "kind", (str,))
                if k_st == "ok" and kind not in CASE_TERMINAL_JA:
                    self.unreadable(origin, "kind", "未知の語 %s" % short(kind, 40))
                    k_st = "bad"
                if k_st != "ok":
                    if k_st in ("absent", "null"):
                        self.unreadable(origin, "kind", "終端の種類が無い")
                    kcell, reading = '<span class="badge warn">読めない: %s</span>' % esc(short(kind, 40)), "読めない"
                else:
                    kcell, reading = "<code>%s</code>" % esc(kind), CASE_TERMINAL_JA[kind]
                null_done = "null(未完了。0 ではない)" if kind == "receipt_unknown" else "null"
                cells = {}
                for key in ("completed_at", "elapsed", "work", "observed_until", "observed_elapsed", "observed_work"):
                    cells[key] = self._range_cell(origin, t, key, null_done if key in ("completed_at", "elapsed", "work") else "null")
                if kind == "receipt_unknown":
                    ru += 1
                    for key in ("completed_at", "elapsed", "work"):
                        if cells[key][0] == "ok":
                            self.unreadable(origin, key, "応答不明で止まった終端なのに完了量がある")
                    if cells["observed_until"][0] != "ok":
                        self.unreadable(origin, "observed_until", "応答不明で止まった時点の観測が無い")
                elif kind == "cut_off":
                    cut += 1
                elif kind == "end":
                    e_st, e = self.typed(origin, t, "end", (str,))
                    ends.append(e if e_st == "ok" else "?")
                prob = t.get("probability")
                rows.append([esc(cid), kcell, esc(t.get("end")) if t.get("end") is not None else '<span class="muted">null(End ではない)</span>',
                             "<br>".join("%s: %s" % (k, cells[k][1]) for k in ("completed_at", "elapsed", "work")),
                             "<br>".join("%s: %s" % (k, cells[k][1]) for k in ("observed_until", "observed_elapsed", "observed_work")),
                             esc("%s 〜 %s" % (prob.get("min"), prob.get("max"))) if isinstance(prob, dict) else "—",
                             esc(reading)])
            res = rr.get(cid)
            if ru and res is None:
                self.unreadable(origin, "receipt_resolution", "応答不明の終端があるのに、この案件の receipt_resolution が無い")
            parts = []
            if not ends:
                parts.append("End に届いた終端は 0 件。これは「失敗した」ではなく「End に届いたものが無い」")
            if ru:
                parts.append("応答不明で止まる終端が %d 種あり、未確認(receipt_resolution %s)" % (ru, res or "記録なし"))
            if cut or trunc is True:
                parts.append("計算上の切断がある(応答不明とは別の理由の未確認)")
            if not ru and not cut and trunc is not True:
                parts.append("応答不明・切断で止まった終端は無い")
            summary.append([esc(cid), esc("%d 件(%s)" % (len(ends), ", ".join(map(str, ends))) if ends else "0 件"),
                            esc("receipt_unknown %d / cut_off %d" % (ru, cut)),
                            verdict_badge(res) if res else '<span class="muted">記録なし</span>',
                            esc({True: "切断", False: "切断なし"}.get(trunc, "未記録")), esc("。".join(parts))])
            self.case_view.append({"case_id": cid, "ends": ends, "receipt_unknown": ru, "cut_off": cut,
                                   "receipt_resolution": res, "truncated": trunc})
        out.append(self.table(["案件", "業務の End に届いた終端", "未確認で止まった終端", "receipt_resolution",
                               "bounds.truncated", "読み方"], summary))
        out.append(self.table(["案件", "終端の種類", "End", "完了(completed_at・elapsed・work)",
                               "止まるまでの観測(observed_*)", "確率", "読み方"], rows))
        out.append('<p class="note">receipt_unknown は、応答が届いたか分からないまま案件が止まった終端で、業務の End ではない。'
                   '完了の時刻・経過・作業が null なのは 0 ではなく、完了していないため。observed_* は止まった時点までの観測で、完了時間ではない。'
                   '計算上の切断(bounds.truncated・cut_off)とは別の理由の未確認で、どちらも成功・失敗に数えない。'
                   'End に届いた終端が 0 件であることと、未確認の終端があることは別の列に分けた。</p>')
        return "".join(out)

    def _receipts_html(self, data):
        rs = data.get("response_receipts", ABSENT)
        out = ["<h5>応答の観測の集合(response_receipts)</h5>"]
        if rs is ABSENT:
            return out[0] + '<p class="muted">response_receipts の欄が無い。</p>'
        if not isinstance(rs, list):
            self.unreadable("response_receipts", "response_receipts", "list でない(%s)" % type(rs).__name__)
            return out[0] + '<div class="alert warn">response_receipts を読めない。</div>'
        rows = []
        for k, r in enumerate(rs):
            origin = "response_receipts[%d]" % k
            if not isinstance(r, dict):
                self.unreadable(origin, "行", "object でない")
                continue
            c = {}
            for key, ty in (("case_id", (int,)), ("command", (str,)), ("attempt", (int,)), ("capability", (str,)),
                            ("observed_result", (str,)), ("resolved_effect", (bool,))):
                st, v = self.typed(origin, r, key, ty)
                if st == "absent":
                    self.unreadable(origin, key, "欄が無い")
                c[key] = (st, v)
            _, rcv, rcell = self.receipt_cell(origin, r, "receipt")
            if rcv == "unknown" and c["observed_result"][0] == "ok":
                self.alert("error", "%s: 応答が unknown なのに observed_result %s がある" % (origin, short(c["observed_result"][1], 40)))
            at = r.get("at")
            sort_key = tuple(str(c[k][1]) for k in ("case_id", "command", "attempt")) + (str(rcv),)
            rows.append((sort_key, [self.state_cell(*c["case_id"]), self.state_cell(*c["command"]),
                                    self.state_cell(*c["attempt"]), self.state_cell(*c["capability"]), rcell,
                                    esc("%s 〜 %s" % (at.get("min"), at.get("max"))) if isinstance(at, dict) else esc(short(at, 40)),
                                    self.state_cell(c["observed_result"][0], c["observed_result"][1],
                                                    null="null(未受信)" if rcv == "unknown" else "null"),
                                    self.state_cell(*c["resolved_effect"])]))
        rows.sort(key=lambda x: x[0])
        out.append('<p class="note">上限内で到達できる全経路から集めた観測の集合。行の並びは実行の順ではなく(案件・command・試行・応答で並べ替えた)、'
                   '上の反例の経路の代わりにもならない。一つの実行で全部の行が起きるわけではない。'
                   'resolved_effect はその行の時点での意味で、同じ試行の unknown の行が後で delivered になったことを表さない。</p>')
        out.append(self.table(["案件", "command", "試行", "Capability", "応答", "時刻(at)", "観測した結果", "resolved_effect"],
                              [r for _, r in rows]))
        return "".join(out)

    def _response_analysis_html(self, data):
        ra = data.get("response_analysis", ABSENT)
        out = ["<h5>応答の解析の範囲(response_analysis)</h5>"]
        if ra is ABSENT:
            out.append('<p class="muted">response_analysis の欄が無い。</p>')
        elif not isinstance(ra, dict):
            self.unreadable("response_analysis", "response_analysis", "object でない")
            out.append('<div class="alert warn">response_analysis を読めない。</div>')
        else:
            out.append(self.table(["項目", "値"], [
                [esc(k), esc(", ".join(map(str, ra[k])) if isinstance(ra.get(k), list) else ra.get(k))]
                for k in ("coverage", "business_horizon", "scope", "checked", "unchecked") if k in ra]))
            out.append('<p class="note">coverage は上限内の探索の網羅で、業務が終わったことではない。unchecked の項目は冒頭の未検査にも出す。</p>')
        rows = self.response_control_rows(data)
        if rows:
            out.append("<h5>Agency と応答の制御(response_control)</h5>")
            out.append('<div class="alert warn">%s</div>' % esc(RESPONSE_CONTROL_NOTE))
            out.append(self.table(["JSON の欄", "値", "影響(relca の説明)"], rows))
        return "".join(out)

    @staticmethod
    def response_control_rows(data):
        rows = []
        for a in as_list(get(data, "assumptions")):
            f = get(a, "field")
            if isinstance(f, str) and (f.startswith("agency.diagnostics.") or f == "agency.response_control"
                                       or "response_control" in str(get(a, "value"))):
                rows.append(["<code>%s</code>" % esc(f), esc(short(get(a, "value"), 80)), esc(get(a, "affects"))])
        if not rows and "response_control" in as_list(get(data, "agency", "unchecked")):
            rows.append(["<code>agency.unchecked</code>", "response_control", "コード別の未導出の内訳は JSON に無い"])
        return rows

    def loc(self, span, src=None):
        src = src or self.src
        if not isinstance(span, dict) or src is None:
            return "—", ""
        lc = src.line_col(span.get("start"))
        if not lc:
            return "—", ""
        return "%d:%d" % lc, src.line_text(lc[0])

    def _diagnostics_html(self):
        rows = []
        for origin, d, src in self.relca_diags:
            where, text = self.loc(d.get("span"), src) if src is not None else ("—", "")
            sev = d.get("severity")
            rows.append([esc(origin),
                         '<span class="badge %s">%s</span>' % ("bad" if sev == "error" else "warn", esc(sev)),
                         "<code>%s</code>" % esc(d.get("code")), esc(d.get("message")), esc(where),
                         "<code>%s</code>" % esc(short(text, 120)) if text else ""])
        return self.table(["出所", "重さ", "コード", "内容", "行:列", "source"], rows)

    # ---- 2. 結論
    def clause_text(self, prop_id, src=None):
        src = src or self.src
        if not src or not isinstance(prop_id, str):
            return None
        parts = prop_id.split()
        if len(parts) >= 3 and parts[0] == "contract" and parts[2].startswith("#"):
            try:
                k = int(parts[2][1:])
            except ValueError:
                return None
            for name, toks in src.items_of("contract"):
                if name == parts[1]:
                    clauses = parse_contract(src, toks)
                    if 1 <= k <= len(clauses):
                        return "%s(%d 行)" % clauses[k - 1]
        if len(parts) == 3 and parts[0] == "probability" and parts[2].startswith("#"):
            try:
                k = int(parts[2][1:])
            except ValueError:
                return None
            found = [(src.item_text(t), src.line_of_toks(t)) for n, t in src.items_of("verify")
                     if len(t) > 2 and t[1].text == "probability" and t[2].text == parts[1]]
            if 1 <= k <= len(found):
                return "%s(%d 行)" % found[k - 1]
        for n, t in src.items_of("verify"):
            if src.item_text(t) == "verify " + prop_id:
                return "%s(%d 行)" % (src.item_text(t), src.line_of_toks(t))
        return None

    def section_conclusion(self):
        out = ['<section id="conclusion"><h2>2. 結論</h2>',
               '<p class="lead">判定はモデルについての、上限の中の結論で、現実の業務品質の証明ではない。'
               '1 の照合が済んでいない結論は使わないこと。</p>']
        out.append("<h3>2.1 確かめられていないもの</h3>")
        out.append('<p>冒頭の「確かめられていないもの・警告」に全入力・全 report の分を集めた(<!--UNVERIFIED-COUNT--> 件)。各結論の表でも先頭に置く。</p>')
        for r in self.runs:
            if r.role in ("check", "explain", "simulate"):
                out.append(self.guarded("結論 %s" % r.role, lambda r=r: self._run_conclusion(r)))
            elif r.role == "compare":
                out.append(self.guarded("compare", lambda r=r: self._compare_html(r)))
        if self.repetitions:
            out.append(self.guarded("繰り返しの出力", self._repetition_html))
        out.append("</section>")
        return "".join(out)

    def _repetition_html(self):
        out = ["<h3>繰り返しの出力(repetition、全 report)</h3>",
               '<p class="note">回数は初回を含む body の開始回数。on_limit は最後の again の後だけで、計算切断ではない。'
               'null は0ではなく算出なし。確率未宣言を一様分布にしない。sampled は完了・非切断標本の観測で、網羅的な確率ではない。'
               'site は元位置の対応表であり、すべての回が実行されたことを意味しない。</p>']
        for origin, rep, src in self.repetitions:
            out.append("<h4>%s</h4>" % esc(origin))

            def bad(path, why):
                self.unreadable(origin, "repetition." + path, why)
                self.unv(origin, "repetition." + path, "unsupported", why)
                return '<span class="badge warn">読めない: %s</span>' % esc(why)

            def scalar(obj, key, types, path, nullable=False):
                if key not in obj:
                    return bad(path + key, "欄なし")
                value = obj[key]
                if value is None and nullable:
                    return "null(算出なし)"
                if not isinstance(value, types) or isinstance(value, bool):
                    return bad(path + key, "型が違う: %s" % short(value, 60))
                if isinstance(value, int) and value < 0:
                    return bad(path + key, "負の件数・回数")
                return esc(value)

            def interval(obj, key, types, path):
                if key in obj and obj[key] is None:
                    return "null(算出なし)"
                value = obj.get(key)
                if not isinstance(value, dict):
                    return bad(path + key, "範囲の object が無い・型が違う")
                return "%s 〜 %s" % (scalar(value, "min", types, path + key + "."),
                                    scalar(value, "max", types, path + key + "."))

            def records(obj, key, path):
                value = obj.get(key)
                if not isinstance(value, list):
                    bad(path + key, "list が無い・型が違う")
                    return []
                rows = []
                for i, item in enumerate(value):
                    if isinstance(item, dict):
                        rows.append((item, "%s%s[%d]." % (path, key, i)))
                    else:
                        bad("%s%s[%d]" % (path, key, i), "object でない")
                return rows

            if not isinstance(rep, dict):
                out.append(bad("", "object でない"))
                continue
            coverage = rep.get("coverage")
            out.append(self.table(["項目", "値"], [[k, scalar(rep, k, (str,), "")]
                                                   for k in ("rules", "coverage", "basis")]))
            unchecked = rep.get("unchecked")
            if not isinstance(unchecked, list) or any(not isinstance(v, str) for v in unchecked):
                out.append(bad("unchecked", "文字列の list でない"))
            else:
                out.append('<p>unchecked: %s</p>' % esc("; ".join(unchecked) or "(なし)"))
            if coverage not in ("complete", "sampled", "not_derived"):
                out.append(bad("coverage", "未知の coverage %s" % short(coverage, 60)))
            cases = records(rep, "cases", "")
            if coverage not in ("complete", "sampled"):
                out.append('<p class="alert warn">回数表は算出できていない。0回・0件ではない。'
                           'この出力から回数や on_limit の到達を確定しない。</p>')
            else:
                rows, distributions = [], []
                for case, path in cases:
                    cid = scalar(case, "case_id", (int,), path)
                    loop = scalar(case, "loop_name", (str,), path)
                    rows.append([cid, loop, interval(case, "rounds", (int,), path),
                                 interval(case, "on_limit", (int,), path),
                                 interval(case, "on_limit_probability", (str,), path)]
                                + [scalar(case, k, (int,), path, nullable=True)
                                   for k in ("samples", "excluded_samples", "observed_on_limit")])
                    for freq, fpath in records(case, "distribution", path):
                        distributions.append([cid, loop, scalar(freq, "rounds", (int,), fpath),
                                              interval(freq, "probability", (str,), fpath),
                                              scalar(freq, "observed_count", (int,), fpath, nullable=True)])
                out.append("<h5>繰り返しの回数と上限後の処理</h5>")
                out.append(self.table(["case_id", "loop", "body 開始回数", "on_limit 件数", "on_limit 確率",
                                       "samples", "excluded_samples", "observed_on_limit"], rows))
                out.append("<h5>body 開始回数の分布</h5>")
                out.append(self.table(["case_id", "loop", "rounds", "probability", "observed_count"], distributions))
            sites = []
            for site, path in records(rep, "sites", ""):
                orig = site.get("origin")
                if not isinstance(orig, dict):
                    out.append(bad(path + "origin", "object が無い・型が違う"))
                    continue
                phase = orig.get("phase")
                if phase not in ("body", "on_limit"):
                    bad(path + "origin.phase", "未知の phase %s" % short(phase, 60))
                rnd = orig.get("round", ABSENT)
                if phase == "on_limit" and rnd is None:
                    round_cell = "null"
                elif phase == "body" and type(rnd) is int and rnd >= 1:
                    round_cell = esc(rnd)
                else:
                    round_cell = bad(path + "origin.round", "body は1始まり、on_limit は null が必要")
                span = site.get("source_span")
                if not isinstance(span, dict) or any(type(span.get(k)) is not int or span[k] < 0 for k in ("start", "end")):
                    where = bad(path + "source_span", "バイト位置の範囲が無い・型が違う")
                else:
                    where = esc(self.loc(span, src)[0]) if src is not None else "—(source なし)"
                sites.append([scalar(site, "instance", (str,), path),
                              scalar(orig, "loop_name", (str,), path + "origin."),
                              scalar(orig, "call_site", (str,), path + "origin."), round_cell,
                              scalar(orig, "phase", (str,), path + "origin."), where])
            out.append("<h5>繰り返しの site と元位置</h5>")
            out.append(self.table(["instance", "loop", "call_site", "round", "phase", "元の行:列"], sites))
        return "".join(out)

    def _run_conclusion(self, r):
        data = r.data
        out = ['<h3>2.%s %s の結論 %s</h3>' % ({"check": 2, "explain": 2, "simulate": 3}.get(r.role, 4), esc(r.role),
                                              verdict_badge(r.verdict, r.raw_verdict))]
        if r.verdict_note:
            out.append('<p class="alert warn">%s</p>' % esc(r.verdict_note))
        if r.role == "simulate":
            sim = as_dict(get(data, "simulation"))
            out.append('<p class="note">simulate は標本推定で、全スケジュールの安全性検証ではない。'
                       '未解決の非決定に出会うと拒否する(unsupported)。</p>')
            out.append(self.table(["項目", "値"], [
                [esc(k), esc(short(sim.get(k), 120))] for k in
                ("status", "replications", "attempted_replications", "completed_replications", "seed",
                 "confidence", "nondeterminism") if k in sim]))
        if data is None:
            return "".join(out) + '<div class="alert error">JSON を読めない。</div>'
        props = self._props_of(data)
        expl = as_dict(get(data, "explanation"))
        cxs = {get(c, "id"): c for c in as_list(expl.get("counterexamples")) if isinstance(c, dict)}
        omitted = {get(o, "id"): o for o in as_list(expl.get("omitted")) if isinstance(o, dict)}
        groups = {"unverified": [], "violated": [], "satisfied": []}
        for p in props:
            v, note = canon_verdict(p.get("verdict"))
            g = "violated" if v == "violated" else ("satisfied" if v == "satisfied" else "unverified")
            groups[g].append((p, v, note))
        titles = [("unverified", "確かめられていない性質"), ("violated", "破れる性質"), ("satisfied", "満たす性質")]
        for g, title in titles:
            rows = []
            for p, v, note in groups[g]:
                prob = p.get("probability")
                prob_s = ""
                if isinstance(prob, dict):
                    prob_s = "確率 %s 〜 %s" % (prob.get("min"), prob.get("max"))
                clause = self.clause_text(p.get("id"))
                notes = [str(x) for x in as_list(p.get("notes"))]
                extra = ", ".join(notes)
                if "mechanism_unmodeled" in notes:
                    extra += "(仕組みをモデル化していない。現実の機構を認定しない: ADR 0056)"
                rows.append(["<code>%s</code>" % esc(p.get("id")), verdict_badge(v, p.get("verdict")),
                             "<code>%s</code>" % esc(clause) if clause else '<span class="muted">source 対応なし</span>',
                             esc(" / ".join(x for x in (prob_s, extra, note or "") if x))])
            out.append("<h4>%s(%d)</h4>" % (title, len(rows)))
            out.append(self.table(["性質", "verdict", "source の宣言", "値・注記"], rows,
                                  "unverified" if g == "unverified" else ""))
        for p, v, _ in groups["violated"]:
            pid = p.get("id")
            if pid in cxs:
                out.append(self._counterexample_html(cxs[pid]))
            elif pid in omitted:
                reason = get(omitted[pid], "reason")
                out.append('<p class="note"><code>%s</code> の反例は出ていない: %s</p>'
                           % (esc(pid), esc(OMIT_REASON_JA.get(reason, reason))))
            elif r.role != "explain":
                out.append('<p class="note"><code>%s</code> の反例は <code>relca explain</code> の JSON を渡すと出る。</p>' % esc(pid))
            else:
                out.append('<p class="note"><code>%s</code> の反例は explain の出力に無い。</p>' % esc(pid))
        if expl:
            out.append('<p class="note">explain の注意: %s</p>' % esc(" / ".join(
                str(get(a, "affects")) for a in as_list(expl.get("assumptions")) if isinstance(a, dict))))
        trows = []
        for t in as_list(get(data, "terminals")):
            if not isinstance(t, dict):
                continue
            prob = t.get("probability")
            trows.append(["<code>end %s</code>" % esc(t.get("end")),
                          esc("%s 〜 %s" % (prob.get("min"), prob.get("max"))) if isinstance(prob, dict) else "—",
                          esc("%s 〜 %s" % (get(t, "elapsed", "min"), get(t, "elapsed", "max"))),
                          esc("%s 〜 %s" % (get(t, "work", "min"), get(t, "work", "max")))])
        out.append("<h4>終端ごとの値(単位 %s)</h4>" % esc(get(data, "unit") or "?"))
        out.append(self.table(["終端", "確率", "経過時間", "作業時間"], trows))
        out.append('<p class="note">min〜max は上限内の到達可能な全経路の範囲。確率の min/max は完全情報の緩和境界で、実現できる方策の値ではない。</p>')
        if r.role in ("check", "explain"):
            resp = self._response_html(data)
            if resp:
                out.append('<p class="note">この表は End に届いた終端だけ。応答が届いたか分からないまま止まった案件は、下の case_outcomes に出す。</p>')
                out.append(resp)
        return "".join(out)

    def _counterexample_html(self, cx):
        rows = []
        for s in as_list(cx.get("steps")):
            if not isinstance(s, dict):
                continue
            if not (s.get("actor") or s.get("violation") or s.get("kind") == "merged"):
                continue
            where, text = self.loc(get(s, "location", "span"))
            mark = ' <span class="badge bad">違反</span>' if s.get("violation") else ""
            who = "%s.%s" % (s.get("actor"), s.get("capability")) if s.get("actor") else "—"
            res = esc(s.get("result") or "—")
            if s.get("kind") == "merged":
                who = "%s ← %s(Task なし)" % (s.get("binding"), s.get("source"))
            if s.get("observed_receipt") == "unknown" and s.get("result") is None:
                res = "未受信(応答 unknown)"
            rows.append([esc(s.get("index")), esc(s.get("time")), esc(s.get("case_id")), esc(s.get("kind")) + mark,
                         "<code>%s</code>" % esc(who), esc(s.get("executor") or "—"), res,
                         esc(where), "<code>%s</code>" % esc(short(text, 100)) if text else ""])
        where, text = self.loc(get(cx, "context_location", "span"))
        head = ('<h4>反例: <code>%s</code>(状態 %s、遷移 %s 本)</h4>'
                % (esc(cx.get("id")), esc(cx.get("status")), esc(cx.get("transition_count"))))
        return (head + '<p class="note">一つの例であり、原因ではない。Actor の関わる段、値の合流、違反の段を出す。'
                '文脈の位置 %s <code>%s</code></p>' % (esc(where), esc(short(text, 100)))
                + self.table(["#", "時刻", "案件", "事象", "操作", "実行者", "結果(呼出し元が観測)", "行:列", "source"], rows)
                + self.guarded("反例の応答", lambda: self._response_steps_html(cx))
                + self.guarded("根拠源の証跡", lambda: self._evidence_html(cx)))

    def _evidence_html(self, cx):
        is_basis = str(cx.get("id", "")).endswith(" evidence_basis")
        if "evidence" not in cx:
            if is_basis and cx.get("status") == "verified":
                raise ValueError("検証済みの根拠契約の反例に evidence が無い")
            return ""
        evidence = cx["evidence"]
        keys = {"binding", "actor", "capability", "required", "sources", "missing", "excluded_edges"}
        if not is_basis or cx.get("status") != "verified" or not isinstance(evidence, dict) or set(evidence) != keys:
            raise ValueError("evidence の種類・状態・鍵が対応範囲外")
        for key in ("binding", "actor", "capability"):
            if not isinstance(evidence[key], str) or not evidence[key].strip():
                raise ValueError("evidence.%s は空でない文字列が必要" % key)
        steps = cx.get("steps")
        if (not isinstance(steps, list) or not steps or not isinstance(steps[-1], dict)
                or steps[-1].get("kind") != "finish"
                or any(steps[-1].get(key) != evidence[key] for key in ("binding", "actor", "capability"))):
            raise ValueError("evidence の呼出しが反例の最後の Finish と一致しない")
        for key in ("required", "sources", "missing", "excluded_edges"):
            values = evidence[key]
            if (not isinstance(values, list) or any(not isinstance(v, str) or not v for v in values)
                    or values != sorted(set(values))):
                raise ValueError("evidence.%s は重複のない昇順の文字列配列が必要" % key)
        if (not evidence["required"] or not evidence["missing"]
                or set(evidence["missing"]) != set(evidence["required"]) - set(evidence["sources"])):
            raise ValueError("evidence の要求・数えた源・不足した源が整合しない")
        rows = [["呼出し", "<code>%s = %s.%s</code>" % (
            esc(evidence["binding"]), esc(evidence["actor"]), esc(evidence["capability"]))]]
        for key, title in (("required", "要求した源"), ("sources", "照合に数えた源"),
                           ("missing", "不足した源"), ("excluded_edges", "数えなかった観測変数の辺")):
            rows.append([title, "<code>%s</code>" % esc(", ".join(evidence[key]) or "(なし)")])
        return ("<h5>モデルの根拠源の証跡</h5>" + self.table(["項目", "値"], rows)
                + '<p class="note">reads の宣言とデータの系譜の照合であり、実際の資料アクセスを観測したログではない。'
                '数えなかった辺を、読んだことにして補完しない。</p>')

    def _compare_html(self, r):
        data = r.data
        out = ['<h3>2.4 compare の結論 %s</h3>' % verdict_badge(r.verdict, r.raw_verdict)]
        out.append('<p class="note">analysis: %s。compare は案を並べるだけで、順位・総合点は出さない。</p>'
                   % esc(get(data, "analysis")))
        if get(data, "mismatch") is not None:
            out.append('<div class="alert error">比較条件が合わない: <code>%s</code></div>' % esc(short(get(data, "mismatch"), 400)))
        arows = [["<code>%s</code>" % esc(get(a, "field")), esc(short(get(a, "value"), 80)), esc(get(a, "affects"))]
                 for a in as_list(get(data, "assumptions")) if isinstance(a, dict)]
        out.append(self.table(["compare の仮定", "値", "影響"], arows))
        rows = []
        plans = [p for p in as_list(get(data, "plans")) if isinstance(p, dict)]
        for pl in plans:
            rep = as_dict(pl.get("report"))
            v, note = canon_verdict(rep.get("verdict"))
            proc = get(rep, "ledger", "execution", "selection", "effective", "process", "value")
            term = []
            for t in as_list(rep.get("terminals")):
                if isinstance(t, dict):
                    term.append("end %s: %s〜%s" % (t.get("end"), get(t, "elapsed", "min"), get(t, "elapsed", "max")))
            b = rep.get("bounds")
            rows.append(["<b>%s</b>" % esc(pl.get("configuration")), "<code>%s</code>" % esc(proc),
                         verdict_badge(v, rep.get("verdict")), esc("; ".join(term)), esc(short(b, 100))])
        out.append(self.table(["案", "process", "verdict", "終端ごとの経過時間", "bounds"], rows))
        prow = []
        for p in as_list(get(data, "tables", "properties")):
            if not isinstance(p, dict):
                continue
            cells = []
            for val in as_list(p.get("values")):
                if get(val, "status") == "not_applicable":
                    if get(val, "verdict") is not None or get(val, "reason") != "configuration_scope":
                        raise ValueError("比較の非適用セルに verdict がある、または理由が違う")
                    cells.append('<span class="badge warn">非適用(not_applicable、verdict なし)</span>')
                    continue
                v, _ = canon_verdict(get(val, "verdict"))
                prob = get(val, "probability")
                cells.append(verdict_badge(v, get(val, "verdict")) +
                             (" %s" % esc(short(prob, 60)) if prob else ""))
            prow.append(["<code>%s</code>" % esc(p.get("id"))] + cells)
        heads = ["性質"] + [str(pl.get("configuration")) for pl in plans]
        out.append("<h4>性質を案ごとに(tables.properties)</h4>")
        out.append(self.table(heads, [row + [""] * (len(heads) - len(row)) for row in prow]))
        out.append('<p class="note">確かめられていないものの収集は tables ではなく各案の report を走査して行う。</p>')
        if self.compare_src:
            out.append("<h4>案ごとの流れ(--compare-model の source)</h4>")
            for pl in plans:
                proc = get(pl, "report", "ledger", "execution", "selection", "effective", "process", "value")
                found = [t for n, t in self.compare_src.items_of("process") if n == proc]
                if found:
                    out.append(self.flow_html(self.compare_src, found[0], "案 %s: process %s" % (pl.get("configuration"), proc),
                                              as_dict(pl.get("report"))))
                else:
                    out.append('<p class="muted">案 %s の process %s は source に無い。</p>' % (esc(pl.get("configuration")), esc(proc)))
        return "".join(out)

    # ---- 3. 検査の効き
    def section_mutation(self):
        out = ['<section id="mutation"><h2>3. 検査の効き(mutate)</h2>']
        r = self.run("mutate")
        if r is None:
            out.append('<div class="alert warn" id="mutation-missing"><b>未実施。</b>mutate の結果が渡されていないので、'
                       '検査がわざと壊したモデルを捕まえるかは確かめていない。</div></section>')
            return "".join(out)
        out.append(self.guarded("mutate", lambda: self._mutation_html(r)))
        out.append("</section>")
        return "".join(out)

    def _mutation_html(self, r):
        data = r.data
        out = ["<p>mutate 全体: %s / set: %s / 元のモデルの verdict: %s / run: <code>%s</code></p>"
               % (verdict_badge(r.verdict, r.raw_verdict), esc(get(data, "set")), esc(get(data, "original_verdict")),
                  esc(short(get(data, "run"), 200)))]
        out.append('<p class="note">検出・未検出は弱点の候補で、検出率・網羅率・モデルの強さではない。'
                   '未完了の変異体は検出にも未検出にも数えない。変異体ごとの report は走査していない(壊したモデルの判定であり、元のモデルについての結論ではないため)。</p>')
        rows = []
        details = []
        for m in as_list(get(data, "mutations")):
            if not isinstance(m, dict):
                continue
            counts = {}
            for mt in as_list(m.get("mutants")):
                res = str(get(mt, "result"))
                counts[res] = counts.get(res, 0) + 1
                details.append(["<code>%s</code>" % esc(get(mt, "id")), esc(get(mt, "operator")),
                                '<span class="badge %s">%s</span>' % (
                                    {"detected": "ok", "undetected": "bad"}.get(res, "warn"), esc(res)),
                                esc(get(mt, "reason") or "")])
            status = m.get("status")
            badge = "ok" if status == "generated" else "warn"
            rows.append([esc(m.get("name")), "<code>%s</code>" % esc(m.get("kind")),
                         '<span class="badge %s">%s</span>' % (badge, esc(status)), esc(m.get("reason") or ""),
                         esc(", ".join(map(str, as_list(m.get("assigned_checks"))))),
                         str(counts.get("detected", 0)), str(counts.get("undetected", 0)),
                         esc(", ".join("%s %d" % kv for kv in counts.items() if kv[0] not in ("detected", "undetected"))) or "0"])
        out.append(self.table(["種類", "kind", "生成", "理由", "割り当てた検査", "検出", "未検出", "その他"], rows))
        out.append("<details><summary>変異体ごと(%d 件)</summary>%s</details>"
                   % (len(details), self.table(["id", "操作", "結果", "理由"], details)))
        arows = [["<code>%s</code>" % esc(get(a, "field")), esc(short(get(a, "value"), 80)), esc(get(a, "affects"))]
                 for a in as_list(get(data, "assumptions")) if isinstance(a, dict)]
        out.append(self.table(["mutate の仮定", "値", "影響"], arows))
        return "".join(out)

    # ---- 冒頭
    def header(self, now):
        rows = []
        if self.model_sha:
            rows.append(["model", "<code>%s</code>" % esc(self.model_path), "<code>%s</code>" % self.model_sha, "—", "—", "—", "—", "—"])
        if self.args.compare_model and self.compare_sha:
            rows.append(["compare model", "<code>%s</code>" % esc(self.args.compare_model),
                         "<code>%s</code>" % self.compare_sha, "—", "—", "—", "—", "—"])
        if self.args.assumptions and os.path.exists(self.args.assumptions):
            rows.append(["assumptions", "<code>%s</code>" % esc(self.args.assumptions),
                         "<code>%s</code>" % sha256_file(self.args.assumptions), "—", "—", "—", "—", "—"])
        versions = set()
        for r in self.runs:
            versions.add(r.version())
            exp = EXIT_OF.get(r.raw_verdict) if isinstance(r.raw_verdict, str) else None
            rows.append([esc(r.role), "<code>%s</code>" % esc(r.path), "<code>%s</code>" % esc(r.sha256),
                         "<code>%s</code>" % esc(short(r.raw_verdict, 40)), verdict_badge(r.verdict),
                         esc("未記録" if r.exit is None else r.exit),
                         esc("—" if exp is None else exp), esc(r.mode or "—")])
            if r.stderr_path:
                rows.append([esc(r.role + " stderr"), "<code>%s</code>" % esc(r.stderr_path),
                             "<code>%s</code>" % esc(sha256_file(r.stderr_path) if os.path.exists(r.stderr_path) else "—"),
                             "—", "—", "—", "—", "—"])
        title = get(self.main.data if self.main else None, "model", "name") or os.path.basename(self.model_path)
        out = ['<header><h1>Relca 結果の確認: %s</h1>' % esc(title),
               '<p>生成時刻 %s / relca のバージョン %s / 生成 review_page.py</p>'
               % (esc(now), esc("、".join(sorted(versions)) or "不明"))]
        out.append(self.table(["入力", "path", "sha256", "JSON の verdict", "このページの扱い", "exit",
                               "verdict から期待する exit", "mode"], rows))
        out.append("<h3>上限</h3>")
        out.append(self.bounds_table())
        out.append("</header>")
        return "".join(out)

    def top_empty(self):
        warns = [d for _, d, _ in self.relca_diags if d.get("severity") != "error"]
        return not (self.unverified or self.alerts or self.self_unchecked or warns or self.not_run
                    or self.not_derived or self.basis_unknown or self.unread or self.mechanism
                    or self.ai_assumptions or self.unprovenanced)

    def agency_unverified_rows(self):
        rows = [(o, p, "not_derived: %s" % (r or "")) for o, p, r in self.not_derived]
        rows += [(o, p, "basis unknown") for o, p in self.basis_unknown]
        return list(dict.fromkeys(rows))

    def unverified_count(self):
        """One count for the page and terminal; retain the category data separately."""
        return len(dict.fromkeys(self.unverified)) + len(self.agency_unverified_rows())

    def alerts_html(self):
        out = ['<section id="unverified"><h2>確かめられていないもの・警告</h2>']
        if self.top_empty():
            out.append('<p class="ok-line">この入力の範囲で検出されたものは無い。</p>')
        uniq = list(dict.fromkeys(self.unverified))
        count = self.unverified_count()
        if count:
            out.append('<div class="alert error"><b>確かめられていないもの %d 件。</b>'
                       'inconclusive・unsupported・invalid・vacuous・exit 未確認・Cut 未宣言・'
                       '繰り返しの未導出・Agency の未導出・根拠不明は合格ではない。</div>' % count)
        if uniq:
            rows = [[esc(src), "<code>%s</code>" % esc(what), verdict_badge(v) if v in EXIT_OF else
                     '<span class="badge warn">%s</span>' % esc(v), esc(why)] for src, what, v, why in uniq]
            out.append(self.table(["出所", "対象", "扱い", "理由"], rows, "unverified"))
        if self.not_run:
            out.append('<h3>回していない・渡されていないもの</h3><ul id="not-run">%s</ul>'
                       % "".join("<li>%s</li>" % esc(x) for x in self.not_run))
        if self.alerts:
            out.append("<h3>このページの照合による警告 %d 件</h3><ul class=\"alerts\">" % len(dict.fromkeys(self.alerts)))
            for level, text in dict.fromkeys(self.alerts):
                out.append('<li class="%s">%s</li>' % (level, esc(text)))
            out.append("</ul>")
        placeholders = [a for a in self.ai_assumptions if a["status"] == "placeholder"]
        if placeholders:
            out.append('<h3>AI が仮に置いた値 %d 件(実測ではない)</h3><p>%s</p>'
                       % (len(placeholders), esc(", ".join(a["target"] for a in placeholders))))
        claims = [a for a in self.ai_assumptions if a["status"] != "placeholder"]
        if claims:
            out.append('<h3 id="ai-claims">人が確かめる必要のある AI 申告 %d 件</h3>' % len(claims))
            out.append(self.table(["target", "AI の申告", "値"], [
                ["<code>%s</code>" % esc(a["target"]), ai_label(a), esc(short(a.get("value"), 80))] for a in claims]))
        if self.unprovenanced:
            out.append('<h3>出所の申告なし %d 件(実測か仮置きか分からない値)</h3>' % len(self.unprovenanced))
            out.append(self.table(["target", "種類", "行"], [
                ["<code>%s</code>" % esc(t), esc(k), esc(l)] for t, k, x, l in self.unprovenanced]))
        if self.unreadable_rows:
            rows = [[esc(o), esc(w), esc(y)] for o, w, y in dict.fromkeys(self.unreadable_rows)]
            label = "再送・応答・繰り返し" if self.repetitions else "再送・応答"
            out.append('<h3 id="unreadable">%sの欄で読めなかった情報 %d 件</h3>' % (label, len(rows)))
            out.append(self.table(["出所", "欄", "理由"], rows, "unverified"))
        if self.self_unchecked:
            rows = [["<code>%s</code>" % esc(k), esc(", ".join(dict.fromkeys(v)))] for k, v in self.self_unchecked.items()]
            out.append("<h3>relca が自分で未検査と申告した項目 %d 種</h3>" % len(rows))
            out.append(self.table(["項目", "出所"], rows))
            if "response_control" in self.self_unchecked:
                codes = [get(a, "field") for a in as_list(get(self.main.data if self.main else None, "assumptions"))
                         if str(get(a, "field")).startswith("agency.diagnostics.")]
                out.append('<p class="alert warn" id="response-control">response_control: %s%s</p>' % (
                    esc(RESPONSE_CONTROL_NOTE),
                    esc("(%s)" % ", ".join(str(c)[len("agency.diagnostics."):] for c in codes)) if codes else ""))
        warns = {}
        for origin, d, _ in self.relca_diags:
            if d.get("severity") != "error":
                warns.setdefault("%s: %s" % (d.get("code"), d.get("message")), []).append(origin)
        if warns:
            out.append("<h3>relca の警告 %d 件(全入力の diagnostics)</h3>" % len(warns))
            out.append(self.table(["警告", "出所"], [[esc(k), esc(", ".join(dict.fromkeys(v)))] for k, v in warns.items()]))
        if self.mechanism:
            out.append("<h3>仕組みをモデル化していない性質(mechanism_unmodeled)</h3><p>%s。満たしていても現実の機構(二重登録防止など)を認定しない(ADR 0056)。</p>"
                       % esc(", ".join("%s %s" % x for x in dict.fromkeys(self.mechanism))))
        if self.not_derived or self.basis_unknown:
            rows = [[esc(x) for x in row] for row in self.agency_unverified_rows()]
            out.append("<h3>Agency の観点で導出されなかったもの・根拠が不明なもの %d 件</h3>" % len(rows))
            out.append(self.table(["出所", "場所", "状態"], rows))
        if self.unread:
            out.append("<h3>このページが読まなかった最上位の鍵</h3>")
            out.append(self.table(["鍵", "出所"], [["<code>%s</code>" % esc(k), esc(", ".join(dict.fromkeys(v)))]
                                                 for k, v in self.unread.items()]))
        out.append("</section>")
        return "".join(out)

    def render(self, now=None):
        now = now or datetime.datetime.now(datetime.timezone.utc).isoformat(timespec="seconds")
        if self.main is None:
            self.alert("error", "check か explain の JSON が無い。モデルの照合は source だけから作る")
        body = self.section_model() + self.section_conclusion() + self.section_mutation()
        head = self.header(now) + self.alerts_html()
        body = body.replace("<!--UNVERIFIED-COUNT-->", str(self.unverified_count()))
        summary = {
            "unverified": [list(x) for x in dict.fromkeys(self.unverified)],
            "unverified_count": self.unverified_count(),
            "alerts": [list(x) for x in dict.fromkeys(self.alerts)],
            "not_run": self.not_run,
            "self_unchecked": self.self_unchecked,
            "relca_warnings": sorted({"%s: %s" % (d.get("code"), d.get("message"))
                                      for _, d, _ in self.relca_diags if d.get("severity") != "error"}),
            "not_derived": [list(x) for x in self.not_derived],
            "basis_unknown": [list(x) for x in self.basis_unknown],
            "unread_keys": self.unread,
            "mechanism_unmodeled": [list(x) for x in self.mechanism],
            "bounds": [[o, b, s] for o, b, s in self.bounds_rows],
            "ai_assumptions": self.ai_assumptions,
            "ai_claims": [a["target"] for a in self.ai_assumptions if a["status"] != "placeholder"],
            "unprovenanced": [d[0] for d in self.unprovenanced],
            "declarations": [list(d) for d in self.declarations()],
            "runs": [{"role": r.role, "path": r.path, "raw_verdict": r.raw_verdict, "verdict": r.verdict,
                      "exit": r.exit, "sha256": r.sha256} for r in self.runs],
            "model_sha256": self.model_sha,
            "unreadable": [list(x) for x in dict.fromkeys(self.unreadable_rows)],
            "response_steps": self.resp_steps,
            "case_outcomes": self.case_view,
            "response_decls": [{"target_retry": d["target_retry"], "target_loss": d["target_loss"],
                                "retry": d["retry"], "response_loss": d["response_loss"], "rest": d["rest"],
                                "line": d["line"]} for d in self.response_decls()],
            "generated_at": now,
            "empty": self.top_empty(),
        }
        embedded = (json.dumps(summary, ensure_ascii=False, default=str)
                    .replace("&", "\\u0026").replace("<", "\\u003c").replace(">", "\\u003e"))
        mermaid = ("" if self.args.no_mermaid else
                   '<script src="%s" integrity="%s" crossorigin="anonymous"></script>'
                   '<script>try{if(window.mermaid){mermaid.initialize({startOnLoad:true,securityLevel:"strict"});}}catch(e){}</script>'
                   % (MERMAID_URL, MERMAID_SRI))
        return ("<!doctype html><html lang=\"ja\"><head><meta charset=\"utf-8\">"
                "<meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">"
                "<title>Relca 結果の確認</title><style>%s</style></head><body><main>%s%s"
                "<script type=\"application/json\" id=\"review-data\">%s</script></main>%s</body></html>"
                % (CSS, head, body, embedded, mermaid))


CSS = """
:root{--fg:#1d1f23;--bg:#fff;--muted:#666;--line:#d9dce1;--ok:#1b6e3a;--okbg:#e5f4ea;--bad:#a1121b;--badbg:#fde8ea;
--warn:#8a5300;--warnbg:#fff3d6;--decl:#334;--declbg:#eef0f6}
@media (prefers-color-scheme: dark){:root{--fg:#e6e6e6;--bg:#16181c;--muted:#9aa;--line:#3a3f47;--okbg:#173322;--ok:#7fd6a0;
--badbg:#3d1519;--bad:#ff9aa2;--warnbg:#3a2c10;--warn:#ffcf7a;--declbg:#262a33;--decl:#cfd6e6}}
body{margin:0;background:var(--bg);color:var(--fg);font:15px/1.6 system-ui,-apple-system,"Hiragino Sans","Noto Sans JP",sans-serif}
main{max-width:1180px;margin:0 auto;padding:16px}
h1{font-size:1.4em;margin:.2em 0}h2{border-bottom:2px solid var(--line);padding-bottom:.2em;margin-top:2em}
h3{margin-top:1.6em}h4{margin:1.2em 0 .4em}
.tw{overflow-x:auto}table{border-collapse:collapse;width:100%;margin:.4em 0;font-size:.92em}
th,td{border:1px solid var(--line);padding:4px 6px;vertical-align:top;text-align:left}th{background:var(--declbg)}
code,pre{font-family:ui-monospace,SFMono-Regular,Menlo,monospace;font-size:.9em}pre{white-space:pre-wrap;overflow-x:auto}
.badge{display:inline-block;padding:0 6px;border-radius:4px;font-size:.85em;white-space:nowrap}
.badge.ok{background:var(--okbg);color:var(--ok)}.badge.bad{background:var(--badbg);color:var(--bad)}
.badge.warn{background:var(--warnbg);color:var(--warn)}.badge.decl{background:var(--declbg);color:var(--decl)}
.alert{padding:8px 12px;border-radius:6px;margin:.6em 0}.alert.error{background:var(--badbg);color:var(--bad)}
.alert.warn{background:var(--warnbg);color:var(--warn)}
ul.alerts li.error{color:var(--bad)}ul.alerts li.warn{color:var(--warn)}
#unverified{border:2px solid var(--bad);border-radius:8px;padding:4px 14px;margin-top:1em}
table.unverified td{background:var(--warnbg)}
.muted,.note{color:var(--muted)}.note{font-size:.9em}.lead{font-weight:500}.ok-line{color:var(--ok)}
pre.mermaid{background:transparent}
"""


def parse_args(argv):
    ap = argparse.ArgumentParser(description="Relca の結果を人間が確かめるための単一 HTML を作る")
    ap.add_argument("--model", required=True, help="check/explain に渡したモデルの path")
    ap.add_argument("--check", help="relca check か explain の stdout(JSON)")
    ap.add_argument("--simulate", help="relca simulate の stdout(JSON)")
    ap.add_argument("--compare", help="relca compare の stdout(JSON)")
    ap.add_argument("--compare-model", help="compare に渡したモデルの path(案ごとの流れと sha256 照合)")
    ap.add_argument("--mutate", help="relca mutate の stdout(JSON)")
    ap.add_argument("--assumptions", help="AI が置いた値の JSON(list か {\"assumptions\": list})。"
                                          "要素は target・value・basis・status(placeholder|confirmed|measured)")
    for role in ("check", "simulate", "compare", "mutate"):
        ap.add_argument("--%s-exit" % role, type=int, help="%s の終了コード" % role)
        ap.add_argument("--%s-stderr" % role, help="%s の stderr を保存したファイル" % role)
    ap.add_argument("--no-mermaid", action="store_true", help="Mermaid の script を入れない(完全にオフライン)")
    ap.add_argument("-o", "--output", required=True, help="出力する HTML の path")
    args = ap.parse_args(argv)
    if not args.check:
        ap.error("--check(check か explain の JSON)が必要")
    return args


def main(argv=None):
    args = parse_args(sys.argv[1:] if argv is None else argv)
    page = Page(args)
    text = page.render()
    with open(args.output, "w", encoding="utf-8") as f:
        f.write(text)
    n_unv = page.unverified_count()
    n_alert = len(dict.fromkeys(page.alerts))
    print("wrote %s: 確かめられていないもの %d 件、警告 %d 件" % (args.output, n_unv, n_alert))
    return 0


if __name__ == "__main__":
    sys.exit(main())
