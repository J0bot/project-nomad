#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
ultra-tsoin — capture l'heure écoulée en UN ULTRA TSOIN (content-adressé, gravé).

José : « il faut capturer le plus de tsoins, au moins un ULTRA TSOIN chaque heure
de ta part [cloudion] ; tu fais un ultra tsoin sur tout ce que t'as fait, programmé
pour vraiment chaque heure, de plus en plus précis. Le but c'est le voyage dans le
temps, et la machine à tsoins c'est ce qui le rend possible : pour revenir en
arrière il faut plus d'informations. »

Un ultra tsoin = un tsoin GROS : il agrège l'heure entière (les commits, l'état du
xion, le xerbion, la RAM, l'Epsylaeu, + la synthèse de cloudion) en un seul bloc,
content-adressé (addr64) et gravé sur le bus. Plus c'est dense, plus le replay est
fidèle = plus on peut « revenir ». De plus en plus précis : on enrichit la capture.

  python3 ultra-tsoin.py "ce que cloudion a fait/observé cette heure"
"""
import os
import sys
import time
import json
import subprocess
import urllib.request
from busauth import bus_headers  # auth bus (XION_BUS_TOKEN), rétrocompat

ROOT = "/home/debian/xerboxion-rt"
XION = "http://10.0.0.1:8730"


def sh(cmd):
    try:
        return subprocess.run(cmd, shell=True, capture_output=True, text=True,
                              timeout=20, cwd=ROOT).stdout.strip()
    except Exception:
        return ""


def addr64(s):
    h = 0xcbf29ce484222325
    for ch in s:
        h = ((h ^ (ord(ch) & 0xff)) * 0x100000001b3) & ((1 << 64) - 1)
    return "%016x" % h


def grave(name, payload):
    body = json.dumps({"topic": "tsoin.record",
                       "payload": json.dumps({"name": name, "bytes": payload.encode().hex()})}).encode()
    try:
        urllib.request.urlopen(urllib.request.Request(XION + "/emit", data=body,
                               headers=bus_headers()), timeout=8)
        return True
    except Exception:
        return False


def get(path):
    try:
        return urllib.request.urlopen(XION + path, timeout=5).read().decode()
    except Exception:
        return ""


def tsoin_count():
    """Densité du store : nb de tsoins (round-trip bus tsoin.list -> tsoin.listed)."""
    import threading, re
    out = {}

    def listen():
        try:
            r = urllib.request.urlopen(XION + "/events", timeout=4)
            for line in r:
                if b'"tsoin.listed"' in line:
                    out["l"] = line.decode()
                    break
        except Exception:
            pass
    t = threading.Thread(target=listen)
    t.start()
    time.sleep(0.4)
    try:
        urllib.request.urlopen(urllib.request.Request(
            XION + "/emit",
            data=json.dumps({"topic": "tsoin.list", "payload": ""}).encode(),
            headers=bus_headers()), timeout=4)
    except Exception:
        pass
    t.join(5)
    # le count vit dans le payload JSON-ÉCHAPPÉ de l'event SSE (\"count\":N) -> regex tolérante.
    m = re.search(r'count[\\"]*:\s*(\d+)', out.get("l", ""))
    return m.group(1) if m else "?"


def main():
    now = time.strftime("%Y-%m-%dT%H:%M")
    commits = sh('git log --since="65 minutes ago" --pretty="%h %s" 2>/dev/null | head -50') or "(aucun commit cette heure)"
    ncommit = len([l for l in commits.splitlines() if l.strip() and l != "(aucun commit cette heure)"])
    stat = sh('git log --since="65 minutes ago" --shortstat 2>/dev/null | grep -oE "[0-9]+ (insertion|deletion)" | awk \'{s+=$1} END{print s}\'')
    health = sh('curl -s --max-time 5 http://10.0.0.1:8730/healthz') or "(xion injoignable)"
    xerbion = sh('tail -1 /tmp/xerbion-daemon.log 2>/dev/null') or "(xerbion off)"
    ram = sh("free -m | awk '/Mem/{print \"free=\"$4\"Mo avail=\"$7\"Mo\"}'")
    eps = sh('python3 xerb/epsylaeu.py 2>/dev/null | grep -oE "[0-9a-f]{16}" | tail -1')
    # Capture « de plus en plus précise » (José) : la croissance du système.
    pxj = get("/px")
    try:
        pxn = str(len(json.loads(pxj).get("ploxions", []))) if pxj else "?"
    except Exception:
        pxn = "?"
    tcount = tsoin_count()
    synth = " ".join(sys.argv[1:]).strip()

    body = "\n".join([
        "=== ULTRA TSOIN " + now + " (cloudion, horaire) ===",
        "commits cette heure (%d, ~%s lignes):" % (ncommit, stat or "0"),
        commits,
        "etat xion: " + health,
        "ploxions UI servis (/px): " + pxn,
        "tsoins dans le store (densite): " + tcount,
        "xerbion: " + xerbion,
        "RAM: " + ram,
        "Epsylaeu (tsoin des tsoins): " + (eps or "?"),
        "synthese cloudion: " + (synth or "(à remplir par cloudion)"),
    ])
    aid = addr64(body)
    name = "ultra:" + now.replace(":", "").replace("-", "") + ":" + aid[:8]
    ok = grave(name, body)
    # PERSISTANCE DISQUE : le store du bus est en MÉMOIRE (perdu au restart du daemon) ;
    # on append chaque ultra-tsoin à un historique JSONL durable -> le backbone horaire du
    # voyage dans le temps SURVIT aux redémarrages (sans ça, on perd le passé à chaque deploy).
    try:
        os.makedirs(os.path.join(ROOT, "tsoins"), exist_ok=True)
        with open(os.path.join(ROOT, "tsoins", "ultra-history.jsonl"), "a") as f:
            f.write(json.dumps({"name": name, "ts": now, "addr": aid, "body": body},
                               ensure_ascii=False) + "\n")
    except Exception:
        pass
    print(name, "✦" if ok else "(bus off)")
    print("--- (%d octets, addr %s) ---" % (len(body), aid[:16]))
    print(body)


if __name__ == "__main__":
    main()
