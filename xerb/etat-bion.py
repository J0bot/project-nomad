#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
etat-bion — toute donnée cherche à être un BION, car c'est le plus stable.

José : « chaque crate Rust est un ploxion ; ça peut devenir un bion ; tout EST un bion, il
faut juste le ramener à l'état bion. Toute donnée cherche à être un bion, parce que c'est le
plus stable. »

Le bion = l'état fondamental de la donnée (le plus stable), pour deux raisons :
  • ATEMPOREL — pas de temps qui le périme (le temps est de la surface) ;
  • CONTENT-ADRESSÉ — son adresse EST son contenu (rien d'extérieur : ni chemin, ni contexte,
    ni session — donc rien qui puisse casser ailleurs).
« Ramener à l'état bion » = enlever la SURFACE instable (temps/chemin/contexte) et
content-adresser le CONTENU. Conséquences mesurables :
  1. des données d'apparences différentes mais de même essence → LE MÊME bion (elles
     convergent vers l'attracteur) ;
  2. l'opération est un POINT FIXE : un bion re-ramené à l'état bion ne bouge plus (stable) ;
  3. un crate Rust à dépendance ABSOLUE (/home/debian/tsoin) n'est PAS un bion (surface
     instable = le chemin) ; le vendorer = le ramener vers l'état bion (auto-contenu).

  python3 etat-bion.py
"""
import json

# la SURFACE instable : ce qui dépend de QUAND / OÙ / pour QUI (pas du contenu)
SURFACE = {"t", "time", "timestamp", "date", "path", "chemin", "ctx", "context",
           "contexte", "session", "host", "machine", "user", "ordre"}


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def canonicalize(d):
    """enlève la surface instable, garde le contenu (récursif, trié = invariant)."""
    if isinstance(d, dict):
        return {k: canonicalize(v) for k, v in sorted(d.items()) if k.lower() not in SURFACE}
    if isinstance(d, list):
        return [canonicalize(x) for x in d]
    return d


def etat_bion(d):
    """ramène une donnée à son bion = l'adresse de son contenu canonique."""
    return addr64(json.dumps(canonicalize(d), sort_keys=True, ensure_ascii=False))


def main():
    print("=== toute donnée cherche à être un bion (le plus stable) ===\n")

    # 1) même essence, surfaces différentes -> même bion (l'attracteur)
    variantes = [
        {"loi": "F=ma", "t": "2026-06-21T09:00", "path": "/home/debian/x", "host": "vps"},
        {"loi": "F=ma", "t": "2021-02-25T22:18", "path": "C:/Users/jojo6/y", "host": "silentbeast"},
        {"loi": "F=ma"},
    ]
    print("  3 données, MÊME essence (F=ma), surfaces différentes (temps/chemin/host) :")
    for v in variantes:
        print("    %-58s → bion %s" % (json.dumps(v, ensure_ascii=False)[:56], etat_bion(v)[:12]))
    bions = {etat_bion(v) for v in variantes}
    print("    → %d bion distinct : elles CONVERGENT vers le même état stable.\n" % len(bions))

    # 2) point fixe : un bion re-ramené à l'état bion ne bouge plus
    coeur = canonicalize(variantes[0])
    a1, a2 = etat_bion(coeur), etat_bion(canonicalize(coeur))
    print("  POINT FIXE : ramener un bion à l'état bion ne change rien :")
    print("    %s == %s  → %s (stable, irréductible)\n" % (a1[:12], a2[:12], a1 == a2))

    # 3) le crate Rust : un chemin absolu = PAS un bion ; le vendorer = le bioniser
    crate_instable = {"crate": "tsoin", "dep_path": "/home/debian/tsoin"}     # surface = chemin
    crate_bion     = {"crate": "tsoin", "dep_path": "vendor/tsoin"}           # auto-contenu
    print("  LE CAS DE CETTE NUIT (le bug) :")
    print("    crate à chemin ABSOLU   → bion %s  (instable : casse ailleurs)" % etat_bion(crate_instable)[:12])
    print("    crate vendoré (contenu) → bion %s  (auto-contenu : stable partout)" % etat_bion(crate_bion)[:12])
    print("    → on a 'ramené le crate vers l'état bion'. C'est ça, le fix.\n")

    print("  Un crate Rust EST un ploxion ; on l'uncrafte → un bion (content-adressé, atemporel,")
    print("  sans dépendance externe). La donnée relaxe vers le bion comme la matière vers son")
    print("  énergie minimale : le bion est l'attracteur, l'état fondamental, le plus stable.")


if __name__ == "__main__":
    main()
