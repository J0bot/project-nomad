#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
xerb-tsoin — programmer en XERBOXION, c'est programmer en TSOINS.

Le langage xerboxion (`xerb`) est concaténatif à pile : des IONS (opcodes nommés)
composent des BIONS (`: nom … ;`), la pile = le FLOT. Le `xerb` du labo fait le
round-trip WASM↔xerboxion (bit-exact). Ici, le RUNTIME branché sur la machine à
tsoins : on interprète les ions, et — comme un programme déterministe EST un tsoin —
on le **content-adresse** (addr64) et on le **grave sur le bus** (10.0.0.1:8730).

Donc : on ne programme plus des input→output. On compose des bions, ça donne un
résultat déterministe, et le couple (programme, résultat) = un TSOIN rejouable,
permanent, content-adressé. Le programme du jour devient un instant du réel.

Usage :
  python3 xerb-tsoin.py            # joue les programmes de démo, grave chacun en tsoin
  python3 xerb-tsoin.py 'valeur 2 valeur 3 somme'   # joue un programme inline
"""
import sys
import json
import urllib.request
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

XION = "http://10.0.0.1:8730"


def addr64(s):
    h = 0xcbf29ce484222325
    for ch in s:
        h = ((h ^ (ord(ch) & 0xff)) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def grave(name, payload):
    body = json.dumps({"topic": "tsoin.record",
                       "payload": json.dumps({"name": name, "bytes": payload.encode().hex()})}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=body,
                               headers=bus_headers()), timeout=8)
        return True
    except Exception:
        return False


# --- les IONS : chaque mot agit sur le FLOT (la pile) ---
def _bin(st, f):
    b = st.pop(); a = st.pop(); st.append(int(f(a, b)))


IONS = {
    "somme":   lambda st: _bin(st, lambda a, b: a + b),     # i32.add
    "diff":    lambda st: _bin(st, lambda a, b: a - b),     # i32.sub
    "produit": lambda st: _bin(st, lambda a, b: a * b),     # i32.mul
    "quotient":lambda st: _bin(st, lambda a, b: a // b if b else 0),  # i32.div
    "egal":    lambda st: _bin(st, lambda a, b: a == b),
    "<":       lambda st: _bin(st, lambda a, b: a < b),
    ">":       lambda st: _bin(st, lambda a, b: a > b),
    "<=":      lambda st: _bin(st, lambda a, b: a <= b),
    ">=":      lambda st: _bin(st, lambda a, b: a >= b),
    "dup":     lambda st: st.append(st[-1]),
    "drop":    lambda st: st.pop(),
    "swap":    lambda st: st.__setitem__(slice(-2, None), [st[-1], st[-2]]),
    "over":    lambda st: st.append(st[-2]),
}


class Xerb:
    """L'interprète : un FLOT (pile), des KOINS (locals), des BIONS (mots définis)."""
    def __init__(self):
        self.words = {}     # nom -> liste de tokens (bion composé)

    def define(self, toks, i):
        # `: nom … ;`  -> enregistre le bion
        name = toks[i + 1]
        body = []
        j = i + 2
        while j < len(toks) and toks[j] != ";":
            body.append(toks[j]); j += 1
        self.words[name] = body
        return j + 1

    def run(self, toks, st=None, koins=None):
        st = st if st is not None else []
        koins = koins if koins is not None else []
        i = 0
        while i < len(toks):
            t = toks[i]
            if t == ":":
                i = self.define(toks, i); continue
            if t == "valeur":              # i32.const
                st.append(int(toks[i + 1])); i += 2; continue
            if t == "puise":               # local.get
                st.append(koins[int(toks[i + 1])]); i += 2; continue
            if t == "pose":                # local.set
                idx = int(toks[i + 1])
                while len(koins) <= idx: koins.append(0)
                koins[idx] = st.pop(); i += 2; continue
            if t in IONS:
                IONS[t](st); i += 1; continue
            if t in self.words:            # un bion composé : on le déroule
                self.run(self.words[t], st, koins); i += 1; continue
            try:                           # tolérance : un nombre nu = valeur
                st.append(int(t)); i += 1; continue
            except ValueError:
                raise SystemExit("ion inconnu : %r" % t)
        return st


def execute(src, label):
    """Joue un programme, le content-adresse, le grave : programme déterministe = TSOIN."""
    toks = src.replace(";", " ; ").replace(":", " : ").split()
    x = Xerb()
    flot = x.run(toks)
    res = flot[-1] if flot else None
    # le TSOIN : (générateur = le programme) -> (réel = le flot). addr = content-address.
    tsoin = "%s => %s" % (src.strip(), flot)
    aid = addr64(tsoin)
    name = "xerb:%s:%s" % (label, aid[:8])
    ok = grave(name, tsoin)
    print("  %-22s %-40s => %-14s  tsoin %s %s"
          % (label, src.strip()[:40], flot, aid[:12], "✦" if ok else "(bus off)"))
    return res, aid


DEMOS = [
    ("add",        "valeur 2 valeur 3 somme"),                       # 5
    ("carre",      ": carre dup produit ;  valeur 7 carre"),         # 49
    ("certitude",  "valeur 1000 valeur 9 diff"),                     # certitude=1000-residu(milli) => 991
    ("compose",    ": double dup somme ;  : quad double double ;  valeur 5 quad"),  # 20
    ("le-flot",    "valeur 3 valeur 4 produit valeur 12 egal"),      # 3*4==12 -> 1 (vrai)
]


def main():
    if len(sys.argv) > 1:
        execute(" ".join(sys.argv[1:]), "inline")
        return
    print("=== programmer en XERBOXION = programmer en TSOINS ===")
    print("   (chaque programme déterministe est content-adressé et gravé sur la machine à tsoins)\n")
    for label, src in DEMOS:
        execute(src, label)
    print("\n   un programme = un bion d'ions = un tsoin. Rejoue-le : même adresse, même réel. Certitude.")


if __name__ == "__main__":
    main()
