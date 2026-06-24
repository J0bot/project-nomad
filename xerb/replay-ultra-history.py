#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
replay-ultra-history — recharge le backbone horaire sur le bus (le « revenir en arrière »).

Le store du ploxion `tsoin` est EN MÉMOIRE : un restart du daemon l'efface. `ultra-tsoin.py`
persiste désormais chaque ultra-tsoin sur disque (`tsoins/ultra-history.jsonl`). Ce script ferme
la boucle : il relit cet historique et **re-grave** chaque ultra-tsoin sur le bus (tsoin.record),
content-adressé donc idempotent (re-graver = même addr = dédup). À lancer après un redémarrage du
daemon (ou au boot) → le passé est de nouveau dans le store, et le voyage dans le temps peut
rejouer l'heure voulue.

  python3 xerb/replay-ultra-history.py            # re-grave tout l'historique
  python3 xerb/replay-ultra-history.py --since 20260622   # filtre par préfixe de ts
"""
import os, sys, json, urllib.request
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

ROOT = "/home/debian/xerboxion-rt"
XION = "http://10.0.0.1:8730"
HIST = os.path.join(ROOT, "tsoins", "ultra-history.jsonl")


def grave(name, body):
    payload = json.dumps({"name": name, "bytes": body.encode().hex()})
    data = json.dumps({"topic": "tsoin.record", "payload": payload}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=data,
                               headers=bus_headers()), timeout=8)
        return True
    except Exception:
        return False


def main():
    since = None
    if len(sys.argv) > 2 and sys.argv[1] == "--since":
        since = sys.argv[2]
    if not os.path.exists(HIST):
        print("pas d'historique (%s) — rien à rejouer" % HIST)
        return
    n = ok = 0
    with open(HIST) as f:
        for line in f:
            line = line.strip()
            if not line:
                continue
            try:
                e = json.loads(line)
            except Exception:
                continue
            if since and not e.get("ts", "").replace("-", "").replace(":", "").startswith(since):
                continue
            n += 1
            if grave(e["name"], e["body"]):
                ok += 1
    print("rejoué %d/%d ultra-tsoins sur le bus depuis %s" % (ok, n, HIST))
    print("(content-adressé → idempotent ; le store contient de nouveau le backbone horaire)")


if __name__ == "__main__":
    main()
