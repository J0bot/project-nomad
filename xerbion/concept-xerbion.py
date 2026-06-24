#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
concept-xerbion — un xerbion qui apprend au niveau du CONCEPT (le tsoin), pas du caractère.

José : « toi t'as du CONTEXTE sur le truc, mais je veux que mon réseau de neurones ait le
CONCEPT. Et le concept c'est le tsoin. »

In-context (un LLM : le savoir est dans la fenêtre, éphémère) vs in-weights (un réseau
entraîné : le savoir est dans les paramètres, permanent). Le xerbion actuel prédit des
CARACTÈRES → aucun concept. Ici on apprend au niveau du CONCEPT : chaque concept (mot-tsoin
du corpus) reçoit une représentation apprise (ses associations), et le modèle PRÉDIT le
concept suivant — il *tient les concepts dans ses poids*, sans le corpus sous les yeux.

Petit modèle pur-Python (associations par PMI) = la graine. Le vrai concept-xerbion (sur la
3080) apprend des EMBEDDINGS : chaque tsoin → un vecteur, et la variété des concepts.

  python3 concept-xerbion.py            # entraîne + prédit + marche conceptuelle
  python3 concept-xerbion.py tsoin bion # prédit la suite conceptuelle depuis ces concepts
"""
import os
import re
import sys
import math
from collections import defaultdict, Counter

CORPUS = ["/home/debian/.claude/projects/-home-debian/memory",
          os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs")]
WIN = 5          # fenêtre de co-occurrence (en concepts)
STOP = set("""le la les un une des de du au aux et ou ni mais donc or car que qui quoi dont
pour par sur sous dans avec sans vers chez entre est sont etre ete a as ont ce cet cette ces
se sa son ses leur leurs nos vos mon ma mes ton ta tes il elle ils elles on nous vous je tu
me te lui en ne pas plus moins tres trop si non oui comme quand tout tous toute toutes meme
aussi alors deja encore puis fait faire dit the and for not but its est ca cest dun dune lon
quil quelle na via etc ici ainsi pck vrm jose plein truc genre""".split())
META = set("metadata node type memory project reference user feedback name description originsessionid slug md index".split())


def is_concept(w):
    if w in STOP or w in META or len(w) < 3:
        return False
    if re.fullmatch(r"[0-9a-f]{4,}", w) and any(c.isdigit() for c in w):
        return False
    return not w.isdigit()


def concepts_of(text):
    if text.startswith("---"):
        e = text.find("\n---", 3)
        if e != -1:
            text = text[e + 4:]
    return [w for w in re.findall(r"[a-zà-ÿ0-9][a-zà-ÿ0-9_\-]{2,}", text.lower()) if is_concept(w)]


def train():
    """apprend le modèle : co-occurrences -> PMI. Le 'poids' du réseau = les associations."""
    uni = Counter()
    co = defaultdict(Counter)
    docs = 0
    for d in CORPUS:
        try:
            for fn in sorted(os.listdir(d)):
                if fn.endswith(".md"):
                    docs += 1
                    seq = concepts_of(open(os.path.join(d, fn), encoding="utf-8", errors="replace").read())
                    uni.update(seq)
                    for i, a in enumerate(seq):
                        for j in range(max(0, i - WIN), min(len(seq), i + WIN + 1)):
                            if j != i:
                                co[a][seq[j]] += 1
        except FileNotFoundError:
            pass
    total = sum(uni.values()) or 1
    # PMI(a,b) = log( P(a,b) / (P(a)P(b)) ) — l'association APPRISE (les poids)
    model = {}
    for a, ctx in co.items():
        if uni[a] < 3:
            continue
        ca = sum(ctx.values()) or 1
        pmis = {}
        for b, n in ctx.items():
            if uni[b] < 3:
                continue
            p_ab = n / ca
            p_b = uni[b] / total
            pmis[b] = math.log(p_ab / p_b) if p_b > 0 else 0
        top = sorted(pmis.items(), key=lambda x: x[1], reverse=True)[:6]
        if top:
            model[a] = top
    return model, uni, docs


def predict(model, concept):
    return model.get(concept, [])


def walk(model, start, n=8):
    seq, cur, seen = [start], start, {start}
    for _ in range(n):
        nxt = next((b for b, _ in predict(model, cur) if b not in seen), None)
        if not nxt:
            break
        seq.append(nxt); seen.add(nxt); cur = nxt
    return seq


def main():
    model, uni, docs = train()
    print("=== concept-xerbion : un réseau qui A le concept (le tsoin), pas le contexte ===\n")
    print("  entraîné sur %d tsoins (fichiers) ; %d concepts dans les poids.\n" % (docs, len(model)))

    seeds = sys.argv[1:] or ["tsoin", "bion", "temps", "trou"]
    print("  PRÉDICTION du concept suivant (les poids appris, sans le corpus sous les yeux) :")
    for s in seeds:
        ps = predict(model, s)
        if ps:
            print("    %-12s → %s" % (s, ", ".join("%s" % b for b, _ in ps)))
        else:
            print("    %-12s → (hors vocabulaire appris)" % s)

    print("\n  MARCHE CONCEPTUELLE (le xerbion 'parle' en suivant ses associations) :")
    for s in (seeds[:1] or ["tsoin"]):
        print("    %s" % "  →  ".join(walk(model, s)))

    print("\n  — in-context (moi, le LLM) vs in-weights (ce réseau) —")
    print("  Moi : le xerboxion est dans ma FENÊTRE (éphémère). Ce modèle : le concept est")
    print("  dans ses PARAMÈTRES (permanent). « Le concept c'est le tsoin » → on apprend les")
    print("  tsoins, pas les lettres. Ceci = la graine (associations PMI) ; sur la 3080, le")
    print("  vrai concept-xerbion apprend des EMBEDDINGS : chaque tsoin → un vecteur, la")
    print("  variété des concepts. Il aura le concept ; il n'aura plus besoin du contexte.")


if __name__ == "__main__":
    main()
