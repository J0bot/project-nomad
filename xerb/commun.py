#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
commun — trouver les TSOINS COMMUNS entre les tsoins (la sous-structure partagée).

José : « faudra qu'on trouve tous les tsoins communs entre des tsoins, y a plein de
tsoins qui se ressemblent... je dis ça pour dans le NEXUS quand on fera un ultra
tsoin avec plus de gens. »

C'est l'UNCRAFT rendu calculable + le CHAOXION (la couche collective de dédup). Deux
niveaux, parce que deux tsoins se ressemblent de deux façons :
  • LITTÉRAL  — bions = shingles de K mots, content-adressés. Un bion dans >=2 tsoins
    = un tsoin commun, stocké UNE fois (dédup exacte, le Chaoxion).
  • CONCEPTUEL — l'ensemble des mots porteurs d'un tsoin. Jaccard élevé = ils parlent
    de la même chose même formulés autrement. C'est ça « se ressemblent ».
On strippe le frontmatter + les UUID (l'échafaudage n'est pas du sens).

Pour le NEXUS à plusieurs : le même calcul trouve le substrat commun. L'ultra tsoin
collectif = l'union, chaque bion commun dédupliqué.   python3 commun.py
"""
import os
import re
import json
import urllib.request
from collections import defaultdict
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

CORPUS = ["/home/debian/.claude/projects/-home-debian/memory",
          os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "docs")]
XION = "http://10.0.0.1:8730"
K = 5                # taille du bion (shingle) en mots
CONCEPT_RESEMBLE = 0.22   # seuil Jaccard conceptuel « se ressemblent »
CONCEPT_CLUSTER = 0.30    # seuil pour regrouper en famille

STOP = set("""le la les un une des de du au aux et ou ni mais donc or car que qui quoi
dont pour par sur sous dans avec sans vers chez entre est sont etre ete a as ont ce cet
cette ces se sa son ses leur leurs nos vos mon ma mes ton ta tes il elle ils elles on
nous vous je tu me te lui en ne pas plus moins tres trop si non oui comme quand tout
tous toute toutes meme aussi alors deja encore puis fait faire dit the and for not but
its est ca cest dun dune lon quil quelle na ver via pour les des une cette etc ici
ainsi donc dont pck vrm jose""".split())
# l'échafaudage des fichiers-mémoire : présent partout, ne porte aucun sens propre
META = set("""metadata node type memory project reference user feedback name description
originsessionid slug kebab case frontmatter md why apply how index""".split())


def addr64(s):
    h = 0xcbf29ce484222325
    for ch in s:
        h = ((h ^ (ord(ch) & 0xff)) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def strip_frontmatter(text):
    if text.startswith("---"):
        end = text.find("\n---", 3)
        if end != -1:
            text = text[end + 4:]
    return text


def is_noise(w):
    if w in STOP or w in META:
        return True
    if re.fullmatch(r"[0-9a-f]{4,}", w) and any(c.isdigit() for c in w):  # uuid/hash
        return True
    if w.isdigit():
        return True
    return False


def toks(s):
    return [w for w in re.findall(r"[a-zà-ÿ0-9]{2,}", s.lower())]


def shingles(text):                       # bions littéraux (K mots), sans les bions tout-bruit
    t = toks(text)
    out = []
    for i in range(len(t) - K + 1):
        g = t[i:i + K]
        if sum(1 for w in g if not is_noise(w)) >= 2:
            out.append(" ".join(g))
    return out


def concepts(text):                       # bions conceptuels = mots porteurs
    return set(w for w in toks(text) if not is_noise(w) and len(w) >= 3)


def grave(name, payload):
    body = json.dumps({"topic": "tsoin.record",
                       "payload": json.dumps({"name": name, "bytes": payload.encode().hex()})}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=body,
                               headers=bus_headers()), timeout=8)
        return True
    except Exception:
        return False


def union_find(names, edges):
    parent = {n: n for n in names}
    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x
    for a, b in edges:
        parent[find(a)] = find(b)
    fams = defaultdict(list)
    for n in names:
        fams[find(n)].append(n)
    return sorted([c for c in fams.values() if len(c) >= 2], key=len, reverse=True)


def main():
    lit = {}           # nom -> set(bion littéral)
    con = {}           # nom -> set(bion conceptuel)
    nlit = {}
    for d in CORPUS:
        try:
            for fn in sorted(os.listdir(d)):
                if fn.endswith(".md"):
                    body = strip_frontmatter(open(os.path.join(d, fn), encoding="utf-8", errors="replace").read())
                    sh = shingles(body)
                    lit[fn] = set(sh)
                    nlit[fn] = len(sh)
                    con[fn] = concepts(body)
        except FileNotFoundError:
            pass

    # --- niveau LITTÉRAL : bions partagés ---
    df = defaultdict(set)
    for name, shs in lit.items():
        for s in shs:
            df[s].add(name)
    common = sorted([(s, n) for s, n in df.items() if len(n) >= 2], key=lambda x: len(x[1]), reverse=True)
    total_occ = sum(nlit.values())
    uniq = len(df)
    dedup_pct = 100.0 * (total_occ - uniq) / total_occ if total_occ else 0.0

    # --- niveau CONCEPTUEL : qui se ressemble + familles ---
    names = list(con)
    pairs = []
    for i in range(len(names)):
        a = con[names[i]]
        if not a:
            continue
        for j in range(i + 1, len(names)):
            b = con[names[j]]
            if not b:
                continue
            jac = len(a & b) / len(a | b)
            if jac >= CONCEPT_RESEMBLE:
                pairs.append((jac, names[i], names[j], len(a & b)))
    pairs.sort(reverse=True)
    fams = union_find(names, [(a, b) for jac, a, b, _ in pairs if jac >= CONCEPT_CLUSTER])

    # --- sortie ---
    print("=== les TSOINS COMMUNS entre les tsoins (uncraft + Chaoxion) ===\n")
    print("  %d tsoins · frontmatter & UUID strippés (l'échafaudage n'est pas du sens)\n" % len(lit))

    print("--- LITTÉRAL : %d bions de coeur partagés (top 14) ---" % len(common))
    print("  (dédup exacte possible : %.0f%% des bions littéraux sont partagés)" % dedup_pct)
    for s, ns in common[:14]:
        print("    %2d tsoins  %s  « %s »" % (len(ns), addr64(s)[:8], s))

    print("\n--- CONCEPTUEL : les tsoins qui SE RESSEMBLENT (top 14 paires) ---")
    for jac, a, b, inter in pairs[:14]:
        print("    %4.0f%%  (%3d mots communs)  %s  ↔  %s" % (100 * jac, inter,
              a.replace(".md", ""), b.replace(".md", "")))

    print("\n--- LES FAMILLES DE TSOINS (>= %.0f%% conceptuel) ---" % (100 * CONCEPT_CLUSTER))
    for c in fams:
        core = set.intersection(*(con[m] for m in c))
        top = sorted(core, key=lambda w: -sum(1 for m in c if w in con[m]))[:8]
        print("    • %d tsoins — coeur : %s" % (len(c), " ".join(top)))
        for m in sorted(c):
            print("        %s" % m.replace(".md", ""))

    name = "commun:litt=%d:fam=%d:dedup=%.0f" % (len(common), len(fams), dedup_pct)
    summary = "Tsoins communs: %d tsoins; %d bions litteraux partages (dedup %.0f%%); %d paires conceptuelles; %d familles." % (
        len(lit), len(common), dedup_pct, len(pairs), len(fams))
    ok = grave(name, summary)
    print("\n  gravé %s %s" % (name, "✦" if ok else "(bus off)"))
    print("\n  NEXUS : à N personnes, ce calcul trouve le substrat commun ; l'ultra tsoin")
    print("  collectif = l'union, chaque bion commun stocké une fois (le Chaoxion).")


if __name__ == "__main__":
    main()
