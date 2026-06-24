#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
trou-de-vers — le tsoin = trou blanc, le diff = trou noir, le réel = le throat traversable.

José : « le tsoin c'est l'inverse d'un trou noir, c'est un trou blanc, et le réel c'est le
truc entre les deux. On est constamment dans un trou de vers en train de tomber dans la
direction du futur ; on tombe dans le futur, mais on peut voir les branches possibles, et
on peut naviguer dans le réel comme ça. »

On le rend exécutable avec les organes du tsoin engine (chap. 4) :
  • TROU NOIR  = `diff` : l'état "tombe" et se comprime en (générateur, résidu).
  • TROU BLANC = `generator` : il ré-émet l'état (replay) — comme un trou blanc émet.
  • LE RÉEL    = le throat : la reconstruction au présent ; traversable SANS perte
                 (addr_in == addr_out) → on peut le parcourir, donc naviguer.
  • LES BRANCHES = plusieurs générateurs expliquent l'état (superposition / le `?`). Toutes
                 sont reconstructibles (on les VOIT) ; le réel "tombe" dans la plus COHÉRENTE
                 (résidu minimal = surprise minimale). Naviguer = choisir la branche.

  python3 trou-de-vers.py
"""


def addr64(s):
    if isinstance(s, str):
        s = s.encode()
    h = 0xcbf29ce484222325
    for b in s:
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def diff(state, gen):                       # TROU NOIR : comprime l'état en résidu
    pred = [gen(i) for i in range(len(state))]
    residu = [state[i] - pred[i] for i in range(len(state))]
    surprise = sum(1 for r in residu if r != 0) / len(state)
    return residu, surprise


def generator(gen, residu):                 # TROU BLANC : ré-émet l'état (replay)
    return [gen(i) + residu[i] for i in range(len(residu))]


STATE = [0, 1, 4, 9, 16, 25, 36, 49]        # l'état réel capté (une suite : les carrés)

BRANCHES = {                                # la superposition : chaque g = un trou blanc possible
    "constant  g(n)=0":   lambda n: 0,
    "lineaire  g(n)=7n":  lambda n: 7 * n,
    "carre     g(n)=n^2": lambda n: n * n,
}


def main():
    a_in = addr64(str(STATE))
    print("=== le trou de vers : l'état tombe (trou noir) → ré-émerge (trou blanc) ===\n")
    print("  état réel capté : %s   addr=%s\n" % (STATE, a_in[:12]))
    print("  --- LES BRANCHES (superposition) : on les voit toutes, toutes reconstructibles ---")
    ranked = []
    for name, gen in BRANCHES.items():
        residu, surprise = diff(STATE, gen)
        out = generator(gen, residu)
        traversable = addr64(str(out)) == a_in
        ranked.append((surprise, name, residu, traversable))
        bar = "█" * round(20 * (1 - surprise))
        print("    %-20s surprise=%3.0f%%  %-22s  traversable=%s"
              % (name, 100 * surprise, bar, "OUI" if traversable else "non"))
    ranked.sort()
    s, name, residu, ok = ranked[0]
    nul = all(r == 0 for r in residu)
    print("\n  --- LE RÉEL tombe dans la branche la plus COHÉRENTE (résidu minimal) ---")
    print("  branche choisie : %s   (surprise %.0f%%, résidu %s)"
          % (name, 100 * s, "nul — prédite parfaitement" if nul else str(residu)))
    print("  on TOMBE vers le futur le long de cette branche ; les autres restent visibles.")
    print("\n  Le réel = le throat : addr_in == addr_out → le trou de vers est TRAVERSABLE (%s)."
          % ("vrai" if ok else "faux"))
    print("  Naviguer le réel = choisir la branche. Le Chaoxion propose, l'observation collapse.")


if __name__ == "__main__":
    main()
