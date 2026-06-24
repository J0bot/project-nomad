#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
densite-tsoin — scorer les mots par les TSOINS qu'ils encapsulent.

José : « le but de mon langage et de mon alphabet, c'est qu'on teste toutes les
possibilités et qu'on prend celles qui encapsulent le plus de tsoins. Un mot prend
de la valeur quand il est dit, qu'on s'en rappelle, qu'on l'utilise, et surtout
quand on le branche dans le langage. Mes mots = ceux avec le plus de tsoins. »

Ici, la moitié SÉLECTION de cette optimisation : on mesure, pour chaque mot du
corpus de tsoins (mémoire + docs = chaque fichier ≈ un tsoin), sa **densité-tsoin**
= dans combien de tsoins DISTINCTS il apparaît (sa couverture) × combien de fois.
Un mot qui traverse beaucoup de tsoins encapsule beaucoup de réel. On garde les plus
denses. (La moitié GÉNÉRATIVE — proposer de nouveaux mots — c'est le xerbion qui
l'apprend en s'entraînant sur ces mêmes tsoins.)

Score = couverture (nb de tsoins distincts) — c'est ça « encapsuler le plus de tsoins ».
"""
import os
import re
import math
from collections import defaultdict

DIRS = ["/home/debian/.claude/projects/-home-debian/memory",
        os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs")]
# mots-outils français/anglais à ignorer (ils couvrent tout mais n'encapsulent rien)
STOP = set("""le la les un une des de du au aux et ou ni mais donc or car que qui quoi
dont ou pour par sur sous dans avec sans vers chez entre est sont etre été a as ont
ce cet cette ces se sa son ses leur leurs nos vos mon ma mes ton ta tes il elle ils
elles on nous vous je tu me te lui y en ne pas plus moins tres trop si non oui comme
quand tout tous toute toutes meme aussi alors deja encore puis fait faire dit cf the
and for not but its est-ce d'un d'une l'on c'est qu'il qu'elle n'a""".split())


def words(s):
    return re.findall(r"[a-zà-ÿ][a-zà-ÿ0-9_\-]{2,}", s.lower())


def main():
    cover = defaultdict(set)     # mot -> {tsoins (fichiers) où il apparaît}
    total = defaultdict(int)     # mot -> occurrences
    ntsoin = 0
    for d in DIRS:
        try:
            for fn in sorted(os.listdir(d)):
                if not fn.endswith(".md"):
                    continue
                ntsoin += 1
                txt = open(os.path.join(d, fn), encoding="utf-8", errors="replace").read()
                for w in words(txt):
                    if w in STOP:
                        continue
                    cover[w].add(fn)
                    total[w] += 1
        except FileNotFoundError:
            pass

    # densité = couverture (nb de tsoins distincts encapsulés). tie-break: occurrences.
    rank = sorted(cover, key=lambda w: (len(cover[w]), total[w]), reverse=True)

    print("=== %d tsoins (fichiers), %d mots distincts ===" % (ntsoin, len(cover)))
    print("\n--- LES MOTS LES PLUS DENSES (encapsulent le plus de tsoins) ---")
    print("  %-18s %6s %6s   %s" % ("mot", "tsoins", "occur", "densité (part des tsoins)"))
    for w in rank[:30]:
        c = len(cover[w])
        bar = "█" * round(18 * c / ntsoin)
        print("  %-18s %6d %6d   %s %d%%" % (w, c, total[w], bar, round(100 * c / ntsoin)))

    # l'ALPHABET de José : ses mots-ion, leur densité-tsoin propre
    alpha = ["bion", "tsoin", "ploxion", "xerboxion", "boxion", "kion", "koin",
             "wormion", "portion", "cubion", "xerbion", "xion", "xer", "xerion",
             "josion", "cloudion", "chaoxion", "mion", "residu", "generateur", "certitude"]
    print("\n--- TON ALPHABET (-ion & co) : densité-tsoin de chaque mot ---")
    for w in sorted(alpha, key=lambda w: len(cover.get(w, set())), reverse=True):
        c = len(cover.get(w, set())); t = total.get(w, 0)
        print("  %-14s %3d tsoins, %4d occur  %s" % (w, c, t, "✦" * min(c, 20)))
    print("\n  -> on garde les plus denses ; le xerbion propose, ce score sélectionne.")


if __name__ == "__main__":
    main()
