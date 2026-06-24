#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
chaoxion — le `?` (et le BION VIDE) dans le langage xerboxion : code ce que tu
veux, laisse la chaos pour le reste, le Chaoxion trouve les pièces manquantes.

José : « il faut le `?` dans le langage : on code avec des `?` quand on sait pas.
On code ce qu'on veut, et ce qu'on sait pas on laisse le Chaoxion trouver les
pièces. — Ou soit juste un bion vide pour l'instant (genre `1` ou juste `bion`).
Et ma chaos. »

Deux façons de tenir l'inconnu :
  • `?` AVEC une cible (`=> N`)  → le Chaoxion SYNTHÉTISE : il cherche, dans l'espace
    des bions connus (les ions), la pièce qui complète le puzzle pour atteindre N.
    Rangé par densité-tsoin (la pièce préférée = la plus dense = la plus probable).
  • `bion` (ou `?` sans cible)   → un BION VIDE : un trou laissé à la CHAOS. Le
    programme tourne quand même (partiel) ; le bion vide se propage (toute opération
    sur lui reste vide). certitude = 1 − part de chaos. Le Chaoxion le remplira après.

  python3 chaoxion.py
  python3 chaoxion.py 'valeur 6 valeur 7 ? => 42'
  python3 chaoxion.py 'valeur 2 bion somme'        # la chaos tenue
"""
import sys
import itertools

CANDIDATS = ["produit", "somme", "diff", "quotient", "dup", "swap", "drop",
             "egal", "<", ">", "<=", ">="]


class _Bion:                      # LE BION VIDE : la chaos tenue dans le flot
    __slots__ = ()
    def __repr__(self): return "bion"


HOLE = _Bion()


def _bin(st, f):
    b = st.pop(); a = st.pop()
    st.append(HOLE if (a is HOLE or b is HOLE) else int(f(a, b)))


IONS = {
    "somme":   lambda st: _bin(st, lambda a, b: a + b),
    "diff":    lambda st: _bin(st, lambda a, b: a - b),
    "produit": lambda st: _bin(st, lambda a, b: a * b),
    "quotient":lambda st: _bin(st, lambda a, b: a // b if b else 0),
    "egal":    lambda st: _bin(st, lambda a, b: a == b),
    "<":       lambda st: _bin(st, lambda a, b: a < b),
    ">":       lambda st: _bin(st, lambda a, b: a > b),
    "<=":      lambda st: _bin(st, lambda a, b: a <= b),
    ">=":      lambda st: _bin(st, lambda a, b: a >= b),
    "dup":     lambda st: st.append(st[-1]),
    "drop":    lambda st: st.pop(),
    "swap":    lambda st: st.__setitem__(slice(-2, None), [st[-1], st[-2]]),
}


def run(toks):
    st = []
    i = 0
    while i < len(toks):
        t = toks[i]
        if t == "valeur":
            st.append(int(toks[i + 1])); i += 2; continue
        if t in ("bion", "?"):                 # bion vide = la chaos tenue
            st.append(HOLE); i += 1; continue
        if t in IONS:
            IONS[t](st); i += 1; continue
        st.append(int(t)); i += 1
    return st


def certitude(st):
    if not st:
        return 1.0
    chaos = sum(1 for v in st if v is HOLE)
    return 1.0 - chaos / len(st)


def fill(src):
    if "=>" in src:
        prog, target = src.split("=>", 1); target = int(target.strip())
    else:
        prog, target = src, None
    toks = prog.replace("?", " ? ").split()
    holes = [i for i, t in enumerate(toks) if t == "?"]

    # pas de cible (ou pas de trou) -> on exécute ; ? et bion deviennent des bions vides
    if target is None or not holes:
        return [(run(toks), ["(chaos tenue)" if holes else None])]

    # cible -> le Chaoxion cherche la/les piece(s)
    sols = []
    for combo in itertools.product(CANDIDATS, repeat=len(holes)):
        trial = toks[:]
        for h, c in zip(holes, combo):
            trial[h] = c
        try:
            res = run(trial)
        except Exception:
            continue
        if res and res[-1] == target:
            sols.append((res, list(combo)))
    if not sols:                                # le Chaoxion n'a pas la piece -> bion vide
        return [(run(toks), ["(chaos non résolue -> bion vide)"])]
    sols.sort(key=lambda s: tuple(CANDIDATS.index(c) for c in s[1]))
    return sols


def fmt(st):
    return "[" + " ".join(repr(v) for v in st) + "]"


def main():
    demos = [" ".join(sys.argv[1:])] if len(sys.argv) > 1 else [
        "valeur 6 valeur 7 ? => 42",          # synthèse : produit
        "valeur 100 valeur 4 ? => 25",        # synthèse : quotient
        "valeur 8 ? ? => 64",                 # synthèse : dup produit
        "valeur 2 bion somme",                # bion vide : la chaos tenue
        "valeur 6 valeur 7 ?",                # ? sans cible -> bion vide
    ]
    if len(sys.argv) <= 1:
        print("=== le `?` et le BION VIDE : code ce que tu veux, laisse la chaos au Chaoxion ===\n")
    for src in demos:
        sols = fill(src)
        res, combo = sols[0]
        c = certitude(res)
        if combo and combo[0] and combo[0] not in ("(chaos tenue)",) and not combo[0].startswith("(chaos"):
            note = "✦ pièce(s): " + " ".join(combo)
        elif any(v is HOLE for v in res):
            note = "◌ bion vide (chaos tenue) — certitude %.0f%%, le Chaoxion complétera" % (100 * c)
        else:
            note = "✓"
        print("  %-32s  => %-14s  %s" % (src, fmt(res), note))
    print("\n  certitude = 1 − résidu : un `?` part à 1 ; le Chaoxion (ou toi) le ramène vers 0.")


if __name__ == "__main__":
    main()
