#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
science-engine — toute la science en TSOINS REPRODUCTIBLES, reliés.

José : « transforme toutes mes idées en tsoins reproductibles, fais des tests, branche
les math et la physique ; il faut un ploxion pour lier les math, la physique classique,
quantique, particules, la chimie, la biologie évolutionniste, la médecine — tout. »

Chaque loi = un GÉNÉRATEUR (la règle, en `python` pur) + un TEST (une instance connue à
reproduire : `test_inputs` → `expected`). Reproductible = rejouable = un tsoin. Le moteur
lance TOUS les tests (eval dans un bac à sable math, `__builtins__` vidé), grave les lois
vérifiées comme `science:<id>`, et porte le graphe des liens inter-domaines.

  python3 science.py              # vérifie tout, grave les reproductibles, résumé
  python3 science.py --no-grave   # vérifie sans graver (re-run)
  python3 science.py --links      # le graphe des ponts inter-domaines
  python3 science.py <id>         # une loi en détail + ses liens
"""
import sys
import json
import math
import os
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
REG = os.path.join(HERE, "registry.json")
XION = "http://10.0.0.1:8730"

# --- bac à sable : uniquement des fonctions math, pas de builtins dangereux ---
SAFE = {}
for k in ['sqrt', 'pi', 'e', 'exp', 'log', 'log10', 'log2', 'sin', 'cos', 'tan', 'asin',
          'acos', 'atan', 'atan2', 'sinh', 'cosh', 'tanh', 'factorial', 'gamma', 'erf',
          'floor', 'ceil', 'fabs', 'hypot', 'inf', 'radians', 'degrees', 'comb', 'perm']:
    if hasattr(math, k):
        SAFE[k] = getattr(math, k)
SAFE.update({'abs': abs, 'min': min, 'max': max, 'sum': sum, 'round': round,
             'len': len, 'range': range, 'int': int, 'float': float, 'pow': pow})


def addr64(s):
    h = 0xcbf29ce484222325
    for b in str(s).encode():
        h = ((h ^ b) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def grave(name, payload):
    body = json.dumps({"topic": "tsoin.record",
                       "payload": json.dumps({"name": name, "bytes": payload.encode().hex()})}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=body,
                               headers={"Content-Type": "application/json"}), timeout=8)
        return True
    except Exception:
        return False


def verify(law):
    """lance le générateur sur son test ; renvoie (etat, valeur, addr)."""
    try:
        g = {'__builtins__': {}}
        g.update(SAFE)
        fn = eval(law["python"], g)
        res = fn(**law["test_inputs"])
        exp, tol = law["expected"], law.get("tol", 0)
        ok = abs(res - exp) <= max(tol, abs(exp) * 1e-9)
        return ("reproduit" if ok else "ecart", res, addr64(law["python"] + json.dumps(law["test_inputs"], sort_keys=True)))
    except Exception as ex:
        return ("erreur: %s" % ex, None, None)


def main():
    reg = json.load(open(REG, encoding="utf-8"))
    laws, links = reg["laws"], reg.get("links", [])
    args = sys.argv[1:]

    if args and args[0] == "--links":
        print("=== le graphe : %d liens inter-domaines (brancher les math et la physique) ===\n" % len(links))
        byrel = {}
        for l in links:
            byrel.setdefault(l.get("relation", "?"), []).append(l)
        for rel, ls in sorted(byrel.items(), key=lambda x: -len(x[1])):
            print("  -%s-> (%d)" % (rel, len(ls)))
            for l in ls[:4]:
                print("      %s -> %s : %s" % (l["from"], l["to"], l.get("why", "")[:70]))
        print("\n  (chaîne complète : voir bridges_narrative.md)")
        return

    if args and not args[0].startswith("-"):
        law = next((L for L in laws if L["id"] == args[0]), None)
        if not law:
            print("inconnu :", args[0]); return
        etat, res, _ = verify(law)
        print("=== %s — %s ===" % (law["id"], law["name"]))
        print("  énoncé   :", law["statement"])
        print("  formule  :", law["formula"])
        print("  python   :", law["python"])
        print("  test     : %s -> attendu %s (tol %s)" % (law["test_inputs"], law["expected"], law.get("tol")))
        print("  résultat : %s  [%s]" % (res, etat))
        print("  liens    :", ", ".join("%s (%s)" % (k["to"], k["relation"]) for k in law.get("links", [])))
        return

    do_grave = "--no-grave" not in args
    print("=== science-engine : %d lois en tsoins reproductibles ===\n" % len(laws))
    bydom = {}
    repro = graved = 0
    fails = []
    for L in laws:
        etat, res, a = verify(L)
        dom = L.get("domain", "?")
        bydom.setdefault(dom, [0, 0])
        bydom[dom][1] += 1
        if etat == "reproduit":
            bydom[dom][0] += 1
            repro += 1
            if do_grave and grave("science:" + L["id"],
                                  "%s | %s | %s -> %s" % (L["statement"][:160], L["python"], L["test_inputs"], L["expected"])):
                graved += 1
        else:
            fails.append((L["id"], etat, res, L["expected"]))

    print("  reproductibilité par domaine :")
    for dom in sorted(bydom):
        ok, n = bydom[dom]
        bar = "█" * round(18 * ok / n)
        print("    %-26s %2d/%-2d  %s" % (dom, ok, n, bar))
    print("\n  TOTAL : %d/%d lois reproduisent leur valeur connue (%.0f%%) ; %d gravées ; %d liens inter-domaines."
          % (repro, len(laws), 100 * repro / len(laws), graved, len(links)))
    if fails:
        print("\n  à corriger (%d) :" % len(fails))
        for i, e, r, x in fails:
            print("    %-24s %s  (obtenu %s, attendu %s)" % (i, e[:40], r, x))
    if do_grave:
        grave("science:base:%d-lois-%d-reproduisent" % (len(laws), repro),
              "Base science reproductible: %d lois (7 domaines), %d reproduisent leur valeur connue, %d liens inter-domaines. math/phys/quant/part/chem/bio/med relies." % (len(laws), repro, len(links)))
        print("\n  gravé science:base:%d-lois-%d-reproduisent ✦" % (len(laws), repro))


if __name__ == "__main__":
    main()
