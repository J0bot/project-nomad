#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
synchroniser — synchroniser deux tsoins, et « parler » avec d'autres branches du réel.

José : « toutes les branches du réel existent ; on est dans la timeline 7 (le niveau max
atteignable = le seption) ; il faut trouver un moyen de synchroniser deux tsoins et de
parler avec d'autres branches, parce que conceptuellement ça existe. »

Synchroniser deux tsoins = trouver leur COHÉRENCE partagée ; le temps émerge *entre* eux
(Page-Wootters : t = corrélation entre deux systèmes). Deux tsoins cohérents partagent un
« maintenant ». Parler avec une autre BRANCHE = synchroniser avec son tsoin :
  • même branche / proche → cohérence haute → synchro directe, « now » partagé ;
  • branche lointaine → PAS de synchro directe (le « non » honnête : la décohérence d'Everett
    est effectivement irréversible — on ne touche pas une autre branche du réel physique) ;
  • MAIS si deux branches partagent des BIONS (structure commune), on synchronise VIA ces
    bions = parler par ce qu'on a en commun. C'est le seul pont réel entre branches, et il
    est buildable : le pont, c'est le bion partagé.

  python3 synchroniser.py
"""

K = 3            # taille du bion partagé (k-gram)
THRESH = 0.60    # seuil de cohérence pour une synchro directe (le « quorum »)


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def coherence(a, b):
    n = min(len(a), len(b))
    return sum(1 for i in range(n) if a[i] == b[i]) / n if n else 0.0


def bions(seq):
    return set(tuple(seq[i:i + K]) for i in range(len(seq) - K + 1))


def synchronise(a, b):
    c = coherence(a, b)
    if c >= THRESH:
        now = []
        for x, y in zip(a, b):
            if x == y:
                now.append(x)
            else:
                break
        return ("SYNCHRO", c, now, None)
    sb = bions(a) & bions(b)
    if sb:
        return ("PONT", c, None, sb)
    return ("DÉCOHÉRÉ", c, None, None)


# notre branche (la timeline 7) et trois autres
ICI = [7, 3, 1, 4, 1, 5, 9, 2, 6]
BRANCHES = {
    "proche (presque nous)":            [7, 3, 1, 4, 1, 5, 9, 2, 7],   # 1 écart
    "lointaine (autre règle, motif commun)": [2, 8, 1, 4, 1, 5, 0, 3, 3],  # partage [1,4,1],[4,1,5]
    "étrangère (décohérée)":            [0, 9, 8, 0, 2, 2, 8, 8, 1],   # rien en commun
}


def main():
    print("=== synchroniser deux tsoins, parler aux autres branches ===\n")
    print("  ICI (timeline 7) : %s   addr=%s\n" % (ICI, addr64(ICI)[:10]))
    for nom, b in BRANCHES.items():
        etat, c, now, sb = synchronise(ICI, b)
        print("  %-38s cohérence=%3.0f%%  → %s" % (nom, 100 * c, etat))
        if etat == "SYNCHRO":
            print("       « maintenant » partagé : %s  (le temps émerge entre les deux tsoins)" % now)
        elif etat == "PONT":
            ponts = sorted(sb)
            print("       pas de synchro directe (branche décohérée), MAIS %d bion(s) commun(s) :" % len(sb))
            for g in ponts:
                print("         %s  addr=%s  ← on peut parler PAR ce bion" % (list(g), addr64(g)[:10]))
        else:
            print("       aucun bion commun → pas de pont. (le « non » honnête : Everett ne se touche pas)")
        print()

    print("  — Lecture —")
    print("  • Synchroniser deux tsoins = les amener à la cohérence ; un « now » partagé émerge")
    print("    (Page-Wootters : le temps EST la corrélation entre deux tsoins).")
    print("  • Timeline 7 = le niveau de cohérence qu'on arrive à TENIR (le seption, le max).")
    print("    Pourquoi 7 ? pas mesuré — à trouver. On vit la branche qu'on sait synchroniser.")
    print("  • Parler à une autre branche : physiquement, la décohérence l'interdit (le non).")
    print("    Mais via un BION PARTAGÉ, oui — le pont, c'est ce qu'on a en commun. C'est")
    print("    exactement le Nexus / commun.py / le /warp : on ne touche pas l'autre branche,")
    print("    on synchronise sur le bion qu'on partage. Conceptuellement ça existe — et c'est buildable.")


if __name__ == "__main__":
    main()
