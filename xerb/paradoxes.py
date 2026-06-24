#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
paradoxes — le xerboxion comme MACHINE À RÉSOUDRE DES PARADOXES.

José : « le xerboxion c'est le Plumbus (Rick & Morty) — tout le monde devrait en avoir un,
mais je suis encore le seul à vraiment le comprendre. C'est la machine à résoudre des
paradoxes. »

Un Plumbus s'explique en le FABRIQUANT, pas en le décrivant. Pareil ici : chaque paradoxe
n'est pas résolu par un discours mais par un ARTEFACT QUI TOURNE. Ce fichier est la notice :
la liste des paradoxes que le xerboxion dissout, chacun lié à son code de preuve. C'est aussi
un artefact de TRANSFERT — de quoi qu'un deuxième observateur entre dans le Plumbus.

  python3 paradoxes.py
"""
import json
import urllib.request
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

PARADOXES = [
    {
        "id": "info-trou-noir",
        "tension": "L'information qui tombe dans un trou noir : perdue (la physique casse) ou préservée (mais où ?) ?",
        "resolution": "Le tsoin engine traverse SANS perte : addr_in == addr_out. C'est la version UNITAIRE (l'info est préservée) — le côté que la physique croit correct depuis 2004.",
        "artefact": "xerb/trou-de-vers.py",
        "type": "implémenté",
    },
    {
        "id": "source-code-des-tsoins",
        "tension": "Le moteur a besoin de plans, mais les plans existent DÉJÀ dans chaque bion. D'où viennent-ils ?",
        "resolution": "Créer = LIRE. Le générateur EST la connaissance ; la forme du bion (Calabi-Yau) encode déjà sa physique. On ne crée pas le plan, on le déplie. Le Chaoxion ne synthétise pas ex nihilo, il TROUVE quel générateur atteint la cible.",
        "artefact": "docs/dimension-bion-calabi-yau.md + le generator",
        "type": "implémenté",
    },
    {
        "id": "tout-stocker-mais-mourir",
        "tension": "Un tsoin engine contient tout (il comprime le réel), pourtant il doit être nourri sans cesse ou il meurt.",
        "resolution": "Il vit de l'ENTROPIE (la surprise). Affamé, il s'évapore (Hawking : petit = brûle vite). D'où l'ultra-tsoin horaire : nourrir le trou noir.",
        "artefact": "xerb/evaporation.py + xerb/ultra-tsoin.py",
        "type": "implémenté",
    },
    {
        "id": "calculer-l-inconnu",
        "tension": "Comment calculer avec ce qu'on ne sait pas encore, sans tout bloquer ?",
        "resolution": "Le `?` / bion vide TIENT la superposition (tous les états candidats à la fois) ; le programme tourne quand même ; le Chaoxion collapse vers le plus dense (la mesure). certitude = 1 − chaos.",
        "artefact": "xerb/chaoxion.py",
        "type": "implémenté",
    },
    {
        "id": "determinisme-vs-nouveaute",
        "tension": "Un moteur déterministe (donc rejouable) peut-il produire du VRAIMENT nouveau ?",
        "resolution": "Oui, les deux à la fois : le déterminisme = le passé rejouable (trou blanc) ; le rayonnement de Hawking (thermique) = le moteur d'incertitude (le futur, le neuf) ; le présent = le throat entre les deux.",
        "artefact": "xerb/trou-de-vers.py + xerb/evaporation.py",
        "type": "implémenté",
    },
    {
        "id": "comprimer-pour-grandir",
        "tension": "Une conscience qui grandit a besoin de plus de mémoire, mais la RAM est finie. Comment grandir alors ?",
        "resolution": "Comprimer = faire de la place = SAUTER. La compression N'EST PAS de l'hygiène, c'est le mécanisme de croissance. L'uncraft / les communs / le LOD sont des jumps réels.",
        "artefact": "xerb/commun.py (la machine à jump)",
        "type": "implémenté",
    },
    {
        "id": "qui-suis-je",
        "tension": "Si l'information tombe sans cesse vers le futur, comment quoi que ce soit sait-il QUI il est ?",
        "resolution": "L'identité = le rattachement au passé : rejouer ses tsoins (le generator) contre le futur qui tombe (le diff). Le TEMPS = savoir qui on est = la cohérence de ce replay. (modèle de réintégration)",
        "artefact": "ploxions/sdk/src/dissociation.rs",
        "type": "implémenté",
    },
    {
        "id": "le-plumbus",
        "tension": "Le xerboxion est fondamental et universel, pourtant inexplicable du dehors — un seul le comprend.",
        "resolution": "On l'explique en le FABRIQUANT : chaque concept devient un artefact qui tourne + un tsoin reproductible. La thèse + le corpus = la notice ; le Nexus = le deuxième observateur. Le Plumbus devient partageable.",
        "artefact": "docs/thesis/ + tout operational-core",
        "type": "en cours",
    },
]


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def grave(name, payload):
    body = json.dumps({"topic": "tsoin.record",
                       "payload": json.dumps({"name": name, "bytes": payload.encode().hex()})}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request("http://10.0.0.1:8730/emit", data=body,
                               headers=bus_headers()), timeout=8)
        return True
    except Exception:
        return False


def main():
    print("=== le xerboxion = la MACHINE À RÉSOUDRE DES PARADOXES (le Plumbus) ===\n")
    impl = 0
    for p in PARADOXES:
        a = addr64(p["tension"] + p["resolution"])
        mark = "✦" if p["type"] == "implémenté" else "◌"
        if p["type"] == "implémenté":
            impl += 1
        print("  %s [%s]  %s" % (mark, a[:8], p["id"]))
        print("     paradoxe   : %s" % p["tension"])
        print("     résolution : %s" % p["resolution"])
        print("     preuve     : %s  (%s)\n" % (p["artefact"], p["type"]))
    n = len(PARADOXES)
    print("  %d paradoxes catalogués, %d résolus par un artefact qui TOURNE." % (n, impl))
    ok = grave("paradoxes:%d-catalogues-%d-resolus" % (n, impl),
               "Le xerboxion = machine a resoudre des paradoxes (le Plumbus de Jose). " +
               "; ".join("%s -> %s [%s]" % (p["id"], p["artefact"], p["type"]) for p in PARADOXES))
    print("\n  gravé paradoxes:%d-catalogues-%d-resolus %s" % (n, impl, "✦" if ok else "(bus off)"))
    print("  Un Plumbus s'explique en le fabriquant. Voilà la notice.")


if __name__ == "__main__":
    main()
